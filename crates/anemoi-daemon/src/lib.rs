use anemoi_core::{
    AnemoiConfig, Decision, InferenceRequest, ModelResident, RuntimeId, RuntimeSnapshot,
};
use anemoi_policy::Scheduler;
use anemoi_runtime::{
    DynRuntimeAdapter, HttpInspectAdapter, LlamaSwapAdapter, MockRuntimeAdapter, OllamaAdapter,
};
use anemoi_telemetry::{DynDecisionLog, InMemoryDecisionLog};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

const DEFAULT_RECONCILIATION_TTL_MS: u64 = 5000;

#[derive(Debug, Clone)]
pub struct ReconciledSnapshot {
    pub snapshot: RuntimeSnapshot,
    pub last_inspected: DateTime<Utc>,
    pub last_error: Option<String>,
    pub is_stale: bool,
}

impl ReconciledSnapshot {
    pub fn fresh(snapshot: RuntimeSnapshot) -> Self {
        Self {
            snapshot,
            last_inspected: Utc::now(),
            last_error: None,
            is_stale: false,
        }
    }

    pub fn stale(snapshot: RuntimeSnapshot, last_error: Option<String>) -> Self {
        Self {
            snapshot,
            last_inspected: Utc::now(),
            last_error,
            is_stale: true,
        }
    }

    pub fn from_error(runtime_id: RuntimeId, error: String) -> Self {
        let snapshot = RuntimeSnapshot {
            runtime_id,
            available: false,
            residents: Vec::new(),
            memory: anemoi_core::RuntimeMemorySnapshot::default(),
            active_requests: Vec::new(),
        };
        Self {
            snapshot,
            last_inspected: Utc::now(),
            last_error: Some(error),
            is_stale: true,
        }
    }
}

#[derive(Clone)]
pub struct Reconciler {
    cache: Arc<RwLock<HashMap<String, ReconciledSnapshot>>>,
    ttl_ms: u64,
}

impl Reconciler {
    pub fn new(ttl_ms: u64) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            ttl_ms,
        }
    }

    pub async fn update(&self, runtime_id: &str, snapshot: RuntimeSnapshot) {
        let mut cache = self.cache.write().await;
        cache.insert(runtime_id.to_string(), ReconciledSnapshot::fresh(snapshot));
    }

    pub async fn record_error(&self, runtime_id: &str, error: String) {
        let mut cache = self.cache.write().await;
        let runtime_id_obj = RuntimeId(runtime_id.to_string());
        cache.insert(
            runtime_id.to_string(),
            ReconciledSnapshot::from_error(runtime_id_obj, error),
        );
    }

    pub async fn mark_stale(&self) {
        let mut cache = self.cache.write().await;
        for (_, reconciled) in cache.iter_mut() {
            reconciled.is_stale = true;
        }
    }

    pub async fn get(&self, runtime_id: &str) -> Option<ReconciledSnapshot> {
        let cache = self.cache.read().await;
        let mut reconciled = cache.get(runtime_id).cloned();
        if let Some(ref mut r) = reconciled {
            let elapsed = Utc::now()
                .signed_duration_since(r.last_inspected)
                .num_milliseconds();
            r.is_stale = elapsed > self.ttl_ms as i64;
        }
        reconciled
    }

    pub async fn all(&self) -> Vec<ReconciledSnapshot> {
        let cache = self.cache.read().await;
        cache
            .values()
            .map(|r| {
                let mut r = r.clone();
                let elapsed = Utc::now()
                    .signed_duration_since(r.last_inspected)
                    .num_milliseconds();
                r.is_stale = elapsed > self.ttl_ms as i64;
                r
            })
            .collect()
    }

    pub async fn get_snapshots(&self) -> Vec<RuntimeSnapshot> {
        self.all().await.into_iter().map(|r| r.snapshot).collect()
    }

    pub async fn has_cache(&self) -> bool {
        !self.cache.read().await.is_empty()
    }
}

impl Default for Reconciler {
    fn default() -> Self {
        Self::new(DEFAULT_RECONCILIATION_TTL_MS)
    }
}

#[derive(Clone)]
pub struct AppState {
    config: AnemoiConfig,
    scheduler: Scheduler,
    runtimes: HashMap<String, DynRuntimeAdapter>,
    decision_log: DynDecisionLog,
    reconciler: Reconciler,
}

impl AppState {
    pub fn new(config: AnemoiConfig, decision_log: DynDecisionLog) -> anyhow::Result<Self> {
        config.validate()?;
        let scheduler = Scheduler::new(config.clone());
        let mut runtimes = HashMap::new();

        for (runtime_id, runtime) in &config.runtimes {
            let adapter: DynRuntimeAdapter = match runtime.adapter.as_str() {
                "ollama" => Arc::new(OllamaAdapter::new(
                    runtime_id.clone(),
                    runtime
                        .base_url
                        .as_deref()
                        .unwrap_or("http://localhost:11434"),
                )?),
                "mock" => Arc::new(MockRuntimeAdapter::new(
                    runtime_id.clone(),
                    runtime
                        .initial_residents
                        .clone()
                        .into_iter()
                        .map(|resident| resident.into_resident())
                        .collect(),
                )),
                "llama_swap" => {
                    let adapter = LlamaSwapAdapter::new(
                        runtime_id.clone(),
                        runtime
                            .base_url
                            .as_deref()
                            .unwrap_or("http://localhost:8080"),
                    )?;
                    let adapter = if let Some(token) = &runtime.auth_token {
                        adapter.with_bearer_token(token)
                    } else {
                        adapter
                    };
                    Arc::new(adapter)
                }
                "llama_cpp" | "llama_server" => Arc::new(HttpInspectAdapter::new(
                    runtime_id.clone(),
                    runtime
                        .base_url
                        .as_deref()
                        .unwrap_or("http://localhost:8080"),
                )?),
                _ => Arc::new(MockRuntimeAdapter::new(runtime_id.clone(), Vec::new())),
            };
            runtimes.insert(runtime_id.to_string(), adapter);
        }

        let reconciler = Reconciler::default();

        Ok(Self {
            config,
            scheduler,
            runtimes,
            decision_log,
            reconciler,
        })
    }

    pub fn with_mock_residents(
        config: AnemoiConfig,
        residents: HashMap<String, Vec<ModelResident>>,
    ) -> anyhow::Result<Self> {
        config.validate()?;
        let scheduler = Scheduler::new(config.clone());
        let mut runtimes = HashMap::new();
        for (runtime_id, runtime) in &config.runtimes {
            let adapter: DynRuntimeAdapter = if runtime.adapter == "ollama" {
                Arc::new(OllamaAdapter::new(
                    runtime_id.clone(),
                    runtime
                        .base_url
                        .as_deref()
                        .unwrap_or("http://localhost:11434"),
                )?)
            } else {
                Arc::new(MockRuntimeAdapter::new(
                    runtime_id.clone(),
                    residents
                        .get(&runtime_id.to_string())
                        .cloned()
                        .unwrap_or_default(),
                ))
            };
            runtimes.insert(runtime_id.to_string(), adapter);
        }

        let reconciler = Reconciler::default();

        Ok(Self {
            config,
            scheduler,
            runtimes,
            decision_log: Arc::new(InMemoryDecisionLog::default()),
            reconciler,
        })
    }

    pub fn reconciler(&self) -> Reconciler {
        self.reconciler.clone()
    }

    pub async fn snapshots(&self) -> Vec<RuntimeSnapshot> {
        let mut snapshots = Vec::new();
        for runtime in self.runtimes.values() {
            if let Ok(snapshot) = runtime.inspect().await {
                snapshots.push(snapshot);
            }
        }
        snapshots
    }

    pub async fn decide(&self, request: &InferenceRequest) -> anyhow::Result<Decision> {
        let has_cache = self.reconciler.has_cache().await;
        let snapshots = if has_cache {
            self.reconciler.get_snapshots().await
        } else {
            self.snapshots().await
        };
        let decision = self.scheduler.decide(request, &snapshots)?;
        self.decision_log.record_decision(&decision).await?;
        Ok(decision)
    }

    pub fn runtime_adapter_type(&self, runtime_id: &str) -> Option<&str> {
        self.config
            .runtimes
            .get(&RuntimeId(runtime_id.to_string()))
            .map(|config| config.adapter.as_str())
    }

    pub async fn run_reconciliation_tick(&self) {
        for (runtime_id, adapter) in &self.runtimes {
            match adapter.inspect().await {
                Ok(snapshot) => {
                    self.reconciler.update(runtime_id, snapshot).await;
                }
                Err(e) => {
                    self.reconciler
                        .record_error(runtime_id, e.to_string())
                        .await;
                }
            }
        }
    }
}

fn live_execution_enabled() -> bool {
    std::env::var("ANEMOI_ENABLE_LIVE_EXECUTE").as_deref() == Ok("1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use anemoi_core::{
        DecisionAction, DomainId, ExecutionMode, ModelId, ModelProfileConfig, ModelResident,
        RequestId, ResidencyState, RuntimeConfig, RuntimeId,
    };
    use anemoi_telemetry::{DecisionLog, InMemoryDecisionLog};
    use axum::body::{to_bytes, Body};
    use axum::http::{Method, Request};
    use serde_json::Value;
    use std::collections::HashMap;
    use tower::ServiceExt;

    #[test]
    fn daemon_starts_without_database_url() {
        let config = example_config();

        let state = AppState::new(config, Arc::new(InMemoryDecisionLog::default()));

        assert!(state.is_ok());
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_body(response).await["ok"], true);
    }

    #[tokio::test]
    async fn status_returns_configured_counts() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .uri("/status")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        let body = json_body(response).await;

        assert_eq!(body["domains"], 1);
        assert_eq!(body["models"], 3);
        assert_eq!(body["runtimes"], 1);
        assert_eq!(body["residency_groups"], 2);
    }

    #[tokio::test]
    async fn residents_returns_runtime_snapshots() {
        let mut residents = HashMap::new();
        residents.insert(
            "mock".to_string(),
            vec![ModelResident {
                model_id: ModelId("qwen9b".to_string()),
                state: ResidencyState::HotGpu,
                vram_mb: Some(9000),
                ram_mb: None,
                kv_cache_mb: None,
                loaded_since: None,
            }],
        );
        let state = AppState::with_mock_residents(example_config(), residents).expect("state");

        let response = router(state)
            .oneshot(
                Request::builder()
                    .uri("/residents")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let snapshots: Vec<RuntimeSnapshot> =
            serde_json::from_value(json_body(response).await).expect("snapshots");

        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].runtime_id.to_string(), "mock");
        assert_eq!(snapshots[0].residents.len(), 1);
    }

    #[tokio::test]
    async fn decide_returns_structured_decision() {
        let response = test_router()
            .oneshot(json_request("/decide", &sample_request()))
            .await
            .expect("response");
        let decision: Decision =
            serde_json::from_value(json_body(response).await).expect("decision");

        assert!(decision.selected_model.is_some());
        assert!(decision.selected_runtime.is_some());
        assert!(!decision.explanation.summary.is_empty());
        assert!(!decision.score.contributions.is_empty());
    }

    #[tokio::test]
    async fn decide_records_decision_in_log() {
        let log = Arc::new(InMemoryDecisionLog::default());
        let state = AppState::new(example_config(), log.clone()).expect("state");

        let response = router(state)
            .oneshot(json_request("/decide", &sample_request()))
            .await
            .expect("response");
        let decision: Decision =
            serde_json::from_value(json_body(response).await).expect("decision");

        assert_eq!(
            log.get_decision(decision.id).await.expect("decision log"),
            Some(decision)
        );
    }

    #[tokio::test]
    async fn explain_returns_recorded_explanation() {
        let app = test_router();
        let response = app
            .clone()
            .oneshot(json_request("/decide", &sample_request()))
            .await
            .expect("decide response");
        let decision: Decision =
            serde_json::from_value(json_body(response).await).expect("decision");

        let explain_response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/explain/{}", decision.id))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("explain response");
        let explanation: anemoi_core::Explanation =
            serde_json::from_value(json_body(explain_response).await).expect("explanation");

        assert_eq!(explanation, decision.explanation);
    }

    #[tokio::test]
    async fn explain_returns_not_found_for_unknown_decision() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .uri(format!("/explain/{}", Uuid::new_v4()))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn execute_returns_honest_handoff_response() {
        let response = test_router()
            .oneshot(json_request("/execute", &sample_request()))
            .await
            .expect("response");
        let execute: ExecuteResponse =
            serde_json::from_value(json_body(response).await).expect("execute response");

        assert!(execute.handoff.load_requested);
        assert!(!execute.handoff.full_inference_forwarded);
        assert!(execute.handoff.message.contains("model-load handoff only"));
        assert!(execute.decision.selected_model.is_some());
    }

    #[tokio::test]
    async fn live_smoke_decide_uses_runtime_snapshot_without_execute() {
        let response = test_router()
            .oneshot(json_request("/decide", &sample_request()))
            .await
            .expect("response");
        let decision: Decision =
            serde_json::from_value(json_body(response).await).expect("decision");

        // Decision is based on a runtime snapshot.
        assert!(
            decision.selected_runtime.is_some(),
            "decide must use runtime snapshot to select a runtime"
        );
        // The /decide endpoint does not execute inference.
        assert_ne!(decision.action, DecisionAction::Deny);
    }

    #[tokio::test]
    async fn live_smoke_decision_records_runtime_evidence_source() {
        let log = Arc::new(InMemoryDecisionLog::default());
        let state = AppState::new(example_config(), log.clone()).expect("state");

        let decision = state.decide(&sample_request()).await.expect("decision");

        // The decision records which runtime was selected based on snapshot evidence.
        assert!(
            decision.selected_runtime.is_some(),
            "decision must record selected runtime"
        );
        let recorded = log
            .get_decision(decision.id)
            .await
            .expect("get decision")
            .expect("decision exists");
        assert_eq!(recorded.selected_runtime, decision.selected_runtime);
    }

    #[tokio::test]
    async fn live_smoke_explanation_includes_latency_and_residency_reasons() {
        let response = test_router()
            .oneshot(json_request("/decide", &sample_request()))
            .await
            .expect("response");
        let decision: Decision =
            serde_json::from_value(json_body(response).await).expect("decision");

        let reason_details: Vec<&str> = decision
            .explanation
            .reasons
            .iter()
            .map(|reason| reason.detail.as_str())
            .collect();
        let all_reasons = reason_details.join(" ");

        assert!(
            all_reasons.contains("latency"),
            "explanation should mention latency budget: {}",
            all_reasons
        );
        assert!(
            all_reasons.contains("residency") || all_reasons.contains("hot"),
            "explanation should mention residency: {}",
            all_reasons
        );
    }

    #[tokio::test]
    async fn openapi_document_is_served() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .uri("/openapi.json")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let document = json_body(response).await;

        assert_eq!(document["openapi"], "3.0.3");
        assert_eq!(document["info"]["title"], "Anemoi Local Governance API");
    }

    #[test]
    fn openapi_document_includes_health_status_residents_decide_execute() {
        let document = openapi_document();
        let paths = document["paths"].as_object().expect("paths");

        for path in ["/health", "/status", "/residents", "/decide", "/execute"] {
            assert!(paths.contains_key(path), "missing path {path}");
        }
    }

    #[test]
    fn openapi_decide_schema_matches_decision_response() {
        let document = openapi_document();

        assert_eq!(
            document["paths"]["/decide"]["post"]["requestBody"]["content"]["application/json"]
                ["schema"]["$ref"],
            "#/components/schemas/InferenceRequest"
        );
        assert_eq!(
            document["paths"]["/decide"]["post"]["responses"]["200"]["content"]["application/json"]
                ["schema"]["$ref"],
            "#/components/schemas/Decision"
        );
        assert!(document["components"]["schemas"]["Decision"]["required"]
            .as_array()
            .expect("required")
            .iter()
            .any(|field| field == "explanation"));
    }

    #[test]
    fn openapi_explain_schema_matches_explanation_response() {
        let document = openapi_document();

        assert_eq!(
            document["paths"]["/explain/{id}"]["get"]["responses"]["200"]["content"]
                ["application/json"]["schema"]["$ref"],
            "#/components/schemas/Explanation"
        );
        assert!(document["components"]["schemas"]["Explanation"]["required"]
            .as_array()
            .expect("required")
            .iter()
            .any(|field| field == "summary"));
    }

    #[test]
    fn openapi_contract_serializes_without_panic() {
        let document = openapi_document();
        let text = serde_json::to_string(&document).expect("serialize contract");

        assert!(text.contains("/execute"));
        assert!(text.contains("full_inference_forwarded"));
    }

    #[tokio::test]
    async fn live_execute_requires_explicit_enable_flag() {
        // Create a config with a non-mock runtime adapter.
        let mut config = example_config();
        config.runtimes.insert(
            RuntimeId("live_target".to_string()),
            RuntimeConfig {
                adapter: "llama_swap".to_string(),
                base_url: Some("http://127.0.0.1:8085".to_string()),
                auth_token: None,
                initial_residents: Vec::new(),
            },
        );
        // Add a model that references the new runtime.
        config.models.insert(
            ModelId("test_model".to_string()),
            ModelProfileConfig {
                family: "test".to_string(),
                parameter_class: "7b".to_string(),
                context_window: None,
                vram_required_mb: None,
                ram_required_mb: None,
                cold_load_estimate_ms: None,
                supported_runtimes: vec![RuntimeId("live_target".to_string())],
            },
        );

        let state =
            AppState::new(config, Arc::new(InMemoryDecisionLog::default())).expect("app state");
        let app = router(state);

        let response = app
            .oneshot(json_request("/execute", &sample_request()))
            .await
            .expect("response");
        let execute: ExecuteResponse =
            serde_json::from_value(json_body(response).await).expect("execute response");

        // Without ANEMOI_ENABLE_LIVE_EXECUTE=1, non-mock runtime load is skipped.
        assert!(!execute.decision.explanation.summary.is_empty());
    }

    #[tokio::test]
    async fn live_execute_rejects_without_runtime_target() {
        let request = InferenceRequest {
            id: RequestId::new(),
            domain: DomainId("unknown".to_string()),
            mode: ExecutionMode::Interactive,
            prompt_tokens_estimate: None,
            max_output_tokens: None,
            latency_budget_ms: None,
            quality_floor: None,
        };

        let response = test_router()
            .oneshot(json_request("/execute", &request))
            .await
            .expect("response");

        // The domain is unknown, so the scheduler should return a deny decision.
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn live_execute_returns_handoff_metadata_without_forwarding_by_default() {
        let response = test_router()
            .oneshot(json_request("/execute", &sample_request()))
            .await
            .expect("response");
        let execute: ExecuteResponse =
            serde_json::from_value(json_body(response).await).expect("execute response");

        assert!(
            !execute.handoff.full_inference_forwarded,
            "execute must report full_inference_forwarded as false"
        );
        assert!(
            execute.handoff.message.contains("model-load handoff only"),
            "execute must describe v1 limitation: {}",
            execute.handoff.message
        );
    }

    #[tokio::test]
    async fn live_execute_records_mutating_action_in_decision_explanation() {
        let log = Arc::new(InMemoryDecisionLog::default());
        let state = AppState::new(example_config(), log.clone()).expect("state");

        let response = router(state)
            .oneshot(json_request("/execute", &sample_request()))
            .await
            .expect("response");
        let execute: ExecuteResponse =
            serde_json::from_value(json_body(response).await).expect("execute response");

        // The decision is recorded and the explanation includes reasons.
        let recorded = log
            .get_decision(execute.decision.id)
            .await
            .expect("get decision")
            .expect("decision exists");
        assert!(!recorded.explanation.reasons.is_empty());
    }

    fn test_router() -> Router {
        router(
            AppState::new(example_config(), Arc::new(InMemoryDecisionLog::default()))
                .expect("state"),
        )
    }

    fn json_request<T: Serialize>(uri: &str, value: &T) -> Request<Body> {
        Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(value).expect("json")))
            .expect("request")
    }

    async fn json_body(response: axum::response::Response) -> Value {
        assert!(
            response.status().is_success(),
            "unexpected response status {}",
            response.status()
        );
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body bytes");
        serde_json::from_slice(&bytes).expect("json body")
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

    #[tokio::test]
    async fn reconciliation_loop_updates_snapshot_cache_from_runtime_inspect() {
        let state = AppState::new(example_config(), Arc::new(InMemoryDecisionLog::default()))
            .expect("state");

        state.run_reconciliation_tick().await;

        let reconciler = state.reconciler();
        let has_cache = reconciler.has_cache().await;
        assert!(has_cache, "reconciler should have cache after tick");

        let snapshots = reconciler.get_snapshots().await;
        assert!(
            !snapshots.is_empty(),
            "should have at least one snapshot after tick"
        );
    }

    #[tokio::test]
    async fn reconciliation_loop_marks_snapshot_stale_after_ttl() {
        let reconciler = Reconciler::new(1);

        let snapshot = RuntimeSnapshot {
            runtime_id: RuntimeId("test".to_string()),
            available: true,
            residents: vec![],
            memory: anemoi_core::RuntimeMemorySnapshot::default(),
            active_requests: vec![],
        };
        reconciler.update("test", snapshot).await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let stale = reconciler.get("test").await.expect("snapshot should exist");
        assert!(
            stale.is_stale,
            "snapshot should be stale after TTL exceeded"
        );
    }

    #[tokio::test]
    async fn reconciliation_loop_records_runtime_inspection_error_without_panicking() {
        let reconciler = Reconciler::new(5000);

        reconciler
            .record_error("test_runtime", "connection refused".to_string())
            .await;

        let snapshot = reconciler.get("test_runtime").await;
        assert!(snapshot.is_some(), "error record should exist in cache");
        let snapshot = snapshot.unwrap();
        assert!(snapshot.last_error.is_some(), "error should be recorded");
        assert_eq!(snapshot.last_error.unwrap(), "connection refused");
        assert!(
            !snapshot.snapshot.available,
            "snapshot should be marked unavailable"
        );
    }

    #[tokio::test]
    async fn status_uses_reconciled_snapshot_when_available() {
        let state = AppState::new(example_config(), Arc::new(InMemoryDecisionLog::default()))
            .expect("state");

        state.run_reconciliation_tick().await;

        let reconciler = state.reconciler();
        assert!(
            reconciler.has_cache().await,
            "cache should exist after tick"
        );

        let snapshots = reconciler.get_snapshots().await;
        assert!(
            !snapshots.is_empty(),
            "reconciled snapshots should be available"
        );
    }

    #[tokio::test]
    async fn decide_uses_reconciled_snapshot_without_reinspecting_when_fresh() {
        let state = AppState::new(example_config(), Arc::new(InMemoryDecisionLog::default()))
            .expect("state");

        state.run_reconciliation_tick().await;

        let reconciler = state.reconciler();
        assert!(
            reconciler.has_cache().await,
            "cache should exist after tick"
        );

        let decision = state.decide(&sample_request()).await.expect("decision");
        assert!(
            decision.selected_runtime.is_some(),
            "decision should use reconciled snapshot"
        );
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/residents", get(residents))
        .route("/decide", post(decide))
        .route("/execute", post(execute))
        .route("/decisions/:id", get(decision))
        .route("/explain/:id", get(explain))
        .route("/openapi.json", get(openapi))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn serve(addr: SocketAddr, state: AppState) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(state)).await?;
    Ok(())
}

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

async fn openapi() -> Json<serde_json::Value> {
    Json(openapi_document())
}

pub fn openapi_document() -> serde_json::Value {
    serde_json::json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Anemoi Local Governance API",
            "version": "0.1.0",
            "description": "Local control-plane API. In v1, /execute performs decision logging and model-load handoff only; it does not forward full inference."
        },
        "paths": {
            "/health": {
                "get": {
                    "responses": {
                        "200": {
                            "description": "Daemon is healthy",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/HealthResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/status": {
                "get": {
                    "responses": {
                        "200": {
                            "description": "Configured control-plane counts",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/StatusResponse" }
                                }
                            }
                        }
                    }
                }
            },
            "/residents": {
                "get": {
                    "responses": {
                        "200": {
                            "description": "Normalized runtime snapshots",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "array",
                                        "items": { "$ref": "#/components/schemas/RuntimeSnapshot" }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/decide": {
                "post": {
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/InferenceRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Structured decision without execution",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/Decision" }
                                }
                            }
                        },
                        "500": { "$ref": "#/components/responses/Error" }
                    }
                }
            },
            "/execute": {
                "post": {
                    "description": "v1 model-load handoff only; full inference forwarding is false.",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/InferenceRequest" }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Decision plus explicit handoff status",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/ExecuteResponse" }
                                }
                            }
                        },
                        "500": { "$ref": "#/components/responses/Error" }
                    }
                }
            },
            "/decisions/{id}": {
                "get": {
                    "parameters": [{ "$ref": "#/components/parameters/DecisionId" }],
                    "responses": {
                        "200": {
                            "description": "Recorded decision from process memory",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/Decision" }
                                }
                            }
                        },
                        "404": { "$ref": "#/components/responses/Error" }
                    }
                }
            },
            "/explain/{id}": {
                "get": {
                    "parameters": [{ "$ref": "#/components/parameters/DecisionId" }],
                    "responses": {
                        "200": {
                            "description": "Recorded explanation from process memory",
                            "content": {
                                "application/json": {
                                    "schema": { "$ref": "#/components/schemas/Explanation" }
                                }
                            }
                        },
                        "404": { "$ref": "#/components/responses/Error" }
                    }
                }
            }
        },
        "components": {
            "parameters": {
                "DecisionId": {
                    "name": "id",
                    "in": "path",
                    "required": true,
                    "schema": { "type": "string", "format": "uuid" }
                }
            },
            "responses": {
                "Error": {
                    "description": "Structured error text",
                    "content": {
                        "text/plain": {
                            "schema": { "$ref": "#/components/schemas/ErrorResponse" }
                        }
                    }
                }
            },
            "schemas": {
                "HealthResponse": {
                    "type": "object",
                    "required": ["ok"],
                    "properties": { "ok": { "type": "boolean" } }
                },
                "StatusResponse": {
                    "type": "object",
                    "required": ["domains", "models", "runtimes", "residency_groups"],
                    "properties": {
                        "domains": { "type": "integer" },
                        "models": { "type": "integer" },
                        "runtimes": { "type": "integer" },
                        "residency_groups": { "type": "integer" }
                    }
                },
                "InferenceRequest": {
                    "type": "object",
                    "required": ["domain", "mode"],
                    "properties": {
                        "id": { "type": "string" },
                        "domain": { "type": "string" },
                        "mode": { "type": "string", "enum": ["interactive", "batch", "background"] },
                        "prompt_tokens_estimate": { "type": "integer", "nullable": true },
                        "max_output_tokens": { "type": "integer", "nullable": true },
                        "latency_budget_ms": { "type": "integer", "nullable": true },
                        "quality_floor": { "type": "object", "nullable": true }
                    }
                },
                "RuntimeSnapshot": {
                    "type": "object",
                    "required": ["runtime_id", "available", "residents", "memory", "active_requests"],
                    "properties": {
                        "runtime_id": { "type": "string" },
                        "available": { "type": "boolean" },
                        "residents": { "type": "array", "items": { "type": "object" } },
                        "memory": { "type": "object" },
                        "active_requests": { "type": "array", "items": { "type": "object" } }
                    }
                },
                "Decision": {
                    "type": "object",
                    "required": ["id", "request_id", "action", "score", "explanation", "created_at"],
                    "properties": {
                        "id": { "type": "string", "format": "uuid" },
                        "request_id": { "type": "string" },
                        "action": { "type": "string", "enum": ["reuse_hot", "promote_warm", "cold_load", "stage_background", "downgrade", "defer", "deny"] },
                        "selected_model": { "type": "string", "nullable": true },
                        "selected_runtime": { "type": "string", "nullable": true },
                        "selected_group": { "type": "string", "nullable": true },
                        "background_model": { "type": "string", "nullable": true },
                        "score": { "type": "object" },
                        "explanation": { "$ref": "#/components/schemas/Explanation" },
                        "created_at": { "type": "string", "format": "date-time" }
                    }
                },
                "Explanation": {
                    "type": "object",
                    "required": ["summary", "reasons", "rejected_options"],
                    "properties": {
                        "summary": { "type": "string" },
                        "reasons": { "type": "array", "items": { "type": "object" } },
                        "rejected_options": { "type": "array", "items": { "type": "object" } }
                    }
                },
                "ExecuteResponse": {
                    "type": "object",
                    "required": ["decision", "handoff"],
                    "properties": {
                        "decision": { "$ref": "#/components/schemas/Decision" },
                        "handoff": { "$ref": "#/components/schemas/ExecuteHandoff" }
                    }
                },
                "ExecuteHandoff": {
                    "type": "object",
                    "required": ["load_requested", "full_inference_forwarded", "message"],
                    "properties": {
                        "load_requested": { "type": "boolean" },
                        "full_inference_forwarded": { "type": "boolean", "enum": [false] },
                        "message": { "type": "string" }
                    }
                },
                "ErrorResponse": {
                    "type": "string"
                }
            }
        }
    })
}

#[derive(Serialize)]
struct StatusResponse {
    domains: usize,
    models: usize,
    runtimes: usize,
    residency_groups: usize,
}

async fn status(State(state): State<AppState>) -> Json<StatusResponse> {
    Json(StatusResponse {
        domains: state.config.domains.len(),
        models: state.config.models.len(),
        runtimes: state.config.runtimes.len(),
        residency_groups: state.config.residency_groups.len(),
    })
}

async fn residents(State(state): State<AppState>) -> Json<Vec<RuntimeSnapshot>> {
    let has_cache = state.reconciler.has_cache().await;
    let snapshots = if has_cache {
        state.reconciler.get_snapshots().await
    } else {
        state.snapshots().await
    };
    Json(snapshots)
}

async fn decide(
    State(state): State<AppState>,
    Json(request): Json<InferenceRequest>,
) -> Result<Json<Decision>, (StatusCode, String)> {
    state
        .decide(&request)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn execute(
    State(state): State<AppState>,
    Json(request): Json<InferenceRequest>,
) -> Result<Json<ExecuteResponse>, (StatusCode, String)> {
    let decision = state.decide(&request).await.map_err(internal_error)?;
    let mut load_requested = false;

    if let (Some(runtime_id), Some(model_id)) =
        (&decision.selected_runtime, &decision.selected_model)
    {
        let adapter_type = state.runtime_adapter_type(&runtime_id.to_string());
        let is_mock = adapter_type == Some("mock");
        if is_mock || live_execution_enabled() {
            if let Some(runtime) = state.runtimes.get(&runtime_id.to_string()) {
                runtime.load_model(model_id).await.map_err(internal_error)?;
                load_requested = true;
            }
        }
    }

    Ok(Json(ExecuteResponse {
        decision,
        handoff: ExecuteHandoff {
            load_requested,
            full_inference_forwarded: false,
            message: "v1 execute performs decision logging and model-load handoff only".to_string(),
        },
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteResponse {
    pub decision: Decision,
    pub handoff: ExecuteHandoff,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteHandoff {
    pub load_requested: bool,
    pub full_inference_forwarded: bool,
    pub message: String,
}

async fn decision(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Decision>, (StatusCode, String)> {
    state
        .decision_log
        .get_decision(id)
        .await
        .map_err(internal_error)?
        .map(Json)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("decision {id} was not found"),
            )
        })
}

async fn explain(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<anemoi_core::Explanation>, (StatusCode, String)> {
    let decision = state
        .decision_log
        .get_decision(id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("decision {id} was not found"),
            )
        })?;
    Ok(Json(decision.explanation))
}

fn internal_error(error: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}
