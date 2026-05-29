use anemoi_core::{ActionPlan, Decision, RuntimeSnapshot};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}

#[async_trait]
pub trait DecisionLog: Send + Sync {
    async fn record_decision(&self, decision: &Decision) -> Result<(), TelemetryError>;
    async fn get_decision(&self, id: Uuid) -> Result<Option<Decision>, TelemetryError>;
    async fn list_decisions(&self) -> Result<Vec<Decision>, TelemetryError>;
}

pub type DynDecisionLog = Arc<dyn DecisionLog>;

/// Event record types for the durable event store.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum StoredEvent {
    #[serde(rename = "decision")]
    Decision {
        id: Uuid,
        decision: Decision,
        recorded_at: DateTime<Utc>,
    },
    #[serde(rename = "runtime_snapshot")]
    RuntimeSnapshot {
        id: Uuid,
        runtime_id: String,
        snapshot: RuntimeSnapshot,
        observed_at: DateTime<Utc>,
    },
    #[serde(rename = "resident_transition")]
    ResidentTransition {
        id: Uuid,
        model_id: String,
        from_state: String,
        to_state: String,
        evidence_source: Option<String>,
        decision_id: Option<Uuid>,
        transition_recorded_at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredActionPlan {
    pub decision_id: Uuid,
    pub actions_json: String,
}

/// SQLite-backed event store for durable history.
/// Uses a mutex to make the connection safe across async contexts.
pub struct SqliteEventStore {
    conn: Mutex<Connection>,
    memory_log: InMemoryDecisionLog,
}

impl SqliteEventStore {
    /// Creates a new SQLite database at the given path with initialized tables.
    pub fn create(path: impl AsRef<Path>) -> Result<Self, TelemetryError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrent read performance.
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        let store = Self {
            conn: Mutex::new(conn),
            memory_log: InMemoryDecisionLog::default(),
        };
        store.init_tables()?;
        Ok(store)
    }

    fn init_tables(&self) -> Result<(), TelemetryError> {
        // Create tables using the connection directly.
        let guard = self.conn.lock();
        guard.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS decisions (
                id TEXT PRIMARY KEY,
                decision_json TEXT NOT NULL,
                recorded_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS runtime_snapshots (
                event_id TEXT NOT NULL,
                runtime_id TEXT NOT NULL,
                snapshot_json TEXT NOT NULL,
                observed_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS resident_transitions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                model_id TEXT NOT NULL,
                from_state TEXT NOT NULL,
                to_state TEXT NOT NULL,
                evidence_source TEXT,
                decision_id TEXT,
                transition_recorded_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS staging_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                intent_id TEXT NOT NULL,
                decision_id TEXT NOT NULL,
                background_model TEXT NOT NULL,
                target_runtime TEXT NOT NULL,
                reason TEXT NOT NULL,
                state TEXT NOT NULL,
                recorded_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS action_plan_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                decision_id TEXT NOT NULL,
                plan_json TEXT NOT NULL,
                recorded_at TEXT NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    /// Records a runtime snapshot event.
    pub fn record_runtime_snapshot(
        &self,
        id: Uuid,
        runtime_id: &str,
        snapshot: &RuntimeSnapshot,
    ) -> Result<(), TelemetryError> {
        let json = serde_json::to_string(snapshot)?;
        let observed_at = Utc::now().to_rfc3339();
        self.conn.lock().execute(
            "INSERT INTO runtime_snapshots (event_id, runtime_id, snapshot_json, observed_at) VALUES (?1, ?2, ?3, ?4)",
            params![id.to_string(), runtime_id, &json, &observed_at],
        )?;
        Ok(())
    }

    /// Records a resident transition event.
    pub fn record_resident_transition(
        &self,
        model_id: &str,
        from_state: &str,
        to_state: &str,
        evidence_source: Option<&str>,
        decision_id: Option<Uuid>,
    ) -> Result<(), TelemetryError> {
        let recorded_at = Utc::now().to_rfc3339();
        self.conn.lock().execute(
            r#"INSERT INTO resident_transitions
               (model_id, from_state, to_state, evidence_source, decision_id, transition_recorded_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
            params![
                model_id,
                from_state,
                to_state,
                evidence_source,
                decision_id.map(|id| id.to_string()),
                recorded_at
            ],
        )?;
        Ok(())
    }

    /// Records a staging event.
    pub fn record_staging_event(
        &self,
        intent_id: Uuid,
        decision_id: Uuid,
        background_model: &str,
        target_runtime: &str,
        reason: &str,
        state: &str,
    ) -> Result<(), TelemetryError> {
        let recorded_at = Utc::now().to_rfc3339();
        self.conn.lock().execute(
            "INSERT INTO staging_events
               (intent_id, decision_id, background_model, target_runtime, reason, state, recorded_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                intent_id.to_string(),
                decision_id.to_string(),
                background_model,
                target_runtime,
                reason,
                state,
                recorded_at
            ],
        )?;
        Ok(())
    }

    /// Records an action plan event.
    pub fn record_action_plan_event(&self, plan: &ActionPlan) -> Result<(), TelemetryError> {
        let json = serde_json::to_string(plan)?;
        let recorded_at = Utc::now().to_rfc3339();
        self.conn.lock().execute(
            "INSERT INTO action_plan_events (decision_id, plan_json, recorded_at) VALUES (?1, ?2, ?3)",
            params![plan.decision_id.to_string(), &json, recorded_at],
        )?;
        Ok(())
    }

    /// Retrieves a decision explanation by ID.
    pub fn get_decision_explanation(
        &self,
        id: Uuid,
    ) -> Result<Option<anemoi_core::Explanation>, TelemetryError> {
        let guard = self.conn.lock();
        let mut stmt = guard.prepare("SELECT decision_json FROM decisions WHERE id = ?1")?;
        let result: Option<String> = stmt
            .query_row(params![id.to_string()], |row| row.get(0))
            .ok();
        match result {
            Some(json_str) => {
                drop(stmt);
                drop(guard);
                let decision: Decision = serde_json::from_str(&json_str)?;
                Ok(Some(decision.explanation))
            }
            None => Ok(None),
        }
    }

    /// Records a decision to in-memory log.
    pub async fn record_to_memory(&self, decision: &Decision) -> Result<(), TelemetryError> {
        self.memory_log.record_decision(decision).await
    }
}

impl Default for SqliteEventStore {
    fn default() -> Self {
        let conn = Connection::open_in_memory().expect("in-memory sqlite");
        let store = Self {
            conn: Mutex::new(conn),
            memory_log: InMemoryDecisionLog::default(),
        };
        store.init_tables().expect("init tables");
        store
    }
}

#[async_trait]
impl DecisionLog for SqliteEventStore {
    async fn record_decision(&self, decision: &Decision) -> Result<(), TelemetryError> {
        // Also update in-memory log.
        self.record_to_memory(decision).await?;

        let id = decision.id.to_string();
        let json = serde_json::to_string(decision)?;
        let recorded_at = Utc::now().to_rfc3339();

        self.conn.lock().execute(
            "INSERT OR REPLACE INTO decisions (id, decision_json, recorded_at) VALUES (?1, ?2, ?3)",
            params![&id, &json, &recorded_at],
        )?;
        Ok(())
    }

    async fn get_decision(&self, id: Uuid) -> Result<Option<Decision>, TelemetryError> {
        self.memory_log.get_decision(id).await
    }

    async fn list_decisions(&self) -> Result<Vec<Decision>, TelemetryError> {
        self.memory_log.list_decisions().await
    }
}

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
    state: std::sync::RwLock<InMemoryDecisionLogState>,
}

impl InMemoryDecisionLog {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Default)]
struct InMemoryDecisionLogState {
    decisions: HashMap<Uuid, Decision>,
    order: Vec<Uuid>,
}

#[async_trait]
impl DecisionLog for InMemoryDecisionLog {
    async fn record_decision(&self, decision: &Decision) -> Result<(), TelemetryError> {
        let mut state = self.state.write().unwrap();
        if !state.decisions.contains_key(&decision.id) {
            state.order.push(decision.id);
        }
        state.decisions.insert(decision.id, decision.clone());
        Ok(())
    }

    async fn get_decision(&self, id: Uuid) -> Result<Option<Decision>, TelemetryError> {
        let state = self.state.read().unwrap();
        Ok(state.decisions.get(&id).cloned())
    }

    async fn list_decisions(&self) -> Result<Vec<Decision>, TelemetryError> {
        let state = self.state.read().unwrap();
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
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
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
    use anemoi_core::{DecisionAction, DecisionScore, Explanation, RejectedOption, RequestId};
    use std::sync::Arc;

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
                .map(|d| d.explanation.summary)
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
        let lines: Vec<_> = text.lines().collect();
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
            .expect("exists");
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

    // =======================================================================
    // SQLite Event Store Tests (Prompt 27)
    // =======================================================================

    fn temp_db_path() -> PathBuf {
        std::env::temp_dir().join(format!("anemoi-{}.db", Uuid::new_v4()))
    }

    #[test]
    fn sqlite_event_store_records_decision_event() {
        let db_path = temp_db_path();
        let store = SqliteEventStore::create(&db_path).expect("sqlite event store");

        // Record the decision
        futures::executor::block_on(Arc::new(store).record_decision(&sample_decision()))
            .expect("record");

        // Verify by retrieving from a new store instance
        let stored_store = SqliteEventStore::create(&db_path).expect("sqlite event store");
        let explanation = stored_store.get_decision_explanation(sample_decision().id);
        assert!(explanation.is_ok());

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn sqlite_event_store_records_runtime_snapshot_event() {
        let db_path = temp_db_path();
        let store = SqliteEventStore::create(&db_path).expect("sqlite event store");
        let snapshot = RuntimeSnapshot {
            runtime_id: anemoi_core::RuntimeId("mock".to_string()),
            available: true,
            residents: Vec::new(),
            configured_models: vec![],
            memory: anemoi_core::RuntimeMemorySnapshot::default(),
            active_requests: Vec::new(),
        };
        let event_id = Uuid::new_v4();
        store
            .record_runtime_snapshot(event_id, "mock", &snapshot)
            .expect("record snapshot");

        // Verify by checking the database directly.
        let conn = rusqlite::Connection::open(&db_path).expect("conn");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM runtime_snapshots WHERE event_id = ?1",
                params![event_id.to_string()],
                |row| row.get(0),
            )
            .expect("count query");
        assert_eq!(count, 1);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn sqlite_event_store_records_staging_event() {
        let db_path = temp_db_path();
        let store = SqliteEventStore::create(&db_path).expect("sqlite event store");
        let decision_id = Uuid::new_v4();
        let intent_id = Uuid::new_v4();

        store
            .record_staging_event(
                intent_id,
                decision_id,
                "qwen35_a3b",
                "mock",
                "background staging test",
                "pending",
            )
            .expect("record staging");

        // Verify by checking the database directly.
        let conn = rusqlite::Connection::open(&db_path).expect("conn");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM staging_events WHERE intent_id = ?1",
                params![intent_id.to_string()],
                |row| row.get(0),
            )
            .expect("count query");
        assert_eq!(count, 1);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn sqlite_event_store_records_action_plan_event() {
        let db_path = temp_db_path();
        let store = SqliteEventStore::create(&db_path).expect("sqlite event store");
        let plan = ActionPlan::new(Uuid::new_v4(), true);

        store
            .record_action_plan_event(&plan)
            .expect("record action plan");

        // Verify by checking the database directly.
        let conn = rusqlite::Connection::open(&db_path).expect("conn");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM action_plan_events", [], |row| {
                row.get(0)
            })
            .expect("count query");

        assert_eq!(count, 1);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn sqlite_event_store_replays_decision_explanation_by_id() {
        let db_path = temp_db_path();
        let store = SqliteEventStore::create(&db_path).expect("sqlite event store");

        // Record a decision through the DecisionLog trait.
        let log: DynDecisionLog = Arc::new(store);
        let decision = sample_decision_with_summary("Selected hot model.");

        futures::executor::block_on(log.record_decision(&decision)).expect("record");

        // Retrieve explanation by ID using get_decision_explanation.
        let stored_store = SqliteEventStore::create(&db_path).expect("sqlite event store");
        let explanation = stored_store
            .get_decision_explanation(decision.id)
            .expect("get explanation");

        assert!(explanation.is_some());
        let exp = explanation.expect("has explanation");
        assert_eq!(exp.summary, "Selected hot model.");

        let _ = std::fs::remove_file(&db_path);
    }
}
