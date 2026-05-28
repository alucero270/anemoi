use anemoi_core::{
    validate_config, AnemoiConfig, DiagnosticSeverity, DomainId, ExecutionMode, InferenceRequest,
    RequestId,
};
use anemoi_daemon::AppState;
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
            output.push_str(&format!("Domains: {}\n", config.domains.len()));
            output.push_str(&format!("Models: {}\n", config.models.len()));
            output.push_str(&format!("Runtimes: {}\n", config.runtimes.len()));
            output.push_str(&format!(
                "Residency groups: {}\n",
                config.residency_groups.len()
            ));
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
    use anemoi_telemetry::InMemoryDecisionLog;
    use std::sync::Arc;

    #[tokio::test]
    async fn cli_status_prints_configured_counts() {
        let output = run_cli(["anemoi", "--config", &example_config_path(), "status"]).await;

        assert!(output.contains("Domains: 1"));
        assert!(output.contains("Models: 3"));
        assert!(output.contains("Runtimes: 1"));
        assert!(output.contains("Residency groups: 2"));
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

        assert!(decision.selected_model.is_some());
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
