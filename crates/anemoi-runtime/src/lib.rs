use anemoi_core::{
    ActiveExecution, ExecutionRequest, ModelId, ModelResident, ResidencyState, RuntimeId,
    RuntimeMemorySnapshot, RuntimeSnapshot,
};
use async_trait::async_trait;
use chrono::Utc;
use futures::stream::{BoxStream, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::Url;
use serde::Deserialize;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("runtime {0} is unavailable")]
    Unavailable(RuntimeId),
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("invalid runtime url: {0}")]
    Url(String),
    #[error("runtime operation is not supported: {0}")]
    Unsupported(&'static str),
}

#[derive(Debug, Clone)]
pub struct LoadHandle {
    pub id: Uuid,
    pub model_id: ModelId,
}

#[derive(Debug, Clone)]
pub struct ExecutionHandle {
    pub id: Uuid,
    pub request: ExecutionRequest,
}

#[async_trait]
pub trait RuntimeAdapter: Send + Sync {
    fn id(&self) -> RuntimeId;

    async fn inspect(&self) -> Result<RuntimeSnapshot, RuntimeError>;

    async fn load_model(&self, model: &ModelId) -> Result<LoadHandle, RuntimeError>;

    async fn unload_model(&self, model: &ModelId) -> Result<(), RuntimeError>;

    async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionHandle, RuntimeError>;
}

pub type DynRuntimeAdapter = Arc<dyn RuntimeAdapter>;

#[derive(Debug, Clone)]
pub struct MockRuntimeAdapter {
    id: RuntimeId,
    snapshot: Arc<RwLock<RuntimeSnapshot>>,
}

impl MockRuntimeAdapter {
    pub fn new(id: RuntimeId, residents: Vec<ModelResident>) -> Self {
        Self {
            id: id.clone(),
            snapshot: Arc::new(RwLock::new(RuntimeSnapshot {
                runtime_id: id,
                available: true,
                residents,
                configured_models: Vec::new(),
                memory: RuntimeMemorySnapshot::default(),
                active_requests: Vec::new(),
            })),
        }
    }

    pub fn with_memory(self, memory: RuntimeMemorySnapshot) -> Self {
        self.snapshot.write().expect("mock runtime lock").memory = memory;
        self
    }

    pub fn with_available(self, available: bool) -> Self {
        self.snapshot.write().expect("mock runtime lock").available = available;
        self
    }

    pub fn with_resident_state(self, model_id: ModelId, state: ResidencyState) -> Self {
        let mut snapshot = self.snapshot.write().expect("mock runtime lock");
        if let Some(resident) = snapshot
            .residents
            .iter_mut()
            .find(|resident| resident.model_id == model_id)
        {
            resident.state = state;
        } else {
            snapshot.residents.push(ModelResident {
                model_id,
                state,
                vram_mb: None,
                ram_mb: None,
                kv_cache_mb: None,
                loaded_since: None,
            });
        }
        drop(snapshot);
        self
    }
}

#[async_trait]
impl RuntimeAdapter for MockRuntimeAdapter {
    fn id(&self) -> RuntimeId {
        self.id.clone()
    }

    async fn inspect(&self) -> Result<RuntimeSnapshot, RuntimeError> {
        Ok(self.snapshot.read().expect("mock runtime lock").clone())
    }

    async fn load_model(&self, model: &ModelId) -> Result<LoadHandle, RuntimeError> {
        let mut snapshot = self.snapshot.write().expect("mock runtime lock");
        if !snapshot
            .residents
            .iter()
            .any(|resident| resident.model_id == *model)
        {
            snapshot.residents.push(ModelResident {
                model_id: model.clone(),
                state: ResidencyState::Loading,
                vram_mb: None,
                ram_mb: None,
                kv_cache_mb: None,
                loaded_since: None,
            });
        }

        Ok(LoadHandle {
            id: Uuid::new_v4(),
            model_id: model.clone(),
        })
    }

    async fn unload_model(&self, model: &ModelId) -> Result<(), RuntimeError> {
        let mut snapshot = self.snapshot.write().expect("mock runtime lock");
        snapshot
            .residents
            .retain(|resident| resident.model_id != *model);
        Ok(())
    }

    async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionHandle, RuntimeError> {
        let mut snapshot = self.snapshot.write().expect("mock runtime lock");
        snapshot.active_requests.push(ActiveExecution {
            request_id: request.request_id.clone(),
            model_id: request.model_id.clone(),
            started_at: Utc::now(),
        });

        Ok(ExecutionHandle {
            id: Uuid::new_v4(),
            request,
        })
    }
}

#[derive(Debug, Clone)]
pub struct OllamaAdapter {
    id: RuntimeId,
    base_url: Url,
    client: reqwest::Client,
}

impl OllamaAdapter {
    pub fn new(id: RuntimeId, base_url: &str) -> Result<Self, RuntimeError> {
        Ok(Self {
            id,
            base_url: Url::parse(base_url).map_err(|error| RuntimeError::Url(error.to_string()))?,
            client: reqwest::Client::new(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct OllamaPsResponse {
    #[serde(default)]
    models: Vec<OllamaRunningModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaRunningModel {
    name: String,
    #[serde(default)]
    size_vram: Option<u64>,
    #[serde(default)]
    size: Option<u64>,
}

#[async_trait]
impl RuntimeAdapter for OllamaAdapter {
    fn id(&self) -> RuntimeId {
        self.id.clone()
    }

    async fn inspect(&self) -> Result<RuntimeSnapshot, RuntimeError> {
        let url = self
            .base_url
            .join("/api/ps")
            .map_err(|error| RuntimeError::Url(error.to_string()))?;
        let http_response = self.client.get(url).send().await?;
        if !http_response.status().is_success() {
            return Ok(RuntimeSnapshot {
                runtime_id: self.id.clone(),
                available: false,
                residents: Vec::new(),
                configured_models: Vec::new(),
                memory: RuntimeMemorySnapshot::default(),
                active_requests: Vec::new(),
            });
        }

        let response: OllamaPsResponse = http_response.json().await?;
        let residents = response
            .models
            .into_iter()
            .map(|model| ModelResident {
                model_id: ModelId(model.name),
                state: ResidencyState::HotGpu,
                vram_mb: model.size_vram.map(bytes_to_mb),
                ram_mb: model.size.map(bytes_to_mb),
                kv_cache_mb: None,
                loaded_since: None,
            })
            .collect();

        Ok(RuntimeSnapshot {
            runtime_id: self.id.clone(),
            available: true,
            residents,
            configured_models: Vec::new(),
            memory: RuntimeMemorySnapshot::default(),
            active_requests: Vec::new(),
        })
    }

    async fn load_model(&self, model: &ModelId) -> Result<LoadHandle, RuntimeError> {
        Ok(LoadHandle {
            id: Uuid::new_v4(),
            model_id: model.clone(),
        })
    }

    async fn unload_model(&self, _model: &ModelId) -> Result<(), RuntimeError> {
        Err(RuntimeError::Unsupported("ollama unload"))
    }

    async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionHandle, RuntimeError> {
        Ok(ExecutionHandle {
            id: Uuid::new_v4(),
            request,
        })
    }
}

/// Inspect-only adapter for a llama.cpp / `llama-server` instance.
///
/// llama.cpp exposes an OpenAI-compatible `/v1/models` endpoint and a
/// `/health` endpoint. Per the residency truth contract
/// (`docs/live_validation/residency-truth-contract.md`), `/v1/models` proves
/// configuration, not residency — so `inspect()` surfaces those ids in
/// `configured_models` and leaves `residents` empty. This adapter never
/// loads, unloads, or executes.
#[derive(Debug, Clone)]
pub struct LlamaCppAdapter {
    id: RuntimeId,
    base_url: Url,
    client: reqwest::Client,
    auth_token: Option<String>,
}

impl LlamaCppAdapter {
    pub fn new(id: RuntimeId, base_url: &str) -> Result<Self, RuntimeError> {
        Self::new_with_timeout(id, base_url, Duration::from_secs(5))
    }

    pub fn new_with_timeout(
        id: RuntimeId,
        base_url: &str,
        timeout: Duration,
    ) -> Result<Self, RuntimeError> {
        Ok(Self {
            id,
            base_url: Url::parse(base_url).map_err(|error| RuntimeError::Url(error.to_string()))?,
            client: reqwest::Client::builder().timeout(timeout).build()?,
            auth_token: None,
        })
    }

    /// Configure a bearer token. The token must originate from the environment
    /// (expanded at config load); it is never committed.
    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// Returns the configured model ids reported by `/v1/models`. This is
    /// configuration evidence, not residency evidence.
    pub async fn inspect_models(&self) -> Result<Vec<ModelId>, RuntimeError> {
        let url = self
            .base_url
            .join("/v1/models")
            .map_err(|error| RuntimeError::Url(error.to_string()))?;
        let response: LlamaCppModelsResponse = self
            .client
            .get(url)
            .headers(self.headers()?)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(response
            .data
            .into_iter()
            .map(|model| normalize_model_id(&model.id))
            .collect())
    }

    fn headers(&self) -> Result<HeaderMap, RuntimeError> {
        let mut headers = HeaderMap::new();
        if let Some(token) = &self.auth_token {
            let value = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|error| RuntimeError::Url(error.to_string()))?;
            headers.insert(AUTHORIZATION, value);
        }
        Ok(headers)
    }
}

#[derive(Debug, Deserialize)]
struct LlamaCppModelsResponse {
    #[serde(default)]
    data: Vec<LlamaCppModel>,
}

#[derive(Debug, Deserialize)]
struct LlamaCppModel {
    id: String,
}

#[async_trait]
impl RuntimeAdapter for LlamaCppAdapter {
    fn id(&self) -> RuntimeId {
        self.id.clone()
    }

    async fn inspect(&self) -> Result<RuntimeSnapshot, RuntimeError> {
        let health_url = self
            .base_url
            .join("/health")
            .map_err(|error| RuntimeError::Url(error.to_string()))?;
        let health = self
            .client
            .get(health_url)
            .headers(self.headers()?)
            .send()
            .await?;

        if !health.status().is_success() {
            return Ok(RuntimeSnapshot {
                runtime_id: self.id.clone(),
                available: false,
                residents: Vec::new(),
                configured_models: Vec::new(),
                memory: RuntimeMemorySnapshot::default(),
                active_requests: Vec::new(),
            });
        }

        let configured_models = self.inspect_models().await?;

        Ok(RuntimeSnapshot {
            runtime_id: self.id.clone(),
            available: true,
            // /v1/models proves configuration, not residency — see
            // docs/live_validation/residency-truth-contract.md. residents
            // stays empty until we have evidence the model is loaded.
            residents: Vec::new(),
            configured_models,
            memory: RuntimeMemorySnapshot::default(),
            active_requests: Vec::new(),
        })
    }

    async fn load_model(&self, _model: &ModelId) -> Result<LoadHandle, RuntimeError> {
        Err(RuntimeError::Unsupported("llama-cpp load"))
    }

    async fn unload_model(&self, _model: &ModelId) -> Result<(), RuntimeError> {
        Err(RuntimeError::Unsupported("llama-cpp unload"))
    }

    async fn execute(&self, _request: ExecutionRequest) -> Result<ExecutionHandle, RuntimeError> {
        Err(RuntimeError::Unsupported("llama-cpp execute"))
    }
}

#[derive(Debug, Clone)]
pub struct LlamaSwapAdapter {
    id: RuntimeId,
    base_url: Url,
    client: reqwest::Client,
    auth_token: Option<String>,
}

impl LlamaSwapAdapter {
    pub fn new(id: RuntimeId, base_url: &str) -> Result<Self, RuntimeError> {
        Self::new_with_timeout(id, base_url, Duration::from_secs(5))
    }

    pub fn new_with_timeout(
        id: RuntimeId,
        base_url: &str,
        timeout: Duration,
    ) -> Result<Self, RuntimeError> {
        Ok(Self {
            id,
            base_url: Url::parse(base_url).map_err(|error| RuntimeError::Url(error.to_string()))?,
            client: reqwest::Client::builder().timeout(timeout).build()?,
            auth_token: None,
        })
    }

    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    pub async fn inspect_models(&self) -> Result<Vec<ModelId>, RuntimeError> {
        let url = self
            .base_url
            .join("/v1/models")
            .map_err(|error| RuntimeError::Url(error.to_string()))?;
        let response: LlamaSwapModelsResponse = self
            .client
            .get(url)
            .headers(self.headers()?)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(response
            .data
            .into_iter()
            .map(|model| normalize_model_id(&model.id))
            .collect())
    }

    fn headers(&self) -> Result<HeaderMap, RuntimeError> {
        let mut headers = HeaderMap::new();
        if let Some(token) = &self.auth_token {
            let value = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|error| RuntimeError::Url(error.to_string()))?;
            headers.insert(AUTHORIZATION, value);
        }
        Ok(headers)
    }
}

#[derive(Debug, Deserialize)]
struct LlamaSwapModelsResponse {
    #[serde(default)]
    data: Vec<LlamaSwapModel>,
}

#[derive(Debug, Deserialize)]
struct LlamaSwapModel {
    id: String,
}

#[async_trait]
impl RuntimeAdapter for LlamaSwapAdapter {
    fn id(&self) -> RuntimeId {
        self.id.clone()
    }

    async fn inspect(&self) -> Result<RuntimeSnapshot, RuntimeError> {
        let health_url = self
            .base_url
            .join("/health")
            .map_err(|error| RuntimeError::Url(error.to_string()))?;
        let health = self
            .client
            .get(health_url)
            .headers(self.headers()?)
            .send()
            .await?;

        if !health.status().is_success() {
            return Ok(RuntimeSnapshot {
                runtime_id: self.id.clone(),
                available: false,
                residents: Vec::new(),
                configured_models: Vec::new(),
                memory: RuntimeMemorySnapshot::default(),
                active_requests: Vec::new(),
            });
        }

        let configured_models = self.inspect_models().await?;

        Ok(RuntimeSnapshot {
            runtime_id: self.id.clone(),
            available: true,
            // /v1/models proves configuration, not residency — see
            // docs/live_validation/residency-truth-contract.md. residents stays
            // empty until we have evidence the model is loaded.
            residents: Vec::new(),
            configured_models,
            memory: RuntimeMemorySnapshot::default(),
            active_requests: Vec::new(),
        })
    }

    async fn load_model(&self, model: &ModelId) -> Result<LoadHandle, RuntimeError> {
        let url = self
            .base_url
            .join("/v1/chat/completions")
            .map_err(|error| RuntimeError::Url(error.to_string()))?;
        let body = serde_json::json!({
            "model": model.to_string(),
            "messages": [{"role": "user", "content": "ping"}],
            "max_tokens": 1,
        });
        self.client
            .post(url)
            .headers(self.headers()?)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(LoadHandle {
            id: Uuid::new_v4(),
            model_id: model.clone(),
        })
    }

    async fn unload_model(&self, _model: &ModelId) -> Result<(), RuntimeError> {
        Err(RuntimeError::Unsupported("llama-swap unload"))
    }

    async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionHandle, RuntimeError> {
        Ok(ExecutionHandle {
            id: Uuid::new_v4(),
            request,
        })
    }
}

#[derive(Debug, Clone)]
pub struct HttpInspectAdapter {
    id: RuntimeId,
    base_url: Url,
    client: reqwest::Client,
}

impl HttpInspectAdapter {
    pub fn new(id: RuntimeId, base_url: &str) -> Result<Self, RuntimeError> {
        Ok(Self {
            id,
            base_url: Url::parse(base_url).map_err(|error| RuntimeError::Url(error.to_string()))?,
            client: reqwest::Client::new(),
        })
    }
}

#[async_trait]
impl RuntimeAdapter for HttpInspectAdapter {
    fn id(&self) -> RuntimeId {
        self.id.clone()
    }

    async fn inspect(&self) -> Result<RuntimeSnapshot, RuntimeError> {
        let health_url = self
            .base_url
            .join("/")
            .map_err(|error| RuntimeError::Url(error.to_string()))?;
        let available = self.client.get(health_url).send().await.is_ok();
        Ok(RuntimeSnapshot {
            runtime_id: self.id.clone(),
            available,
            residents: Vec::new(),
            configured_models: Vec::new(),
            memory: RuntimeMemorySnapshot::default(),
            active_requests: Vec::new(),
        })
    }

    async fn load_model(&self, model: &ModelId) -> Result<LoadHandle, RuntimeError> {
        Ok(LoadHandle {
            id: Uuid::new_v4(),
            model_id: model.clone(),
        })
    }

    async fn unload_model(&self, _model: &ModelId) -> Result<(), RuntimeError> {
        Err(RuntimeError::Unsupported("http unload"))
    }

    async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionHandle, RuntimeError> {
        Ok(ExecutionHandle {
            id: Uuid::new_v4(),
            request,
        })
    }
}

fn bytes_to_mb(bytes: u64) -> u64 {
    bytes / 1024 / 1024
}

fn normalize_model_id(raw: &str) -> ModelId {
    let leaf = raw.replace('\\', "/");
    let leaf = leaf.rsplit('/').next().unwrap_or(raw);
    ModelId(
        leaf.strip_suffix(".gguf")
            .or_else(|| leaf.strip_suffix(".bin"))
            .unwrap_or(leaf)
            .to_string(),
    )
}

/// Where a forwarded chat completion should go. `auth_token` originates from
/// the environment (expanded at config load) and is never committed.
#[derive(Debug, Clone)]
pub struct ForwardTarget {
    pub base_url: String,
    pub auth_token: Option<String>,
}

/// A chat-completion response forwarded back from a runtime. The body is a
/// stream so the caller can relay it without buffering (the inference gateway
/// requirement).
pub struct ForwardedChatCompletion {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: BoxStream<'static, Result<Vec<u8>, RuntimeError>>,
}

const FORWARD_TIMEOUT: Duration = Duration::from_secs(120);

/// Forwards an OpenAI-style chat completion `body` to a runtime's
/// `/v1/chat/completions` endpoint, injecting the runtime bearer token when one
/// is configured. The response body is streamed straight through; only
/// transport failures (connection refused, timeout) surface as an `Err`.
pub async fn forward_chat_completion(
    target: &ForwardTarget,
    body: &serde_json::Value,
) -> Result<ForwardedChatCompletion, RuntimeError> {
    let base =
        Url::parse(&target.base_url).map_err(|error| RuntimeError::Url(error.to_string()))?;
    let url = base
        .join("/v1/chat/completions")
        .map_err(|error| RuntimeError::Url(error.to_string()))?;

    let client = reqwest::Client::builder()
        .timeout(FORWARD_TIMEOUT)
        .build()?;
    let mut request = client.post(url).json(body);
    if let Some(token) = &target.auth_token {
        request = request.bearer_auth(token);
    }

    let response = request.send().await?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body = response
        .bytes_stream()
        .map(|chunk| {
            chunk
                .map(|bytes| bytes.to_vec())
                .map_err(RuntimeError::from)
        })
        .boxed();

    Ok(ForwardedChatCompletion {
        status,
        content_type,
        body,
    })
}

/// Builds a deterministic OpenAI-style SSE response for the mock runtime. The
/// payload echoes `selected_model`, so a test can confirm the gateway rewrote
/// the `model` field to the decision's selected model. No network is involved.
pub fn mock_chat_completion(selected_model: &str) -> ForwardedChatCompletion {
    let chunk = format!(
        "data: {{\"object\":\"chat.completion.chunk\",\"model\":\"{selected_model}\",\
\"choices\":[{{\"index\":0,\"delta\":{{\"role\":\"assistant\",\"content\":\"mock response from {selected_model}\"}}}}]}}\n\n"
    );
    let chunks: Vec<Result<Vec<u8>, RuntimeError>> =
        vec![Ok(chunk.into_bytes()), Ok(b"data: [DONE]\n\n".to_vec())];
    ForwardedChatCompletion {
        status: 200,
        content_type: Some("text/event-stream".to_string()),
        body: futures::stream::iter(chunks).boxed(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anemoi_core::{RequestId, RuntimeMemorySnapshot};
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn adapter_id_is_stable() {
        let adapter = MockRuntimeAdapter::new(RuntimeId("mock".to_string()), Vec::new());

        assert_eq!(adapter.id(), RuntimeId("mock".to_string()));
        assert_eq!(adapter.id(), RuntimeId("mock".to_string()));
    }

    #[tokio::test]
    async fn inspect_returns_normalized_runtime_snapshot() {
        let adapter = MockRuntimeAdapter::new(
            RuntimeId("mock".to_string()),
            vec![ModelResident {
                model_id: ModelId("qwen9b".to_string()),
                state: ResidencyState::HotGpu,
                vram_mb: Some(9000),
                ram_mb: Some(12000),
                kv_cache_mb: Some(512),
                loaded_since: None,
            }],
        )
        .with_memory(RuntimeMemorySnapshot {
            vram_total_mb: Some(24_000),
            vram_used_mb: Some(9_000),
            ram_total_mb: Some(64_000),
            ram_used_mb: Some(12_000),
        });

        let snapshot = adapter.inspect().await.expect("snapshot");

        assert_eq!(snapshot.runtime_id, RuntimeId("mock".to_string()));
        assert!(snapshot.available);
        assert_eq!(snapshot.residents.len(), 1);
        assert_eq!(
            snapshot.residents[0].model_id,
            ModelId("qwen9b".to_string())
        );
        assert_eq!(snapshot.residents[0].state, ResidencyState::HotGpu);
        assert_eq!(snapshot.memory.pressure_percent(), Some(37));
    }

    #[tokio::test]
    async fn load_model_returns_model_load_handle() {
        let adapter = MockRuntimeAdapter::new(RuntimeId("mock".to_string()), Vec::new());
        let model_id = ModelId("qwen9b".to_string());

        let handle = adapter.load_model(&model_id).await.expect("load handle");
        let snapshot = adapter.inspect().await.expect("snapshot");

        assert_eq!(handle.model_id, model_id);
        assert!(snapshot.residents.iter().any(|resident| {
            resident.model_id == ModelId("qwen9b".to_string())
                && resident.state == ResidencyState::Loading
        }));
    }

    #[tokio::test]
    async fn execute_returns_execution_handle() {
        let adapter = MockRuntimeAdapter::new(RuntimeId("mock".to_string()), Vec::new());
        let request = ExecutionRequest {
            request_id: RequestId::new(),
            model_id: ModelId("qwen9b".to_string()),
            prompt: Some("hello".to_string()),
        };

        let handle = adapter
            .execute(request.clone())
            .await
            .expect("execute handle");
        let snapshot = adapter.inspect().await.expect("snapshot");

        assert_eq!(handle.request, request);
        assert_eq!(snapshot.active_requests.len(), 1);
        assert_eq!(snapshot.active_requests[0].request_id, request.request_id);
        assert_eq!(snapshot.active_requests[0].model_id, request.model_id);
    }

    #[tokio::test]
    async fn unsupported_unload_returns_runtime_error() {
        let adapter =
            HttpInspectAdapter::new(RuntimeId("llama_swap".to_string()), "http://localhost:8080")
                .expect("adapter");

        let error = adapter
            .unload_model(&ModelId("qwen9b".to_string()))
            .await
            .expect_err("unsupported unload");

        assert_eq!(
            error.to_string(),
            "runtime operation is not supported: http unload"
        );
    }

    #[test]
    fn runtime_errors_are_human_readable() {
        let unavailable = RuntimeError::Unavailable(RuntimeId("ollama".to_string()));
        let unsupported = RuntimeError::Unsupported("ollama unload");
        let url = RuntimeError::Url("relative URL without a base".to_string());

        assert_eq!(unavailable.to_string(), "runtime ollama is unavailable");
        assert_eq!(
            unsupported.to_string(),
            "runtime operation is not supported: ollama unload"
        );
        assert_eq!(
            url.to_string(),
            "invalid runtime url: relative URL without a base"
        );
    }

    #[tokio::test]
    async fn mock_runtime_starts_with_configured_residents() {
        let adapter = MockRuntimeAdapter::new(
            RuntimeId("mock".to_string()),
            vec![ModelResident {
                model_id: ModelId("qwen9b".to_string()),
                state: ResidencyState::HotGpu,
                vram_mb: Some(9000),
                ram_mb: None,
                kv_cache_mb: None,
                loaded_since: None,
            }],
        );

        let snapshot = adapter.inspect().await.expect("snapshot");

        assert_eq!(snapshot.residents.len(), 1);
        assert_eq!(
            snapshot.residents[0].model_id,
            ModelId("qwen9b".to_string())
        );
        assert_eq!(snapshot.residents[0].state, ResidencyState::HotGpu);
    }

    #[tokio::test]
    async fn mock_runtime_load_adds_loading_resident_once() {
        let adapter = MockRuntimeAdapter::new(RuntimeId("mock".to_string()), Vec::new());
        let model_id = ModelId("qwen9b".to_string());

        adapter.load_model(&model_id).await.expect("first load");
        adapter.load_model(&model_id).await.expect("second load");
        let snapshot = adapter.inspect().await.expect("snapshot");

        assert_eq!(
            snapshot
                .residents
                .iter()
                .filter(|resident| resident.model_id == model_id)
                .count(),
            1
        );
        assert_eq!(snapshot.residents[0].state, ResidencyState::Loading);
    }

    #[tokio::test]
    async fn mock_runtime_unload_removes_resident() {
        let adapter = MockRuntimeAdapter::new(RuntimeId("mock".to_string()), Vec::new())
            .with_resident_state(ModelId("qwen9b".to_string()), ResidencyState::HotGpu);

        adapter
            .unload_model(&ModelId("qwen9b".to_string()))
            .await
            .expect("unload");
        let snapshot = adapter.inspect().await.expect("snapshot");

        assert!(snapshot.residents.is_empty());
    }

    #[tokio::test]
    async fn mock_runtime_execute_records_active_request() {
        let adapter = MockRuntimeAdapter::new(RuntimeId("mock".to_string()), Vec::new());
        let request = ExecutionRequest {
            request_id: RequestId::new(),
            model_id: ModelId("qwen9b".to_string()),
            prompt: None,
        };

        adapter.execute(request.clone()).await.expect("execute");
        let snapshot = adapter.inspect().await.expect("snapshot");

        assert_eq!(snapshot.active_requests.len(), 1);
        assert_eq!(snapshot.active_requests[0].request_id, request.request_id);
        assert_eq!(snapshot.active_requests[0].model_id, request.model_id);
    }

    #[tokio::test]
    async fn mock_runtime_memory_snapshot_is_configurable() {
        let adapter = MockRuntimeAdapter::new(RuntimeId("mock".to_string()), Vec::new())
            .with_memory(RuntimeMemorySnapshot {
                vram_total_mb: Some(30_000),
                vram_used_mb: Some(27_000),
                ram_total_mb: Some(64_000),
                ram_used_mb: Some(12_000),
            })
            .with_available(false);

        let snapshot = adapter.inspect().await.expect("snapshot");

        assert!(!snapshot.available);
        assert_eq!(snapshot.memory.pressure_percent(), Some(90));
    }

    #[tokio::test]
    async fn mock_runtime_inspect_is_repeatable() {
        let adapter = MockRuntimeAdapter::new(RuntimeId("mock".to_string()), Vec::new())
            .with_resident_state(ModelId("qwen9b".to_string()), ResidencyState::WarmCpu)
            .with_memory(RuntimeMemorySnapshot {
                vram_total_mb: Some(24_000),
                vram_used_mb: Some(12_000),
                ram_total_mb: None,
                ram_used_mb: None,
            });

        let first = adapter.inspect().await.expect("first snapshot");
        let second = adapter.inspect().await.expect("second snapshot");

        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn llama_swap_health_marks_runtime_available() {
        let server = spawn_fixture(vec![
            http_response(200, "{}"),
            http_response(200, r#"{"data":[]}"#),
        ])
        .await;
        let adapter = LlamaSwapAdapter::new(RuntimeId("llama_swap".to_string()), &server.base_url)
            .expect("adapter");

        let snapshot = adapter.inspect().await.expect("snapshot");

        assert!(snapshot.available);
    }

    #[tokio::test]
    async fn llama_swap_failed_health_marks_runtime_unavailable() {
        let server = spawn_fixture(vec![http_response(500, "{}")]).await;
        let adapter = LlamaSwapAdapter::new(RuntimeId("llama_swap".to_string()), &server.base_url)
            .expect("adapter");

        let snapshot = adapter.inspect().await.expect("snapshot");

        assert!(!snapshot.available);
        assert!(snapshot.residents.is_empty());
    }

    #[tokio::test]
    async fn llama_swap_models_response_normalizes_model_ids() {
        let server = spawn_fixture(vec![http_response(
            200,
            r#"{"data":[{"id":"models/qwen9b.gguf"},{"id":"granite8b"}]}"#,
        )])
        .await;
        let adapter = LlamaSwapAdapter::new(RuntimeId("llama_swap".to_string()), &server.base_url)
            .expect("adapter");

        let models = adapter.inspect_models().await.expect("models");

        assert_eq!(
            models,
            vec![
                ModelId("qwen9b".to_string()),
                ModelId("granite8b".to_string())
            ]
        );
    }

    #[tokio::test]
    async fn llama_swap_inspect_returns_runtime_snapshot() {
        let server = spawn_fixture(vec![
            http_response(200, "{}"),
            http_response(200, r#"{"data":[{"id":"models/qwen9b.gguf"}]}"#),
        ])
        .await;
        let adapter = LlamaSwapAdapter::new(RuntimeId("llama_swap".to_string()), &server.base_url)
            .expect("adapter");

        let snapshot = adapter.inspect().await.expect("snapshot");

        assert_eq!(snapshot.runtime_id, RuntimeId("llama_swap".to_string()));
        assert!(snapshot.available);
        assert!(snapshot.residents.is_empty());
        assert_eq!(snapshot.memory, RuntimeMemorySnapshot::default());
    }

    #[tokio::test]
    async fn llama_swap_probe_does_not_require_mutating_endpoint() {
        let server = spawn_fixture(vec![
            http_response(200, "{}"),
            http_response(200, r#"{"data":[]}"#),
        ])
        .await;
        let adapter = LlamaSwapAdapter::new(RuntimeId("llama_swap".to_string()), &server.base_url)
            .expect("adapter");

        let _ = adapter.inspect().await.expect("inspect");

        let requests = server.requests.lock().expect("requests").clone();
        for request_text in &requests {
            assert!(
                request_text.starts_with("GET"),
                "probe must use GET only, got: {request_text:?}"
            );
        }
    }

    #[tokio::test]
    async fn llama_swap_probe_records_unknown_residency_when_endpoint_is_ambiguous() {
        let server = spawn_fixture(vec![
            http_response(200, "{}"),
            http_response(
                200,
                r#"{"data":[{"id":"models/qwen9b.gguf"},{"id":"models/qwen35_a3b.gguf"}]}"#,
            ),
        ])
        .await;
        let adapter = LlamaSwapAdapter::new(RuntimeId("llama_swap".to_string()), &server.base_url)
            .expect("adapter");

        let snapshot = adapter.inspect().await.expect("snapshot");

        // /v1/models returns models but does not prove hot residency.
        assert!(snapshot.available);
        assert!(
            snapshot.residents.is_empty(),
            "ambiguous endpoint must not claim hot residency"
        );
    }

    #[tokio::test]
    async fn llama_swap_probe_maps_configured_models_without_claiming_hot_residency() {
        let server = spawn_fixture(vec![
            // inspect_models: /v1/models
            http_response(
                200,
                r#"{"data":[{"id":"models/qwen9b.gguf"},{"id":"models/qwen35_a3b.gguf"}]}"#,
            ),
            // inspect: /health
            http_response(200, "{}"),
            // inspect: /v1/models
            http_response(
                200,
                r#"{"data":[{"id":"models/qwen9b.gguf"},{"id":"models/qwen35_a3b.gguf"}]}"#,
            ),
        ])
        .await;
        let adapter = LlamaSwapAdapter::new(RuntimeId("llama_swap".to_string()), &server.base_url)
            .expect("adapter");

        let models = adapter.inspect_models().await.expect("models");

        // inspect_models returns configured model ids but inspect does not claim residency.
        assert_eq!(
            models,
            vec![
                ModelId("qwen9b".to_string()),
                ModelId("qwen35_a3b".to_string()),
            ]
        );

        let snapshot = adapter.inspect().await.expect("snapshot");
        assert!(
            snapshot.residents.is_empty(),
            "configured models must not be reported as hot residents"
        );
    }

    #[tokio::test]
    async fn configured_model_without_runtime_residency_evidence_is_not_hot() {
        let server = spawn_fixture(vec![
            // /health returns ok
            http_response(200, "{}"),
            // /v1/models returns configured models
            http_response(
                200,
                r#"{"data":[{"id":"models/qwen9b.gguf"},{"id":"models/qwen35_a3b.gguf"}]}"#,
            ),
        ])
        .await;
        let adapter = LlamaSwapAdapter::new(RuntimeId("llama_swap".to_string()), &server.base_url)
            .expect("adapter");

        let snapshot = adapter.inspect().await.expect("snapshot");

        // /v1/models returns models but inspect must not claim hot residency.
        assert!(snapshot.available);
        assert_eq!(
            snapshot.residents.len(),
            0,
            "configured models without runtime evidence must not be hot"
        );
    }

    #[tokio::test]
    async fn llama_swap_inspect_populates_residents_from_models_endpoint() {
        // Name preserved from issue #46. Per the residency truth contract,
        // /v1/models proves configuration, not residency — so configured
        // models surface in `configured_models`, not `residents`.
        let server = spawn_fixture(vec![
            http_response(200, "{}"),
            http_response(
                200,
                r#"{"data":[{"id":"models/qwen9b.gguf"},{"id":"granite8b"}]}"#,
            ),
        ])
        .await;
        let adapter = LlamaSwapAdapter::new(RuntimeId("llama_swap".to_string()), &server.base_url)
            .expect("adapter");

        let snapshot = adapter.inspect().await.expect("snapshot");

        assert!(snapshot.available);
        assert_eq!(
            snapshot.configured_models,
            vec![
                ModelId("qwen9b".to_string()),
                ModelId("granite8b".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn llama_swap_inspect_marks_residents_as_configured_not_hot() {
        // Name preserved from issue #46. Configured models from /v1/models must
        // not be reported as residents at all — residency requires evidence
        // beyond configuration (see residency-truth-contract.md).
        let server = spawn_fixture(vec![
            http_response(200, "{}"),
            http_response(200, r#"{"data":[{"id":"models/qwen9b.gguf"}]}"#),
        ])
        .await;
        let adapter = LlamaSwapAdapter::new(RuntimeId("llama_swap".to_string()), &server.base_url)
            .expect("adapter");

        let snapshot = adapter.inspect().await.expect("snapshot");

        assert_eq!(
            snapshot.configured_models,
            vec![ModelId("qwen9b".to_string())]
        );
        assert!(
            snapshot.residents.is_empty(),
            "configured models must not appear as residents"
        );
    }

    #[tokio::test]
    async fn failed_runtime_health_maps_to_unavailable_snapshot() {
        let server = spawn_fixture(vec![http_response(503, "Service Unavailable")]).await;
        let adapter = LlamaSwapAdapter::new(RuntimeId("llama_swap".to_string()), &server.base_url)
            .expect("adapter");

        let snapshot = adapter.inspect().await.expect("snapshot");

        assert!(!snapshot.available);
        assert!(snapshot.residents.is_empty());
        assert_eq!(snapshot.memory, RuntimeMemorySnapshot::default());
    }

    #[tokio::test]
    async fn running_model_endpoint_maps_to_hot_or_serving() {
        let server = spawn_fixture(vec![http_response(
            200,
            r#"{"models":[{"name":"qwen9b","size_vram":9437184000}]}"#,
        )])
        .await;
        let adapter =
            OllamaAdapter::new(RuntimeId("ollama".to_string()), &server.base_url).expect("adapter");

        let snapshot = adapter.inspect().await.expect("snapshot");

        assert_eq!(snapshot.residents.len(), 1);
        assert_eq!(snapshot.residents[0].state, ResidencyState::HotGpu);
    }

    #[tokio::test]
    async fn llama_swap_load_model_posts_to_chat_completions() {
        let server = spawn_fixture(vec![http_response(
            200,
            r#"{"choices":[{"message":{"role":"assistant","content":"ok"}}]}"#,
        )])
        .await;
        let adapter = LlamaSwapAdapter::new(RuntimeId("llama_swap".to_string()), &server.base_url)
            .expect("adapter");

        let handle = adapter
            .load_model(&ModelId("qwen9b".to_string()))
            .await
            .expect("load handle");

        assert_eq!(handle.model_id, ModelId("qwen9b".to_string()));

        let requests = server.requests.lock().expect("requests").clone();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert!(
            request.starts_with("POST /v1/chat/completions"),
            "load_model must POST to /v1/chat/completions, got: {request:?}"
        );
        assert!(
            request.contains("\"model\":\"qwen9b\""),
            "request must carry the requested model id, got: {request:?}"
        );
    }

    #[tokio::test]
    async fn llama_swap_load_model_surfaces_upstream_5xx_as_error() {
        let server = spawn_fixture(vec![http_response(500, "{}")]).await;
        let adapter = LlamaSwapAdapter::new(RuntimeId("llama_swap".to_string()), &server.base_url)
            .expect("adapter");

        let error = adapter
            .load_model(&ModelId("qwen9b".to_string()))
            .await
            .expect_err("upstream 5xx must surface as error");

        assert!(matches!(error, RuntimeError::Http(_)));
    }

    #[tokio::test]
    async fn llama_swap_timeout_returns_runtime_error() {
        let base_url = spawn_timeout_server().await;
        let adapter = LlamaSwapAdapter::new_with_timeout(
            RuntimeId("llama_swap".to_string()),
            &base_url,
            Duration::from_millis(50),
        )
        .expect("adapter");

        let error = adapter.inspect().await.expect_err("timeout");

        assert!(matches!(error, RuntimeError::Http(error) if error.is_timeout()));
    }

    #[tokio::test]
    async fn llama_swap_auth_header_is_applied_when_configured() {
        let server = spawn_fixture(vec![
            http_response(200, "{}"),
            http_response(200, r#"{"data":[]}"#),
        ])
        .await;
        let adapter = LlamaSwapAdapter::new(RuntimeId("llama_swap".to_string()), &server.base_url)
            .expect("adapter")
            .with_bearer_token("secret");

        let _ = adapter.inspect().await.expect("snapshot");

        let requests = server.requests.lock().expect("requests").clone();
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|request| request
            .to_ascii_lowercase()
            .contains("authorization: bearer secret")));
    }

    #[tokio::test]
    async fn ollama_ps_response_maps_running_models_to_hot_residents() {
        let server = spawn_fixture(vec![http_response(
            200,
            r#"{"models":[{"name":"qwen9b","size_vram":9437184000,"size":12582912000}]}"#,
        )])
        .await;
        let adapter =
            OllamaAdapter::new(RuntimeId("ollama".to_string()), &server.base_url).expect("adapter");

        let snapshot = adapter.inspect().await.expect("snapshot");

        assert!(snapshot.available);
        assert_eq!(snapshot.residents.len(), 1);
        assert_eq!(
            snapshot.residents[0].model_id,
            ModelId("qwen9b".to_string())
        );
        assert_eq!(snapshot.residents[0].state, ResidencyState::HotGpu);
    }

    #[tokio::test]
    async fn ollama_ps_empty_response_returns_no_residents() {
        let server = spawn_fixture(vec![http_response(200, r#"{"models":[]}"#)]).await;
        let adapter =
            OllamaAdapter::new(RuntimeId("ollama".to_string()), &server.base_url).expect("adapter");

        let snapshot = adapter.inspect().await.expect("snapshot");

        assert!(snapshot.available);
        assert!(snapshot.residents.is_empty());
    }

    #[tokio::test]
    async fn ollama_ps_vram_bytes_convert_to_mb() {
        let server = spawn_fixture(vec![http_response(
            200,
            r#"{"models":[{"name":"qwen9b","size_vram":9437184000,"size":12582912000}]}"#,
        )])
        .await;
        let adapter =
            OllamaAdapter::new(RuntimeId("ollama".to_string()), &server.base_url).expect("adapter");

        let snapshot = adapter.inspect().await.expect("snapshot");

        assert_eq!(snapshot.residents[0].vram_mb, Some(9000));
        assert_eq!(snapshot.residents[0].ram_mb, Some(12000));
    }

    #[tokio::test]
    async fn ollama_unavailable_runtime_returns_error_or_unavailable_snapshot() {
        let server = spawn_fixture(vec![http_response(503, "{}")]).await;
        let adapter =
            OllamaAdapter::new(RuntimeId("ollama".to_string()), &server.base_url).expect("adapter");

        let snapshot = adapter.inspect().await.expect("snapshot");

        assert!(!snapshot.available);
        assert!(snapshot.residents.is_empty());
    }

    #[tokio::test]
    async fn ollama_malformed_response_returns_runtime_error() {
        let server = spawn_fixture(vec![http_response(200, "not json")]).await;
        let adapter =
            OllamaAdapter::new(RuntimeId("ollama".to_string()), &server.base_url).expect("adapter");

        let error = adapter.inspect().await.expect_err("malformed json");

        assert!(matches!(error, RuntimeError::Http(_)));
    }

    #[test]
    fn ollama_base_url_validation_rejects_invalid_url() {
        let error = OllamaAdapter::new(RuntimeId("ollama".to_string()), "not a url")
            .expect_err("invalid url");

        assert!(error.to_string().contains("invalid runtime url"));
    }

    #[tokio::test]
    async fn llama_cpp_health_marks_runtime_available() {
        let server = spawn_fixture(vec![
            http_response(200, "{}"),
            http_response(200, r#"{"data":[]}"#),
        ])
        .await;
        let adapter = LlamaCppAdapter::new(RuntimeId("llama_cpp".to_string()), &server.base_url)
            .expect("adapter");

        let snapshot = adapter.inspect().await.expect("snapshot");

        assert!(snapshot.available);
    }

    #[tokio::test]
    async fn llama_cpp_failed_health_marks_runtime_unavailable() {
        let server = spawn_fixture(vec![http_response(500, "{}")]).await;
        let adapter = LlamaCppAdapter::new(RuntimeId("llama_cpp".to_string()), &server.base_url)
            .expect("adapter");

        let snapshot = adapter.inspect().await.expect("snapshot");

        assert!(!snapshot.available);
        assert!(snapshot.residents.is_empty());
        assert!(snapshot.configured_models.is_empty());
    }

    #[tokio::test]
    async fn llama_cpp_models_response_normalizes_model_ids() {
        let server = spawn_fixture(vec![http_response(
            200,
            r#"{"data":[{"id":"models/qwen9b.gguf"},{"id":"granite8b"}]}"#,
        )])
        .await;
        let adapter = LlamaCppAdapter::new(RuntimeId("llama_cpp".to_string()), &server.base_url)
            .expect("adapter");

        let models = adapter.inspect_models().await.expect("models");

        assert_eq!(
            models,
            vec![
                ModelId("qwen9b".to_string()),
                ModelId("granite8b".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn llama_cpp_inspect_surfaces_configured_models_without_claiming_residency() {
        let server = spawn_fixture(vec![
            http_response(200, "{}"),
            http_response(200, r#"{"data":[{"id":"models/qwen9b.gguf"}]}"#),
        ])
        .await;
        let adapter = LlamaCppAdapter::new(RuntimeId("llama_cpp".to_string()), &server.base_url)
            .expect("adapter");

        let snapshot = adapter.inspect().await.expect("snapshot");

        assert_eq!(snapshot.runtime_id, RuntimeId("llama_cpp".to_string()));
        assert!(snapshot.available);
        assert_eq!(
            snapshot.configured_models,
            vec![ModelId("qwen9b".to_string())]
        );
        assert!(
            snapshot.residents.is_empty(),
            "/v1/models proves configuration, not residency"
        );
        assert_eq!(snapshot.memory, RuntimeMemorySnapshot::default());
    }

    #[tokio::test]
    async fn llama_cpp_probe_uses_get_only() {
        let server = spawn_fixture(vec![
            http_response(200, "{}"),
            http_response(200, r#"{"data":[]}"#),
        ])
        .await;
        let adapter = LlamaCppAdapter::new(RuntimeId("llama_cpp".to_string()), &server.base_url)
            .expect("adapter");

        let _ = adapter.inspect().await.expect("inspect");

        let requests = server.requests.lock().expect("requests").clone();
        for request_text in &requests {
            assert!(
                request_text.starts_with("GET"),
                "inspect must use GET only, got: {request_text:?}"
            );
        }
    }

    #[tokio::test]
    async fn llama_cpp_load_unload_execute_are_unsupported() {
        let adapter =
            LlamaCppAdapter::new(RuntimeId("llama_cpp".to_string()), "http://localhost:8080")
                .expect("adapter");
        let model = ModelId("qwen9b".to_string());

        let load_error = adapter.load_model(&model).await.expect_err("load");
        let unload_error = adapter.unload_model(&model).await.expect_err("unload");
        let execute_error = adapter
            .execute(ExecutionRequest {
                request_id: RequestId::new(),
                model_id: model,
                prompt: None,
            })
            .await
            .expect_err("execute");

        assert_eq!(
            load_error.to_string(),
            "runtime operation is not supported: llama-cpp load"
        );
        assert_eq!(
            unload_error.to_string(),
            "runtime operation is not supported: llama-cpp unload"
        );
        assert_eq!(
            execute_error.to_string(),
            "runtime operation is not supported: llama-cpp execute"
        );
    }

    #[tokio::test]
    async fn llama_cpp_auth_header_is_applied_when_configured() {
        let server = spawn_fixture(vec![
            http_response(200, "{}"),
            http_response(200, r#"{"data":[]}"#),
        ])
        .await;
        let adapter = LlamaCppAdapter::new(RuntimeId("llama_cpp".to_string()), &server.base_url)
            .expect("adapter")
            .with_bearer_token("secret");

        let _ = adapter.inspect().await.expect("snapshot");

        let requests = server.requests.lock().expect("requests").clone();
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|request| request
            .to_ascii_lowercase()
            .contains("authorization: bearer secret")));
    }

    #[tokio::test]
    async fn llama_cpp_timeout_returns_runtime_error() {
        let base_url = spawn_timeout_server().await;
        let adapter = LlamaCppAdapter::new_with_timeout(
            RuntimeId("llama_cpp".to_string()),
            &base_url,
            Duration::from_millis(50),
        )
        .expect("adapter");

        let error = adapter.inspect().await.expect_err("timeout");

        assert!(matches!(error, RuntimeError::Http(error) if error.is_timeout()));
    }

    #[test]
    fn llama_cpp_base_url_validation_rejects_invalid_url() {
        let error = LlamaCppAdapter::new(RuntimeId("llama_cpp".to_string()), "not a url")
            .expect_err("invalid url");

        assert!(error.to_string().contains("invalid runtime url"));
    }

    #[tokio::test]
    async fn inference_gateway_injects_runtime_auth_token() {
        let server = spawn_fixture(vec![http_response(200, "{}")]).await;
        let target = ForwardTarget {
            base_url: server.base_url.clone(),
            auth_token: Some("s3cr3t-token".to_string()),
        };
        let body = serde_json::json!({ "model": "qwen9b", "messages": [] });

        forward_chat_completion(&target, &body)
            .await
            .expect("forward");

        let request = server.requests.lock().expect("requests")[0].to_lowercase();
        assert!(
            request.contains("authorization: bearer s3cr3t-token"),
            "forwarded request must carry the runtime bearer token, got:\n{request}"
        );
    }

    #[tokio::test]
    async fn forward_chat_completion_omits_auth_when_unset() {
        let server = spawn_fixture(vec![http_response(200, "{}")]).await;
        let target = ForwardTarget {
            base_url: server.base_url.clone(),
            auth_token: None,
        };
        let body = serde_json::json!({ "model": "qwen9b", "messages": [] });

        forward_chat_completion(&target, &body)
            .await
            .expect("forward");

        let request = server.requests.lock().expect("requests")[0].to_lowercase();
        assert!(
            !request.contains("authorization:"),
            "no auth header should be sent when the runtime has no token"
        );
    }

    struct TestServer {
        base_url: String,
        requests: Arc<Mutex<Vec<String>>>,
    }

    async fn spawn_fixture(responses: Vec<String>) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let request_log = requests.clone();
        let responses = Arc::new(Mutex::new(VecDeque::from(responses)));
        let response_queue = responses.clone();

        tokio::spawn(async move {
            loop {
                let Some(response) = response_queue.lock().expect("responses").pop_front() else {
                    break;
                };
                let (mut socket, _) = listener.accept().await.expect("accept");
                let mut buffer = vec![0; 4096];
                let read = socket.read(&mut buffer).await.expect("read");
                request_log
                    .lock()
                    .expect("requests")
                    .push(String::from_utf8_lossy(&buffer[..read]).to_string());
                socket
                    .write_all(response.as_bytes())
                    .await
                    .expect("write response");
            }
        });

        TestServer {
            base_url: format!("http://{addr}"),
            requests,
        }
    }

    async fn spawn_timeout_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut buffer = vec![0; 1024];
            let _ = socket.read(&mut buffer).await;
            tokio::time::sleep(Duration::from_secs(2)).await;
        });
        format!("http://{addr}")
    }

    fn http_response(status: u16, body: &str) -> String {
        let reason = match status {
            200 => "OK",
            500 => "Internal Server Error",
            _ => "Unknown",
        };
        format!(
            "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
            body.len()
        )
    }
}
