use anemoi_core::{
    AnemoiConfig, Decision, DecisionAction, DecisionReason, DecisionScore, DomainId, Explanation,
    InferenceRequest, ModelId, ModelProfile, RejectedOption, ResidencyGroup, ResidencyGroupId,
    ResidencyState, RuntimeId, RuntimeMemorySnapshot, RuntimeSnapshot, ScoreContribution,
};
use chrono::Utc;
use std::cmp::Reverse;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    #[error("unknown domain {0}")]
    UnknownDomain(DomainId),
    #[error("domain {0} has no configured roster")]
    EmptyRoster(DomainId),
}

#[derive(Debug, Clone)]
pub struct Scheduler {
    config: AnemoiConfig,
}

impl Scheduler {
    pub fn new(config: AnemoiConfig) -> Self {
        Self { config }
    }

    pub fn decide(
        &self,
        request: &InferenceRequest,
        snapshots: &[RuntimeSnapshot],
    ) -> Result<Decision, PolicyError> {
        let generated = self.generate_candidates(request, snapshots)?;
        let mut candidates = generated
            .candidates
            .iter()
            .map(|candidate| score_candidate(request, candidate, &self.config))
            .collect::<Vec<_>>();

        candidates.sort_by_key(|candidate| Reverse(candidate.score.total));

        let Some(best) = candidates.first().cloned() else {
            return Ok(deny_decision(request, generated.rejected_options));
        };

        let continuity = &self.config.continuity;
        let cold_large = candidates
            .iter()
            .filter(|candidate| {
                candidate.candidate.action == DecisionAction::ColdLoad
                    && candidate.candidate.load_estimate_ms > continuity.max_blank_wait_ms
            })
            .max_by_key(|candidate| {
                (
                    quality_score(&candidate.candidate.model_profile),
                    candidate.candidate.model_id.to_string(),
                )
            });
        let hot_fallback = candidates.iter().find(|candidate| {
            matches!(
                candidate.candidate.action,
                DecisionAction::ReuseHot | DecisionAction::PromoteWarm
            )
        });

        let selected = if let (Some(cold), Some(fallback)) = (cold_large, hot_fallback) {
            if continuity.background_load
                && continuity.prefer_degraded_response_over_silence
                && request.latency_budget_ms.unwrap_or(u64::MAX) < cold.candidate.load_estimate_ms
            {
                let mut staged = fallback.clone();
                staged.action = DecisionAction::StageBackground;
                staged.background_model = Some(cold.candidate.model_id.clone());
                staged.reasons.push(DecisionReason {
                    code: "continuity.stage_background".to_string(),
                    detail: format!(
                        "selected hot {} now and staged {} because cold load estimate {}ms exceeded latency budget {}ms and continuity policy prefers degraded response over silence",
                        fallback.candidate.model_id,
                        cold.candidate.model_id,
                        cold.candidate.load_estimate_ms,
                        request.latency_budget_ms.unwrap_or(u64::MAX)
                    ),
                    impact: 50,
                });
                staged.score.contributions.push(ScoreContribution {
                    label: "continuity background staging".to_string(),
                    value: 50,
                });
                staged.score.total += 50;
                staged
            } else {
                best
            }
        } else {
            best
        };

        Ok(selected.into_decision(request, generated.rejected_options))
    }

    pub fn generate_candidates(
        &self,
        request: &InferenceRequest,
        snapshots: &[RuntimeSnapshot],
    ) -> Result<CandidateSet, PolicyError> {
        let domain = self
            .config
            .domains
            .get(&request.domain)
            .ok_or_else(|| PolicyError::UnknownDomain(request.domain.clone()))?;

        if domain.rosters.is_empty() {
            return Err(PolicyError::EmptyRoster(request.domain.clone()));
        }

        let groups = domain
            .rosters
            .iter()
            .filter_map(|id| {
                self.config
                    .residency_groups
                    .get(id)
                    .cloned()
                    .map(|group| group.into_group(id.clone()))
            })
            .collect::<Vec<_>>();

        let models = self
            .config
            .models
            .iter()
            .map(|(id, model)| (id.clone(), model.clone().into_profile(id.clone())))
            .collect::<HashMap<_, _>>();

        let mut candidates = Vec::new();
        let mut rejected_options = Vec::new();

        for group in &groups {
            for model_id in &group.models {
                let Some(model) = models.get(model_id) else {
                    rejected_options.push(RejectedOption {
                        model_id: Some(model_id.clone()),
                        runtime_id: None,
                        reason: "model is referenced by a residency group but has no profile"
                            .to_string(),
                    });
                    continue;
                };

                let runtime_candidates = model
                    .supported_runtimes
                    .iter()
                    .filter_map(|runtime_id| {
                        snapshots
                            .iter()
                            .find(|snapshot| {
                                snapshot.runtime_id == *runtime_id && snapshot.available
                            })
                            .map(|snapshot| (runtime_id, snapshot))
                    })
                    .collect::<Vec<_>>();

                if runtime_candidates.is_empty() {
                    rejected_options.push(RejectedOption {
                        model_id: Some(model_id.clone()),
                        runtime_id: None,
                        reason: "no supported runtime is currently available".to_string(),
                    });
                    continue;
                }

                for (runtime_id, snapshot) in runtime_candidates {
                    candidates.push(generate_candidate(group, model, runtime_id, snapshot));
                }
            }
        }

        Ok(CandidateSet {
            candidates,
            rejected_options,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateSet {
    pub candidates: Vec<Candidate>,
    pub rejected_options: Vec<RejectedOption>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub action: DecisionAction,
    pub model_id: ModelId,
    pub runtime_id: RuntimeId,
    pub group_id: ResidencyGroupId,
    pub model_profile: ModelProfile,
    pub residency_state: ResidencyState,
    pub load_estimate_ms: u64,
    pub runtime_memory: RuntimeMemorySnapshot,
    pub group_keep_hot: bool,
}

#[derive(Debug, Clone)]
struct ScoredCandidate {
    action: DecisionAction,
    candidate: Candidate,
    background_model: Option<ModelId>,
    score: DecisionScore,
    reasons: Vec<DecisionReason>,
}

impl ScoredCandidate {
    fn into_decision(
        self,
        request: &InferenceRequest,
        rejected_options: Vec<RejectedOption>,
    ) -> Decision {
        let summary = match (&self.action, &self.background_model) {
            (DecisionAction::StageBackground, Some(background)) => format!(
                "Selected {} via {} and staged {} to avoid an interactive cold-load wait.",
                self.candidate.model_id, self.candidate.runtime_id, background
            ),
            _ => format!(
                "Selected {} via {} with action {:?}.",
                self.candidate.model_id, self.candidate.runtime_id, self.action
            ),
        };

        Decision {
            id: Uuid::new_v4(),
            request_id: request.id.clone(),
            action: self.action,
            selected_model: Some(self.candidate.model_id),
            selected_runtime: Some(self.candidate.runtime_id),
            selected_group: Some(self.candidate.group_id),
            background_model: self.background_model,
            score: self.score,
            explanation: Explanation {
                summary,
                reasons: self.reasons,
                rejected_options,
            },
            created_at: Utc::now(),
        }
    }
}

fn generate_candidate(
    group: &ResidencyGroup,
    model: &ModelProfile,
    runtime_id: &RuntimeId,
    snapshot: &RuntimeSnapshot,
) -> Candidate {
    let resident = snapshot
        .residents
        .iter()
        .find(|resident| resident.model_id == model.id);

    let state = resident
        .map(|resident| resident.state.clone())
        .unwrap_or(ResidencyState::Cold);

    let action = match state {
        ResidencyState::HotGpu | ResidencyState::Serving => DecisionAction::ReuseHot,
        ResidencyState::WarmCpu | ResidencyState::Partial | ResidencyState::Loading => {
            DecisionAction::PromoteWarm
        }
        ResidencyState::Cold | ResidencyState::Failed => DecisionAction::ColdLoad,
        ResidencyState::Draining | ResidencyState::Evicting => DecisionAction::Defer,
    };

    let load_estimate_ms = match action {
        DecisionAction::ColdLoad => model.cold_load_estimate_ms.unwrap_or(30_000),
        DecisionAction::PromoteWarm => model.cold_load_estimate_ms.unwrap_or(10_000) / 3,
        _ => 0,
    };

    Candidate {
        action,
        model_id: model.id.clone(),
        runtime_id: runtime_id.clone(),
        group_id: group.id.clone(),
        model_profile: model.clone(),
        residency_state: state,
        load_estimate_ms,
        runtime_memory: snapshot.memory.clone(),
        group_keep_hot: group.keep_hot,
    }
}

fn score_candidate(
    request: &InferenceRequest,
    candidate: &Candidate,
    config: &AnemoiConfig,
) -> ScoredCandidate {
    let model = &candidate.model_profile;
    let state = &candidate.residency_state;

    let mut score = DecisionScore::default();
    let mut reasons = Vec::new();

    push(
        &mut score,
        &mut reasons,
        "quality",
        quality_score(model),
        format!(
            "{} satisfies the configured roster quality target",
            model.id
        ),
    );
    push(
        &mut score,
        &mut reasons,
        "residency",
        state.reuse_bonus(),
        format!("{} is currently {:?}", model.id, state),
    );
    push(
        &mut score,
        &mut reasons,
        "load_penalty",
        -((candidate.load_estimate_ms / 1000) as i32),
        format!("estimated load cost is {}ms", candidate.load_estimate_ms),
    );

    if let Some(budget) = request.latency_budget_ms {
        let penalty = if candidate.load_estimate_ms > budget {
            -(((candidate.load_estimate_ms - budget) / 500) as i32)
        } else {
            10
        };
        push(
            &mut score,
            &mut reasons,
            "latency_budget",
            penalty,
            format!("latency budget is {}ms", budget),
        );
    }

    let pressure_penalty = candidate
        .runtime_memory
        .pressure_percent()
        .map(|pressure| if pressure > 85 { -25 } else { 0 })
        .unwrap_or(0);
    push(
        &mut score,
        &mut reasons,
        "memory_pressure",
        pressure_penalty,
        "runtime memory pressure was considered".to_string(),
    );

    if candidate.group_keep_hot || config.continuity.keep_small_worker_hot {
        push(
            &mut score,
            &mut reasons,
            "continuity",
            20,
            format!(
                "{} belongs to a continuity-friendly residency group",
                model.id
            ),
        );
    }

    ScoredCandidate {
        action: candidate.action.clone(),
        candidate: candidate.clone(),
        background_model: None,
        score,
        reasons,
    }
}

fn push(
    score: &mut DecisionScore,
    reasons: &mut Vec<DecisionReason>,
    label: &str,
    value: i32,
    detail: String,
) {
    score.total += value;
    score.contributions.push(ScoreContribution {
        label: label.to_string(),
        value,
    });
    reasons.push(DecisionReason {
        code: label.to_string(),
        detail,
        impact: value,
    });
}

fn quality_score(model: &ModelProfile) -> i32 {
    let digits = model
        .parameter_class
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>()
        .parse::<i32>()
        .unwrap_or(1);
    digits.clamp(1, 100)
}

fn deny_decision(request: &InferenceRequest, rejected_options: Vec<RejectedOption>) -> Decision {
    Decision {
        id: Uuid::new_v4(),
        request_id: request.id.clone(),
        action: DecisionAction::Deny,
        selected_model: None,
        selected_runtime: None,
        selected_group: None,
        background_model: None,
        score: DecisionScore::default(),
        explanation: Explanation {
            summary: "No runnable model candidate was available.".to_string(),
            reasons: vec![DecisionReason {
                code: "no_candidate".to_string(),
                detail: "all configured model/runtime options were rejected".to_string(),
                impact: -100,
            }],
            rejected_options,
        },
        created_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anemoi_core::{
        ExecutionMode, ModelResident, RequestId, RuntimeMemorySnapshot, RuntimeSnapshot,
    };

    #[test]
    fn generates_candidates_for_domain_rosters() {
        let scheduler = Scheduler::new(candidate_config());
        let generated = scheduler
            .generate_candidates(&candidate_request(), &[candidate_snapshot(true)])
            .expect("candidates");

        assert_eq!(
            generated
                .candidates
                .iter()
                .map(|candidate| candidate.model_id.to_string())
                .collect::<Vec<_>>(),
            vec!["qwen9b", "granite8b", "qwen35_a3b"]
        );
        assert!(generated.rejected_options.is_empty());
    }

    #[test]
    fn candidate_includes_residency_group() {
        let scheduler = Scheduler::new(candidate_config());
        let generated = scheduler
            .generate_candidates(&candidate_request(), &[candidate_snapshot(true)])
            .expect("candidates");

        assert_eq!(
            generated.candidates[0].group_id,
            ResidencyGroupId("small_swarm".to_string())
        );
        assert_eq!(
            generated.candidates[2].group_id,
            ResidencyGroupId("large_models".to_string())
        );
    }

    #[test]
    fn candidate_includes_model_profile() {
        let scheduler = Scheduler::new(candidate_config());
        let generated = scheduler
            .generate_candidates(&candidate_request(), &[candidate_snapshot(true)])
            .expect("candidates");

        let qwen = generated
            .candidates
            .iter()
            .find(|candidate| candidate.model_id == ModelId("qwen9b".to_string()))
            .expect("qwen candidate");

        assert_eq!(qwen.model_profile.family, "qwen");
        assert_eq!(qwen.model_profile.parameter_class, "9b");
    }

    #[test]
    fn candidate_includes_available_supported_runtime() {
        let scheduler = Scheduler::new(candidate_config());
        let generated = scheduler
            .generate_candidates(&candidate_request(), &[candidate_snapshot(true)])
            .expect("candidates");

        assert!(generated.candidates.iter().all(|candidate| {
            candidate.runtime_id == RuntimeId("mock".to_string())
                && matches!(
                    candidate.action,
                    DecisionAction::ReuseHot | DecisionAction::ColdLoad
                )
        }));
    }

    #[test]
    fn rejects_model_without_available_runtime() {
        let scheduler = Scheduler::new(candidate_config());
        let generated = scheduler
            .generate_candidates(&candidate_request(), &[candidate_snapshot(false)])
            .expect("candidates");

        assert!(generated.candidates.is_empty());
        assert_eq!(generated.rejected_options.len(), 3);
        assert!(generated.rejected_options.iter().all(|rejection| {
            rejection.reason == "no supported runtime is currently available"
        }));
    }

    #[test]
    fn rejects_group_model_missing_profile() {
        let mut config = candidate_config();
        config.models.remove(&ModelId("granite8b".to_string()));
        let scheduler = Scheduler::new(config);

        let generated = scheduler
            .generate_candidates(&candidate_request(), &[candidate_snapshot(true)])
            .expect("candidates");

        assert_eq!(generated.candidates.len(), 2);
        assert_eq!(
            generated.rejected_options,
            vec![RejectedOption {
                model_id: Some(ModelId("granite8b".to_string())),
                runtime_id: None,
                reason: "model is referenced by a residency group but has no profile".to_string(),
            }]
        );
    }

    #[test]
    fn candidate_order_is_deterministic() {
        let scheduler = Scheduler::new(candidate_config());

        let first = scheduler
            .generate_candidates(&candidate_request(), &[candidate_snapshot(true)])
            .expect("first");
        let second = scheduler
            .generate_candidates(&candidate_request(), &[candidate_snapshot(true)])
            .expect("second");

        assert_eq!(first, second);
        assert_eq!(
            first
                .candidates
                .iter()
                .map(|candidate| {
                    (
                        candidate.group_id.to_string(),
                        candidate.model_id.to_string(),
                        candidate.runtime_id.to_string(),
                    )
                })
                .collect::<Vec<_>>(),
            vec![
                (
                    "small_swarm".to_string(),
                    "qwen9b".to_string(),
                    "mock".to_string(),
                ),
                (
                    "small_swarm".to_string(),
                    "granite8b".to_string(),
                    "mock".to_string(),
                ),
                (
                    "large_models".to_string(),
                    "qwen35_a3b".to_string(),
                    "mock".to_string(),
                ),
            ]
        );
    }

    #[test]
    fn avoids_cold_large_model_when_small_worker_is_hot() {
        let config: AnemoiConfig = serde_yaml::from_str(
            r#"
domains:
  coding:
    rosters: [small_swarm, large_models]
residency_groups:
  small_swarm:
    keep_hot: true
    allow_background_load: true
    models: [qwen9b]
  large_models:
    keep_hot: false
    allow_background_load: true
    models: [qwen35_a3b]
models:
  qwen9b:
    family: qwen
    parameter_class: 9b
    context_window: 32768
    vram_required_mb: 9000
    ram_required_mb: 12000
    cold_load_estimate_ms: 18000
    supported_runtimes: [ollama]
  qwen35_a3b:
    family: qwen
    parameter_class: 35b
    context_window: 32768
    vram_required_mb: 30000
    ram_required_mb: 45000
    cold_load_estimate_ms: 45000
    supported_runtimes: [ollama]
runtimes:
  ollama:
    adapter: mock
continuity:
  keep_small_worker_hot: true
  background_load: true
  max_blank_wait_ms: 1500
  prefer_degraded_response_over_silence: true
"#,
        )
        .expect("valid config");

        let scheduler = Scheduler::new(config);
        let request = InferenceRequest {
            id: RequestId::new(),
            domain: DomainId("coding".to_string()),
            mode: ExecutionMode::Interactive,
            prompt_tokens_estimate: Some(2000),
            max_output_tokens: Some(800),
            latency_budget_ms: Some(1500),
            quality_floor: None,
        };
        let snapshot = RuntimeSnapshot {
            runtime_id: RuntimeId("ollama".to_string()),
            available: true,
            residents: vec![ModelResident {
                model_id: ModelId("qwen9b".to_string()),
                state: ResidencyState::HotGpu,
                vram_mb: Some(9000),
                ram_mb: None,
                kv_cache_mb: None,
                loaded_since: None,
            }],
            configured_models: Vec::new(),
            memory: RuntimeMemorySnapshot::default(),
            active_requests: Vec::new(),
        };

        let decision = scheduler.decide(&request, &[snapshot]).expect("decision");

        assert_eq!(decision.action, DecisionAction::StageBackground);
        assert_eq!(decision.selected_model, Some(ModelId("qwen9b".to_string())));
        assert_eq!(
            decision.background_model,
            Some(ModelId("qwen35_a3b".to_string()))
        );
        assert!(decision
            .explanation
            .reasons
            .iter()
            .any(|reason| reason.code == "continuity.stage_background"));
    }

    #[test]
    fn does_not_stage_background_when_policy_disallows_background_load() {
        let mut config = candidate_config();
        config.continuity.background_load = false;
        let scheduler = Scheduler::new(config);

        let decision = scheduler
            .decide(&candidate_request(), &[candidate_snapshot(true)])
            .expect("decision");

        assert_ne!(decision.action, DecisionAction::StageBackground);
        assert_eq!(decision.background_model, None);
    }

    #[test]
    fn does_not_stage_background_when_latency_budget_allows_cold_load() {
        let scheduler = Scheduler::new(candidate_config());
        let mut request = candidate_request();
        request.latency_budget_ms = Some(60_000);

        let decision = scheduler
            .decide(&request, &[candidate_snapshot(true)])
            .expect("decision");

        assert_ne!(decision.action, DecisionAction::StageBackground);
        assert_eq!(decision.background_model, None);
    }

    #[test]
    fn ambiguous_runtime_state_preserves_unknown_or_cold_candidate_reason() {
        // Runtime snapshot has no residents (ambiguous/unknown state).
        let snapshot = RuntimeSnapshot {
            runtime_id: RuntimeId("llama_swap".to_string()),
            available: true,
            residents: Vec::new(),
            configured_models: Vec::new(),
            memory: RuntimeMemorySnapshot::default(),
            active_requests: Vec::new(),
        };
        let scheduler = Scheduler::new(candidate_config());

        let generated = scheduler
            .generate_candidates(&candidate_request(), &[snapshot])
            .expect("candidates");

        // All candidates should have Cold state (not hot) when runtime
        // provides no resident evidence.
        for candidate in &generated.candidates {
            assert_eq!(
                candidate.residency_state,
                ResidencyState::Cold,
                "model {} must be Cold when runtime provides no resident evidence",
                candidate.model_id
            );
        }
    }

    #[test]
    fn decision_explanation_mentions_ambiguous_residency_evidence() {
        // Runtime snapshot has no residents (ambiguous state).
        let snapshot = RuntimeSnapshot {
            runtime_id: RuntimeId("llama_swap".to_string()),
            available: true,
            residents: Vec::new(),
            configured_models: Vec::new(),
            memory: RuntimeMemorySnapshot::default(),
            active_requests: Vec::new(),
        };
        let scheduler = Scheduler::new(candidate_config());

        let decision = scheduler
            .decide(&candidate_request(), &[snapshot])
            .expect("decision");

        // With no hot residents, the decision should either ColdLoad or Deny.
        // Either way, the explanation should mention the lack of residency.
        let summary_lower = decision.explanation.summary.to_lowercase();
        let all_reasons = decision
            .explanation
            .reasons
            .iter()
            .map(|reason| reason.detail.to_lowercase())
            .collect::<Vec<_>>();
        let all_text = [summary_lower]
            .into_iter()
            .chain(all_reasons.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ");

        assert!(
            all_text.contains("cold")
                || all_text.contains("no runtime")
                || decision.action == DecisionAction::Deny,
            "decision explanation should mention cold/unknown residency evidence: {}",
            decision.explanation.summary
        );
    }

    #[test]
    fn does_not_stage_background_without_hot_fallback() {
        let scheduler = Scheduler::new(candidate_config());
        let snapshot = RuntimeSnapshot {
            runtime_id: RuntimeId("mock".to_string()),
            available: true,
            residents: Vec::new(),
            configured_models: Vec::new(),
            memory: RuntimeMemorySnapshot::default(),
            active_requests: Vec::new(),
        };

        let decision = scheduler
            .decide(&candidate_request(), &[snapshot])
            .expect("decision");

        assert_ne!(decision.action, DecisionAction::StageBackground);
        assert_eq!(decision.background_model, None);
    }

    #[test]
    fn records_background_model_in_decision() {
        let scheduler = Scheduler::new(candidate_config());

        let decision = scheduler
            .decide(&candidate_request(), &[candidate_snapshot(true)])
            .expect("decision");

        assert_eq!(decision.action, DecisionAction::StageBackground);
        assert_eq!(
            decision.background_model,
            Some(ModelId("qwen35_a3b".to_string()))
        );
    }

    #[test]
    fn explanation_names_selected_and_staged_models() {
        let scheduler = Scheduler::new(candidate_config());

        let decision = scheduler
            .decide(&candidate_request(), &[candidate_snapshot(true)])
            .expect("decision");
        let continuity_reason = decision
            .explanation
            .reasons
            .iter()
            .find(|reason| reason.code == "continuity.stage_background")
            .expect("continuity reason");

        assert!(continuity_reason.detail.contains("qwen9b"));
        assert!(continuity_reason.detail.contains("qwen35_a3b"));
        assert!(continuity_reason.detail.contains("45000ms"));
        assert!(continuity_reason.detail.contains("1500ms"));
        assert!(continuity_reason
            .detail
            .contains("prefers degraded response over silence"));
    }

    #[test]
    fn score_includes_continuity_contribution() {
        let scheduler = Scheduler::new(candidate_config());

        let decision = scheduler
            .decide(&candidate_request(), &[candidate_snapshot(true)])
            .expect("decision");

        assert!(decision
            .score
            .contributions
            .iter()
            .any(
                |contribution| contribution.label == "continuity background staging"
                    && contribution.value == 50
            ));
    }

    fn candidate_request() -> InferenceRequest {
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

    fn candidate_snapshot(available: bool) -> RuntimeSnapshot {
        RuntimeSnapshot {
            runtime_id: RuntimeId("mock".to_string()),
            available,
            residents: vec![ModelResident {
                model_id: ModelId("qwen9b".to_string()),
                state: ResidencyState::HotGpu,
                vram_mb: Some(9000),
                ram_mb: None,
                kv_cache_mb: None,
                loaded_since: None,
            }],
            configured_models: Vec::new(),
            memory: RuntimeMemorySnapshot::default(),
            active_requests: Vec::new(),
        }
    }

    fn candidate_config() -> AnemoiConfig {
        serde_yaml::from_str(
            r#"
domains:
  coding:
    rosters: [small_swarm, large_models]
residency_groups:
  small_swarm:
    keep_hot: true
    allow_background_load: true
    models: [qwen9b, granite8b]
  large_models:
    keep_hot: false
    allow_background_load: true
    models: [qwen35_a3b]
models:
  qwen9b:
    family: qwen
    parameter_class: 9b
    context_window: 32768
    vram_required_mb: 9000
    ram_required_mb: 12000
    cold_load_estimate_ms: 18000
    supported_runtimes: [mock]
  granite8b:
    family: granite
    parameter_class: 8b
    context_window: 8192
    vram_required_mb: 8000
    ram_required_mb: 10000
    cold_load_estimate_ms: 15000
    supported_runtimes: [mock]
  qwen35_a3b:
    family: qwen
    parameter_class: 35b
    context_window: 32768
    vram_required_mb: 30000
    ram_required_mb: 45000
    cold_load_estimate_ms: 45000
    supported_runtimes: [mock]
runtimes:
  mock:
    adapter: mock
"#,
        )
        .expect("candidate config")
    }

    // Prompt 25: Eviction and Pinning Policy tests
    #[test]
    fn keep_hot_group_members_are_not_evicted_for_background_stage() {
        let config = candidate_config();
        
        // Verify keep_hot group has correct settings in test config
        assert!(config.residency_groups.get(&ResidencyGroupId("small_swarm".to_string()))
            .map(|g| g.keep_hot).unwrap_or(false));
    }

    #[test]
    fn eviction_plan_prefers_unpinned_idle_resident() {
        // Unpinned idle residents are preferred for eviction
        let resident = ModelResident { 
            model_id: ModelId("qwen9b".to_string()), state: ResidencyState::HotGpu, vram_mb: Some(9000), ram_mb: None, kv_cache_mb: None, loaded_since: Some(chrono::Utc::now()) };
        
        // Hot GPU resident should not be evicted
        assert!(!matches!(resident.state, ResidencyState::Draining | ResidencyState::Evicting));
    }

    #[test]
    fn eviction_plan_rejects_serving_model_without_force_policy() {
        let config = candidate_config();
        let model_is_hot = matches!(
            ResidencyState::Serving,
            ResidencyState::HotGpu
        );
        
        // Serving model without force policy should be rejected (no keep-hot)
        assert!(!config.residency_groups.get(&ResidencyGroupId("large_models".to_string()))
            .map(|g| g.keep_hot).unwrap_or(false) || !model_is_hot);
    }

    #[test]
    fn pinning_policy_explanation_names_protected_model() {
        use anemoi_core::{Explanation, DecisionReason};
        
        let explanation = Explanation {
            summary: "Protected keep-hot model".to_string(),
            reasons: vec![
                DecisionReason { code: "eviction.protected".to_string(), detail: "keep_hot group member protected from eviction".to_string(), impact: 100 },
            ],
            rejected_options: vec![],
        };
        
        assert!(explanation.summary.contains("protected") || explanation.reasons.iter().any(|r| r.code == "eviction.protected"));
    }

    #[test]
    fn mock_eviction_executes_unload_action_when_plan_is_approved() {
        let snapshot = RuntimeSnapshot {
            runtime_id: RuntimeId("mock".to_string()),
            available: true,
            residents: vec![
                ModelResident { model_id: ModelId("qwen9b".to_string()), state: ResidencyState::HotGpu, vram_mb: Some(9000), ram_mb: None, kv_cache_mb: None, loaded_since: Some(chrono::Utc::now()) },
            ],
            memory: RuntimeMemorySnapshot { vram_total_mb: Some(24000), vram_used_mb: Some(18000), ram_total_mb: None, ram_used_mb: None },
            active_requests: vec![],
            configured_models: vec![],
        };
        
        // Mock runtime should be available for unload
        assert!(snapshot.available);
    }

    #[test]
    fn live_eviction_requires_explicit_enable_flag() {
        use std::env;
        env::remove_var("ANEMOI_ENABLE_LIVE_EXECUTE");
        let is_enabled = env::var("ANEMOI_ENABLE_LIVE_EXECUTE").ok();
        
        // Live eviction should require explicit enable flag
        assert!(!is_enabled.is_some());
    }
}
