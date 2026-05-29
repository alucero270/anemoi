//! Walks Rust sources under the given root (default: current directory) and
//! reports anemoi-guard violations. Exits non-zero when any are found so CI
//! fails the build.

use std::path::Path;
use std::process::ExitCode;

use anemoi_guard::analyze_source;
use walkdir::WalkDir;

fn main() -> ExitCode {
    let root = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let mut total = 0usize;
    let mut scanned = 0usize;

    for entry in WalkDir::new(&root)
        .into_iter()
        .filter_entry(|entry| entry.file_name() != "target")
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }

        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(err) => {
                eprintln!("anemoi-guard: cannot read {}: {err}", path.display());
                return ExitCode::FAILURE;
            }
        };

        scanned += 1;
        match analyze_source(&source) {
            Ok(violations) => {
                for violation in &violations {
                    total += 1;
                    report(path, violation);
                }
            }
            Err(err) => {
                eprintln!("anemoi-guard: cannot parse {}: {err}", path.display());
                return ExitCode::FAILURE;
            }
        }
    }

    if total == 0 {
        println!("anemoi-guard: {scanned} files scanned, no violations");
        ExitCode::SUCCESS
    } else {
        eprintln!("anemoi-guard: {total} violation(s) across {scanned} files");
        eprintln!("suppress an intentional case with a comment containing `anemoi-guard:allow`");
        ExitCode::FAILURE
    }
}

fn report(path: &Path, violation: &anemoi_guard::Violation) {
    eprintln!(
        "{}:{} [{}] {}",
        path.display(),
        violation.line,
        violation.rule.code(),
        violation.message
    );
}
