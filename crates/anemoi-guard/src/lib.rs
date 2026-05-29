//! Static checks that catch tests which look green but verify nothing, and
//! production code that hides inside test-only modules.
//!
//! These two patterns were the root cause of a durable-event-store change that
//! passed `cargo test`, `fmt`, and `clippy` while delivering no working
//! feature: the only code path that read `ANEMOI_DATABASE_URL` lived in a
//! `pub fn` inside a `#[cfg(test)]` module (so the real binary never reached
//! it), and the tests asserted only `result.is_ok()` (so they could not tell a
//! working store from a broken one).
//!
//! A check fires unless the offending function carries an escape-hatch comment
//! containing `anemoi-guard:allow` somewhere within its span.

use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, Item, Macro, Visibility};

const ALLOW_MARKER: &str = "anemoi-guard:allow";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// A `pub`/`pub(...)` function declared inside a `#[cfg(test)]` module.
    /// Real tests never need to be public; visibility here is a tell that
    /// production logic is hiding where the shipped binary cannot reach it.
    PubFnInTestModule,
    /// A test whose only assertions are `is_ok`/`is_some` with no value
    /// comparison — it proves a call succeeded or produced *something*, not
    /// *what* it produced, so it cannot distinguish a working implementation
    /// from a broken one. Negative assertions (`is_err`/`is_none`) are left
    /// alone: asserting a refusal or an absence is a deliberate behavioral
    /// claim, not a missing value check.
    VacuousTest,
}

impl Rule {
    pub fn code(self) -> &'static str {
        match self {
            Rule::PubFnInTestModule => "pub-fn-in-test-module",
            Rule::VacuousTest => "vacuous-test",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub rule: Rule,
    pub item: String,
    pub line: usize,
    pub message: String,
}

/// Parse `source` and return every violation found. The `Err` case is reserved
/// for source that does not parse as Rust; callers should surface it rather
/// than treat unparseable input as clean.
pub fn analyze_source(source: &str) -> Result<Vec<Violation>, syn::Error> {
    let file = syn::parse_file(source)?;
    let lines: Vec<&str> = source.lines().collect();
    let mut violations = Vec::new();
    check_items(&file.items, false, &lines, &mut violations);
    Ok(violations)
}

/// A function is suppressed when an `anemoi-guard:allow` marker appears within
/// its token span, or in the contiguous block of comments/attributes directly
/// above it (the natural place to write the marker).
fn is_allowed(lines: &[&str], start: usize, end: usize) -> bool {
    if (start..=end).any(|line| line_contains_marker(lines, line)) {
        return true;
    }
    let mut line = start;
    while line > 1 {
        line -= 1;
        let text = lines.get(line - 1).map(|t| t.trim()).unwrap_or("");
        let is_decorator = text.is_empty()
            || text.starts_with("//")
            || text.starts_with("#[")
            || text.starts_with("#!");
        if !is_decorator {
            break;
        }
        if line_contains_marker(lines, line) {
            return true;
        }
    }
    false
}

fn line_contains_marker(lines: &[&str], line: usize) -> bool {
    lines
        .get(line - 1)
        .is_some_and(|text| text.contains(ALLOW_MARKER))
}

fn check_items(items: &[Item], in_test_module: bool, lines: &[&str], out: &mut Vec<Violation>) {
    for item in items {
        match item {
            Item::Mod(module) => {
                let nested_test = in_test_module || has_cfg_test(&module.attrs);
                if let Some((_, inner)) = &module.content {
                    check_items(inner, nested_test, lines, out);
                }
            }
            Item::Fn(func) => {
                let span = func.span();
                let (start, end) = (span.start().line, span.end().line);
                let allowed = is_allowed(lines, start, end);

                if in_test_module && !allowed && !matches!(func.vis, Visibility::Inherited) {
                    out.push(Violation {
                        rule: Rule::PubFnInTestModule,
                        item: func.sig.ident.to_string(),
                        line: start,
                        message: format!(
                            "`{}` is public inside a #[cfg(test)] module; production code reached \
                             only from tests is unreachable from the binary",
                            func.sig.ident
                        ),
                    });
                }

                if has_test_attr(&func.attrs) && !allowed && is_vacuous(func) {
                    out.push(Violation {
                        rule: Rule::VacuousTest,
                        item: func.sig.ident.to_string(),
                        line: start,
                        message: format!(
                            "test `{}` only asserts is_ok/is_some; assert the observed value, \
                             not just that a call succeeded or produced something",
                            func.sig.ident
                        ),
                    });
                }
            }
            _ => {}
        }
    }
}

fn has_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg")
            && matches!(&attr.meta, syn::Meta::List(list) if list.tokens.to_string().contains("test"))
    })
}

fn has_test_attr(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "test")
    })
}

fn is_vacuous(func: &syn::ItemFn) -> bool {
    let mut scan = AssertScan::default();
    scan.visit_block(&func.block);
    scan.weak >= 1 && scan.strong == 0
}

#[derive(Default)]
struct AssertScan {
    weak: usize,
    strong: usize,
}

impl<'ast> Visit<'ast> for AssertScan {
    fn visit_macro(&mut self, mac: &'ast Macro) {
        let name = mac
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .unwrap_or_default();
        match name.as_str() {
            "assert_eq" | "assert_ne" | "assert_matches" | "debug_assert_eq"
            | "debug_assert_ne" => self.strong += 1,
            "assert" | "debug_assert" => {
                if is_weak_condition(&mac.tokens.to_string()) {
                    self.weak += 1;
                } else {
                    self.strong += 1;
                }
            }
            _ => {}
        }
        syn::visit::visit_macro(self, mac);
    }
}

fn is_weak_condition(tokens: &str) -> bool {
    let probes = ["is_ok", "is_some"];
    let has_probe = probes.iter().any(|probe| tokens.contains(probe));
    let has_comparison = tokens.contains("==") || tokens.contains("!=");
    has_probe && !has_comparison
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(source: &str) -> Vec<Rule> {
        analyze_source(source)
            .expect("source parses")
            .into_iter()
            .map(|violation| violation.rule)
            .collect()
    }

    #[test]
    fn flags_pub_fn_inside_cfg_test_module() {
        let source = r#"
            #[cfg(test)]
            mod tests {
                pub fn create_decision_log() -> u32 { 1 }
            }
        "#;

        let found = analyze_source(source).expect("parses");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rule, Rule::PubFnInTestModule);
        assert_eq!(found[0].item, "create_decision_log");
    }

    #[test]
    fn ignores_private_fn_inside_cfg_test_module() {
        let source = r#"
            #[cfg(test)]
            mod tests {
                fn helper() -> u32 { 1 }
            }
        "#;

        assert_eq!(rules(source), Vec::<Rule>::new());
    }

    #[test]
    fn ignores_pub_fn_outside_test_module() {
        let source = "pub fn record_decision() -> u32 { 1 }";

        assert_eq!(rules(source), Vec::<Rule>::new());
    }

    #[test]
    fn flags_test_that_only_checks_is_ok() {
        let source = r#"
            #[cfg(test)]
            mod tests {
                #[test]
                fn stores_decision() {
                    let result = save();
                    assert!(result.is_ok());
                }
            }
        "#;

        let found = analyze_source(source).expect("parses");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rule, Rule::VacuousTest);
        assert_eq!(found[0].item, "stores_decision");
    }

    #[test]
    fn flags_test_that_only_checks_is_some() {
        let source = r#"
            #[cfg(test)]
            mod tests {
                #[tokio::test]
                async fn finds_record() {
                    assert!(lookup().is_some());
                }
            }
        "#;

        assert_eq!(rules(source), vec![Rule::VacuousTest]);
    }

    #[test]
    fn accepts_test_whose_only_assertion_is_a_refusal() {
        let source = r#"
            #[cfg(test)]
            mod tests {
                #[tokio::test]
                async fn live_execution_requires_flag() {
                    assert!(execute().await.is_err(), "should refuse without flag");
                }
            }
        "#;

        assert_eq!(rules(source), Vec::<Rule>::new());
    }

    #[test]
    fn accepts_test_whose_only_assertion_is_an_absence() {
        let source = r#"
            #[cfg(test)]
            mod tests {
                #[test]
                fn nothing_resident_yet() {
                    assert!(snapshot.last_inspected.is_none());
                }
            }
        "#;

        assert_eq!(rules(source), Vec::<Rule>::new());
    }

    #[test]
    fn accepts_test_that_compares_a_value() {
        let source = r#"
            #[cfg(test)]
            mod tests {
                #[test]
                fn reads_back_what_was_written() {
                    let got = store.get(id);
                    assert_eq!(got, Some(expected));
                }
            }
        "#;

        assert_eq!(rules(source), Vec::<Rule>::new());
    }

    #[test]
    fn accepts_weak_probe_when_a_strong_assert_is_present() {
        let source = r#"
            #[cfg(test)]
            mod tests {
                #[test]
                fn records_and_reads_back() {
                    let stored = store.record(&decision);
                    assert!(stored.is_ok());
                    assert_eq!(store.get(decision.id), Some(decision));
                }
            }
        "#;

        assert_eq!(rules(source), Vec::<Rule>::new());
    }

    #[test]
    fn accepts_assert_with_explicit_comparison_even_using_a_probe_value() {
        let source = r#"
            #[cfg(test)]
            mod tests {
                #[test]
                fn count_is_one() {
                    assert!(rows.len() == 1 && first.is_some());
                }
            }
        "#;

        assert_eq!(rules(source), Vec::<Rule>::new());
    }

    #[test]
    fn allow_marker_suppresses_vacuous_test() {
        let source = r#"
            #[cfg(test)]
            mod tests {
                // anemoi-guard:allow vacuous-test - intentional smoke test
                #[test]
                fn boots() {
                    assert!(start().is_ok());
                }
            }
        "#;

        assert_eq!(rules(source), Vec::<Rule>::new());
    }

    #[test]
    fn allow_marker_suppresses_pub_fn_in_test_module() {
        let source = r#"
            #[cfg(test)]
            mod tests {
                // anemoi-guard:allow pub-fn-in-test-module
                pub fn fixture() -> u32 { 1 }
            }
        "#;

        assert_eq!(rules(source), Vec::<Rule>::new());
    }

    #[test]
    fn reports_unparseable_source_as_error() {
        let result = analyze_source("fn broken( {");
        assert!(result.is_err());
    }
}
