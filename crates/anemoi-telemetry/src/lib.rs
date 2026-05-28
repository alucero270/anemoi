use anemoi_core::Decision;
use async_trait::async_trait;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[async_trait]
pub trait DecisionLog: Send + Sync {
    async fn record_decision(&self, decision: &Decision) -> Result<(), TelemetryError>;
    async fn get_decision(&self, id: Uuid) -> Result<Option<Decision>, TelemetryError>;
    async fn list_decisions(&self) -> Result<Vec<Decision>, TelemetryError>;
}

pub type DynDecisionLog = Arc<dyn DecisionLog>;

pub fn default_decision_log(jsonl_path: Option<PathBuf>) -> Result<DynDecisionLog, TelemetryError> {
    jsonl_path
        .map(JsonlDecisionLog::new)
        .transpose()
        .map(|log| {
            log.map(|log| Arc::new(log) as DynDecisionLog)
                .unwrap_or_else(|| Arc::new(InMemoryDecisionLog::default()))
        })
}

#[derive(Debug, Default)]
pub struct InMemoryDecisionLog {
    state: RwLock<InMemoryDecisionLogState>,
}

#[derive(Debug, Default)]
struct InMemoryDecisionLogState {
    decisions: HashMap<Uuid, Decision>,
    order: Vec<Uuid>,
}

#[async_trait]
impl DecisionLog for InMemoryDecisionLog {
    async fn record_decision(&self, decision: &Decision) -> Result<(), TelemetryError> {
        let mut state = self.state.write().expect("decision log lock");
        if !state.decisions.contains_key(&decision.id) {
            state.order.push(decision.id);
        }
        state.decisions.insert(decision.id, decision.clone());
        Ok(())
    }

    async fn get_decision(&self, id: Uuid) -> Result<Option<Decision>, TelemetryError> {
        Ok(self
            .state
            .read()
            .expect("decision log lock")
            .decisions
            .get(&id)
            .cloned())
    }

    async fn list_decisions(&self) -> Result<Vec<Decision>, TelemetryError> {
        let state = self.state.read().expect("decision log lock");
        Ok(state
            .order
            .iter()
            .filter_map(|id| state.decisions.get(id).cloned())
            .collect())
    }
}

#[derive(Debug)]
pub struct JsonlDecisionLog {
    memory: InMemoryDecisionLog,
    path: PathBuf,
}

impl JsonlDecisionLog {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, TelemetryError> {
        let path = path.into();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self {
            memory: InMemoryDecisionLog::default(),
            path,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[async_trait]
impl DecisionLog for JsonlDecisionLog {
    async fn record_decision(&self, decision: &Decision) -> Result<(), TelemetryError> {
        self.memory.record_decision(decision).await?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        serde_json::to_writer(&mut file, decision)?;
        file.write_all(b"\n")?;
        Ok(())
    }

    async fn get_decision(&self, id: Uuid) -> Result<Option<Decision>, TelemetryError> {
        self.memory.get_decision(id).await
    }

    async fn list_decisions(&self) -> Result<Vec<Decision>, TelemetryError> {
        self.memory.list_decisions().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anemoi_core::{
        Decision, DecisionAction, DecisionScore, Explanation, RejectedOption, RequestId,
    };
    use chrono::Utc;

    #[tokio::test]
    async fn memory_decision_log_stores_and_gets_decision() {
        let log = InMemoryDecisionLog::default();
        let decision = sample_decision();

        log.record_decision(&decision).await.expect("record");

        assert_eq!(
            log.get_decision(decision.id).await.expect("get"),
            Some(decision)
        );
    }

    #[tokio::test]
    async fn memory_decision_log_returns_none_for_unknown_decision() {
        let log = InMemoryDecisionLog::default();

        let found = log.get_decision(Uuid::new_v4()).await.expect("get");

        assert_eq!(found, None);
    }

    #[tokio::test]
    async fn memory_decision_log_keeps_recent_decisions_in_insert_order() {
        let log = InMemoryDecisionLog::default();
        let first = sample_decision_with_summary("first");
        let second = sample_decision_with_summary("second");

        log.record_decision(&second).await.expect("record second");
        log.record_decision(&first).await.expect("record first");

        assert_eq!(
            log.list_decisions()
                .await
                .expect("list")
                .into_iter()
                .map(|decision| decision.explanation.summary)
                .collect::<Vec<_>>(),
            vec!["second", "first"]
        );
    }

    #[tokio::test]
    async fn jsonl_decision_log_appends_one_json_object_per_decision() {
        let path = temp_jsonl_path();
        let log = JsonlDecisionLog::new(&path).expect("jsonl log");
        let first = sample_decision_with_summary("first");
        let second = sample_decision_with_summary("second");

        log.record_decision(&first).await.expect("record first");
        log.record_decision(&second).await.expect("record second");

        let text = std::fs::read_to_string(&path).expect("jsonl file");
        let lines = text.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert_eq!(
            serde_json::from_str::<Decision>(lines[0]).expect("first json"),
            first
        );
        assert_eq!(
            serde_json::from_str::<Decision>(lines[1]).expect("second json"),
            second
        );

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn jsonl_decision_log_creates_parent_directory_when_needed() {
        let dir = std::env::temp_dir().join(format!("anemoi-{}", Uuid::new_v4()));
        let path = dir.join("nested").join("decisions.jsonl");

        let log = JsonlDecisionLog::new(&path).expect("jsonl log");
        log.record_decision(&sample_decision())
            .await
            .expect("record");

        assert!(path.exists());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn jsonl_decision_log_does_not_require_sqlite() {
        let log = default_decision_log(None).expect("memory log");
        let decision = sample_decision();

        log.record_decision(&decision).await.expect("record");

        assert_eq!(
            log.get_decision(decision.id).await.expect("get"),
            Some(decision)
        );
    }

    #[tokio::test]
    async fn telemetry_trait_supports_memory_and_jsonl_logs() {
        let memory: DynDecisionLog = Arc::new(InMemoryDecisionLog::default());
        let jsonl_path = temp_jsonl_path();
        let jsonl: DynDecisionLog = Arc::new(JsonlDecisionLog::new(&jsonl_path).expect("jsonl"));

        for log in [memory, jsonl] {
            let decision = sample_decision();
            log.record_decision(&decision).await.expect("record");
            assert_eq!(
                log.get_decision(decision.id).await.expect("get"),
                Some(decision)
            );
        }

        let _ = std::fs::remove_file(jsonl_path);
    }

    #[tokio::test]
    async fn decision_log_defaults_to_memory() {
        let log = default_decision_log(None).expect("default log");
        let decision = sample_decision();

        log.record_decision(&decision)
            .await
            .expect("record decision");

        let found = log
            .get_decision(decision.id)
            .await
            .expect("get decision")
            .expect("decision exists");
        assert_eq!(found.id, decision.id);
    }

    #[tokio::test]
    async fn jsonl_decision_log_appends_decisions() {
        let path = std::env::temp_dir().join(format!("anemoi-{}.jsonl", Uuid::new_v4()));
        let log = JsonlDecisionLog::new(&path).expect("jsonl log");
        let decision = sample_decision();

        log.record_decision(&decision)
            .await
            .expect("record decision");

        let text = std::fs::read_to_string(&path).expect("jsonl file");
        assert!(text.contains(&decision.id.to_string()));

        let _ = std::fs::remove_file(path);
    }

    fn sample_decision() -> Decision {
        sample_decision_with_summary("No runnable model candidate was available.")
    }

    fn sample_decision_with_summary(summary: &str) -> Decision {
        Decision {
            id: Uuid::new_v4(),
            request_id: RequestId::new(),
            action: DecisionAction::Deny,
            selected_model: None,
            selected_runtime: None,
            selected_group: None,
            background_model: None,
            score: DecisionScore::default(),
            explanation: Explanation {
                summary: summary.to_string(),
                reasons: Vec::new(),
                rejected_options: Vec::<RejectedOption>::new(),
            },
            created_at: Utc::now(),
        }
    }

    fn temp_jsonl_path() -> PathBuf {
        std::env::temp_dir().join(format!("anemoi-{}.jsonl", Uuid::new_v4()))
    }
}
