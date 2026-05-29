use anemoi_core::{
    validate_config, AnemoiConfig, DiagnosticSeverity, DomainId, ExecutionMode, InferenceRequest,
    RequestId,
};
use anemoi_daemon::{AppState, OperatorStatus};
use anemoi_telemetry::default_decision_log;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(name = "anemoi")]
#[command(about = "Local inference governance control plane")]
struct Args {
    #[arg(long, default_value = "config/anemoi.example.yaml")]
    config: String,
    #[arg(long)]
    decision_log: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Status,
    Residents,
    Decide {
        #[arg(long)]
        domain: String,
        #[arg(long, default_value = "interactive")]
        mode: ModeArg,
        #[arg(long)]
        latency_budget_ms: Option<u64>,
    },
    Explain {
        decision_id: Uuid,
    },
    Runtimes,
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
    },
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum ModeArg {
    Interactive,
    Batch,
    Background,
}

impl From<ModeArg> for ExecutionMode {
    fn from(value: ModeArg) -> Self {
        match value {
            ModeArg::Interactive => Self::Interactive,
            ModeArg::Batch => Self::Batch,
            ModeArg::Background => Self::Background,
        }
    }
}

#[derive(Debug, Subcommand)]
enum PolicyCommand {
    Check,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    print!("{}", run(args).await?);
    Ok(())
}

async fn run(args: Args) -> anyhow::Result<String> {
    let config = AnemoiConfig::from_yaml_file(&args.config)?;

    if let Command::Policy {
        command: PolicyCommand::Check,
    } = &args.command
    {
        return Ok(format_policy_check(&config));
    }

    config.validate()?;
    let log = default_decision_log(args.decision_log)?;
    let state = AppState::new(config.clone(), log.clone())?;
    let mut output = String::new();

    match args.command {
        Command::Status => {
            // One reconciliation tick to populate the cache, then report purely
            // from the reconciled snapshot — no further live inspection.
            state.run_reconciliation_tick().await;
            output.push_str(&format_status(&state.operator_status().await));
        }
        Command::Residents => {
            let snapshots = state.snapshots().await;
            output.push_str(&serde_json::to_string_pretty(&snapshots)?);
            output.push('\n');
        }
        Command::Decide {
            domain,
            mode,
            latency_budget_ms,
        } => {
            let request = InferenceRequest {
                id: RequestId::new(),
                domain: DomainId(domain),
                mode: mode.into(),
                prompt_tokens_estimate: None,
                max_output_tokens: None,
                latency_budget_ms,
                quality_floor: None,
            };
            let decision = state.decide(&request).await?;
            output.push_str(&format_decision(&decision));
        }
        Command::Explain { decision_id } => {
            let Some(decision) = log.get_decision(decision_id).await? else {
                anyhow::bail!("decision {decision_id} was not found in recent in-memory decisions");
            };
            output.push_str(&serde_json::to_string_pretty(&decision.explanation)?);
            output.push('\n');
        }
        Command::Runtimes => {
            for (runtime_id, runtime) in config.runtimes {
                output.push_str(&format!("{runtime_id}: {}\n", runtime.adapter));
            }
        }
        Command::Policy { .. } => unreachable!("policy check handled before state construction"),
    }

    Ok(output)
}

fn format_policy_check(config: &AnemoiConfig) -> String {
    let diagnostics = validate_config(config);
    if diagnostics.is_empty() {
        return "Policy configuration loaded successfully.\n".to_string();
    }

    let mut output = String::from("Policy configuration has diagnostics:\n");
    for diagnostic in diagnostics {
        let severity = match diagnostic.severity {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
        };
        output.push_str(&format!(
            "- {severity} {}: {}\n",
            diagnostic.path, diagnostic.message
        ));
    }
    output
}

fn format_status(status: &OperatorStatus) -> String {
    let mut output = String::from("Anemoi operator status\n======================\n");

    output.push_str(&format!(
        "Live execution: {}\n",
        if status.live_execution_enabled {
            "enabled"
        } else {
            "disabled (ANEMOI_ENABLE_LIVE_EXECUTE not set)"
        }
    ));
    output.push_str(&format!(
        "Reconciliation cache: {}\n",
        if status.cache_populated {
            "populated"
        } else {
            "empty (governance state unknown until first inspection)"
        }
    ));
    output.push_str(&format!(
        "Active requests: {}\n",
        status.active_request_count
    ));
    output.push_str(&format!(
        "Recent decisions: {}\n",
        status.recent_decision_count
    ));

    output.push_str("\nRuntimes:\n");
    for runtime in &status.runtimes {
        output.push_str(&format!(
            "  {} ({}): availability={}, freshness={}\n",
            runtime.runtime_id, runtime.adapter, runtime.availability, runtime.freshness
        ));
        if let Some(error) = &runtime.last_error {
            output.push_str(&format!("    last error: {error}\n"));
        }
        output.push_str(&format!(
            "    active requests: {}\n",
            runtime.active_request_count
        ));
        if runtime.residents.is_empty() {
            output.push_str("    residents: none observed\n");
        } else {
            output.push_str("    residents:\n");
            for resident in &runtime.residents {
                let idle = resident
                    .idle_secs
                    .map(|s| format!("{s}s"))
                    .unwrap_or_else(|| "unknown".to_string());
                output.push_str(&format!(
                    "      - {} [{}] idle: {}\n",
                    resident.model_id,
                    format_residency_state(&resident.state),
                    idle
                ));
            }
        }
    }

    output.push_str("\nResidency groups:\n");
    for group in &status.residency_groups {
        let flags = match (group.keep_hot, group.pinned) {
            (true, true) => "keep-hot, pinned",
            (true, false) => "keep-hot",
            (false, true) => "pinned",
            (false, false) => "no protection",
        };
        output.push_str(&format!(
            "  {}: {} ({}, {}/{} hot)\n",
            group.group_id, group.health, flags, group.hot_resident_count, group.member_count
        ));
    }

    output.push_str(&format!(
        "\nStaging queue: total {} (blocked {}, pending {}, failed {}, completed {})\n",
        status.staging.total,
        status.staging.blocked,
        status.staging.pending,
        status.staging.failed,
        status.staging.completed
    ));

    output.push_str("\nPolicy warnings:\n");
    if status.policy_warnings.is_empty() {
        output.push_str("  none\n");
    } else {
        for warning in &status.policy_warnings {
            output.push_str(&format!("  - {warning}\n"));
        }
    }

    output
}

fn format_residency_state(state: &anemoi_core::ResidencyState) -> String {
    serde_json::to_value(state)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{state:?}"))
}

fn format_decision(decision: &anemoi_core::Decision) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "Selected: {} via {}",
        decision
            .selected_model
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "none".to_string()),
        decision
            .selected_runtime
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "none".to_string())
    ));
    output.push('\n');
    output.push_str(&format!("Action: {:?}\n", decision.action));
    output.push_str(&format!("Decision ID: {}\n\n", decision.id));
    output.push_str("Reasons:\n");
    for reason in &decision.explanation.reasons {
        output.push_str(&format!("- {}\n", reason.detail));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use anemoi_core::ModelId;
    use anemoi_telemetry::InMemoryDecisionLog;
    use std::sync::Arc;

    #[tokio::test]
    async fn cli_status_prints_residents_staging_and_policy_summary() {
        let output = run_cli(["anemoi", "--config", &example_config_path(), "status"]).await;

        // Residents observed from the reconciled cache.
        assert!(output.contains("mock"), "runtime should be listed");
        assert!(output.contains("qwen9b"), "resident model should be listed");
        assert!(
            output.contains("hot_gpu"),
            "resident state should be listed"
        );

        // Residency group health.
        assert!(output.contains("small_swarm"));
        assert!(output.contains("large_models"));

        // Staging queue summary.
        assert!(output.to_lowercase().contains("staging queue"));

        // Policy / governance summary.
        assert!(output.to_lowercase().contains("policy warnings"));
        assert!(output.to_lowercase().contains("live execution"));
    }

    #[tokio::test]
    async fn cli_status_marks_unknown_and_stale_state_plainly() {
        let config = AnemoiConfig::from_yaml_file(example_config_path()).expect("config");
        let state = AppState::new(config, Arc::new(InMemoryDecisionLog::default())).expect("state");

        state.run_reconciliation_tick().await;
        state.reconciler().mark_stale().await;

        let output = format_status(&state.operator_status().await);
        let lower = output.to_lowercase();

        // Aged snapshot is plainly labeled stale, not silently treated as fresh.
        assert!(lower.contains("stale"), "stale state must be labeled");
        // Mock residents have no known load time, so idle is plainly unknown.
        assert!(
            lower.contains("unknown"),
            "unknown state must be labeled, not omitted"
        );
    }

    #[tokio::test]
    async fn cli_policy_check_reports_valid_config() {
        let output = run_cli([
            "anemoi",
            "--config",
            &example_config_path(),
            "policy",
            "check",
        ])
        .await;

        assert_eq!(output, "Policy configuration loaded successfully.\n");
    }

    #[tokio::test]
    async fn cli_policy_check_reports_invalid_config_diagnostics() {
        let path = invalid_config_path();

        let output = run_cli(["anemoi", "--config", &path, "policy", "check"]).await;

        assert!(output.contains("Policy configuration has diagnostics"));
        assert!(output.contains("domains.coding.rosters[0]"));
        assert!(output.contains("unknown residency group 'missing_group'"));

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn cli_decide_prints_selected_model_and_action() {
        let output = run_cli([
            "anemoi",
            "--config",
            &example_config_path(),
            "decide",
            "--domain",
            "coding",
            "--latency-budget-ms",
            "1500",
        ])
        .await;

        assert!(output.contains("Selected:"));
        assert!(output.contains("Action:"));
    }

    #[tokio::test]
    async fn cli_decide_prints_explanation_reasons() {
        let output = run_cli([
            "anemoi",
            "--config",
            &example_config_path(),
            "decide",
            "--domain",
            "coding",
            "--latency-budget-ms",
            "1500",
        ])
        .await;

        assert!(output.contains("Reasons:"));
        assert!(output.contains("- "));
    }

    #[tokio::test]
    async fn cli_residents_prints_runtime_snapshots() {
        let output = run_cli(["anemoi", "--config", &example_config_path(), "residents"]).await;

        assert!(output.contains("\"runtime_id\""));
        assert!(output.contains("mock"));
    }

    #[tokio::test]
    async fn cli_runtimes_prints_configured_adapters() {
        let output = run_cli(["anemoi", "--config", &example_config_path(), "runtimes"]).await;

        assert_eq!(output, "mock: mock\n");
    }

    #[tokio::test]
    async fn cli_decide_works_without_database_url() {
        let config_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("config")
            .join("anemoi.example.yaml");
        let config = AnemoiConfig::from_yaml_file(config_path).expect("example config");
        let state = AppState::new(config, Arc::new(InMemoryDecisionLog::default()))
            .expect("app state without database");
        let request = InferenceRequest {
            id: RequestId::new(),
            domain: DomainId("coding".to_string()),
            mode: ExecutionMode::Interactive,
            prompt_tokens_estimate: None,
            max_output_tokens: None,
            latency_budget_ms: Some(1500),
            quality_floor: None,
        };

        let decision = state.decide(&request).await.expect("decision");

        assert_eq!(decision.selected_model, Some(ModelId("qwen9b".to_string())));
    }

    async fn run_cli<const N: usize>(args: [&str; N]) -> String {
        let args = Args::try_parse_from(args).expect("args");
        run(args).await.expect("cli output")
    }

    fn example_config_path() -> String {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("config")
            .join("anemoi.example.yaml")
            .to_string_lossy()
            .into_owned()
    }

    fn invalid_config_path() -> String {
        let path = std::env::temp_dir().join(format!("anemoi-invalid-{}.yaml", Uuid::new_v4()));
        std::fs::write(
            &path,
            r#"
domains:
  coding:
    rosters: [missing_group]
residency_groups: {}
models: {}
runtimes:
  mock:
    adapter: mock
"#,
        )
        .expect("write invalid config");
        path.to_string_lossy().into_owned()
    }
}
