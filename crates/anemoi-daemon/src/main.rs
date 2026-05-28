use anemoi_core::AnemoiConfig;
use anemoi_daemon::{serve, AppState};
use anemoi_telemetry::default_decision_log;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config_path =
        std::env::var("ANEMOI_CONFIG").unwrap_or_else(|_| "config/anemoi.example.yaml".to_string());
    let bind = std::env::var("ANEMOI_BIND").unwrap_or_else(|_| "127.0.0.1:7070".to_string());
    let decision_log_path = std::env::var("ANEMOI_DECISION_LOG").ok().map(Into::into);

    let config = AnemoiConfig::from_yaml_file(config_path)?;
    let decision_log = default_decision_log(decision_log_path)?;

    let state = AppState::new(config, decision_log)?;
    serve(bind.parse::<SocketAddr>()?, state).await
}
