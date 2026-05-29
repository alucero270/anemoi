use anemoi_core::{
    validate_config, AnemoiConfig, ConfigDiagnostic, Decision, Explanation, InferenceRequest,
    RuntimeSnapshot,
};
use anemoi_daemon::AppState;
use anemoi_telemetry::{DynDecisionLog, InMemoryDecisionLog};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct McpService {
    config: AnemoiConfig,
    state: AppState,
    decision_log: DynDecisionLog,
}

impl McpService {
    pub fn new(config: AnemoiConfig) -> Result<Self, McpError> {
        let decision_log: DynDecisionLog = Arc::new(InMemoryDecisionLog::default());
        let state = AppState::new(config.clone(), decision_log.clone())?;
        Ok(Self {
            config,
            state,
            decision_log,
        })
    }

    pub fn tools(&self) -> Vec<ToolDescriptor> {
        vec![
            ToolDescriptor::new("get_status"),
            ToolDescriptor::new("list_residents"),
            ToolDescriptor::new("decide"),
            ToolDescriptor::new("explain_decision"),
            ToolDescriptor::new("check_policy"),
        ]
    }

    pub fn status(&self) -> McpStatus {
        McpStatus {
            domains: self.config.domains.len(),
            models: self.config.models.len(),
            runtimes: self.config.runtimes.len(),
            residency_groups: self.config.residency_groups.len(),
        }
    }

    pub async fn residents(&self) -> Vec<RuntimeSnapshot> {
        self.state.snapshots().await
    }

    pub async fn decide(&self, request: InferenceRequest) -> Result<Decision, McpError> {
        Ok(self.state.decide(&request).await?)
    }

    pub async fn explain_decision(&self, id: Uuid) -> Result<Explanation, McpError> {
        self.decision_log
            .get_decision(id)
            .await?
            .map(|decision| decision.explanation)
            .ok_or(McpError::DecisionNotFound(id))
    }

    pub fn check_policy(&self) -> Vec<ConfigDiagnostic> {
        validate_config(&self.config)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
}

impl ToolDescriptor {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpStatus {
    pub domains: usize,
    pub models: usize,
    pub runtimes: usize,
    pub residency_groups: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error(transparent)]
    App(#[from] anyhow::Error),
    #[error(transparent)]
    Telemetry(#[from] anemoi_telemetry::TelemetryError),
    #[error("decision {0} was not found")]
    DecisionNotFound(Uuid),
}

#[cfg(test)]
mod tests {
    use super::*;
    use anemoi_core::{DomainId, ExecutionMode, RequestId};

    #[test]
    fn mcp_lists_expected_tools() {
        let service = McpService::new(example_config()).expect("service");

        let tools = service
            .tools()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();

        assert_eq!(
            tools,
            vec![
                "get_status",
                "list_residents",
                "decide",
                "explain_decision",
                "check_policy",
            ]
        );
    }

    #[tokio::test]
    async fn mcp_decide_returns_same_decision_shape_as_http_api() {
        let service = McpService::new(example_config()).expect("service");

        let decision = service.decide(sample_request()).await.expect("decision");
        let json = serde_json::to_value(&decision).expect("decision json");

        let roundtrip: anemoi_core::Decision =
            serde_json::from_value(json).expect("decision deserializes from its json shape");
        assert_eq!(roundtrip, decision);
    }

    #[test]
    fn mcp_status_returns_runtime_and_policy_summary() {
        let service = McpService::new(example_config()).expect("service");

        assert_eq!(
            service.status(),
            McpStatus {
                domains: 1,
                models: 3,
                runtimes: 1,
                residency_groups: 2,
            }
        );
    }

    #[tokio::test]
    async fn mcp_residents_returns_normalized_snapshots() {
        let service = McpService::new(example_config()).expect("service");

        let snapshots = service.residents().await;

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].runtime_id.to_string(), "mock");
    }

    #[tokio::test]
    async fn mcp_explain_returns_recorded_explanation() {
        let service = McpService::new(example_config()).expect("service");
        let decision = service.decide(sample_request()).await.expect("decision");

        let explanation = service
            .explain_decision(decision.id)
            .await
            .expect("explanation");

        assert_eq!(explanation, decision.explanation);
    }

    #[tokio::test]
    async fn mcp_rejects_invalid_decide_request() {
        let service = McpService::new(example_config()).expect("service");
        let mut request = sample_request();
        request.domain = DomainId("missing".to_string());

        let error = service.decide(request).await.expect_err("invalid request");

        assert!(error.to_string().contains("unknown domain missing"));
    }

    fn sample_request() -> InferenceRequest {
        InferenceRequest {
            id: RequestId::new(),
            domain: DomainId("coding".to_string()),
            mode: ExecutionMode::Interactive,
            prompt_tokens_estimate: Some(1000),
            max_output_tokens: Some(500),
            latency_budget_ms: Some(1500),
            quality_floor: None,
        }
    }

    fn example_config() -> AnemoiConfig {
        let config_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("config")
            .join("anemoi.example.yaml");
        AnemoiConfig::from_yaml_file(config_path).expect("example config")
    }
}
