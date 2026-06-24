use std::fmt;
use std::time::{Duration, Instant};

use super::candidate::CandidateBuffer;
use super::overlay::OverlayStateMachine;
use super::pipeline::{CritiquePipeline, PipelineOutcome, PostSendEvent};
use super::provider::MockProvider;
use super::sensitivity::ElementMetadata;
use super::types::{CritiqueResult, SarcasmStrength, SpellingStrength};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportClaim {
    HarnessOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessOutcome {
    FeedbackShown {
        result: CritiqueResult,
        overlay_within_target: bool,
        support_claim: SupportClaim,
    },
    Excluded {
        reasons: Vec<String>,
        support_claim: SupportClaim,
    },
    NoCandidate,
    ProviderFailed,
}

#[derive(Clone, PartialEq, Eq)]
pub struct HarnessScenario {
    pub app_id: String,
    pub message: String,
    pub metadata: ElementMetadata,
    pub spelling_strength: SpellingStrength,
    pub sarcasm_strength: SarcasmStrength,
}

impl fmt::Debug for HarnessScenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HarnessScenario")
            .field("app_id", &self.app_id)
            .field("message", &"<redacted>")
            .field("metadata", &self.metadata)
            .field("spelling_strength", &self.spelling_strength)
            .field("sarcasm_strength", &self.sarcasm_strength)
            .finish()
    }
}

impl HarnessScenario {
    pub fn discord_typo(message: impl Into<String>) -> Self {
        Self {
            app_id: "discord".to_owned(),
            message: message.into(),
            metadata: ElementMetadata::chat_input("discord", "Message #없는말-test"),
            spelling_strength: SpellingStrength::Medium,
            sarcasm_strength: SarcasmStrength::Weak,
        }
    }

    pub fn support_claim(&self) -> SupportClaim {
        SupportClaim::HarnessOnly
    }
}

pub fn run_post_send_harness(scenario: HarnessScenario) -> HarnessOutcome {
    let detected_at = Instant::now();
    let mut overlay = OverlayStateMachine::default();
    overlay.on_send_detected(detected_at);
    let overlay_within_target =
        overlay.mark_shell_rendered(detected_at + Duration::from_millis(16));

    let mut pipeline =
        CritiquePipeline::new(MockProvider, CandidateBuffer::new(Duration::from_secs(5)));
    pipeline.ingest_candidate(&scenario.message, &scenario.app_id, detected_at);
    match pipeline.handle_post_send(
        PostSendEvent {
            app_id: scenario.app_id.clone(),
            detected_at,
        },
        &scenario.metadata,
        scenario.spelling_strength,
        scenario.sarcasm_strength,
    ) {
        PipelineOutcome::Submitted(result) => {
            overlay.apply_result(result.clone(), detected_at + Duration::from_millis(80));
            HarnessOutcome::FeedbackShown {
                result,
                overlay_within_target,
                support_claim: scenario.support_claim(),
            }
        }
        PipelineOutcome::Excluded { reasons } => HarnessOutcome::Excluded {
            reasons,
            support_claim: scenario.support_claim(),
        },
        PipelineOutcome::NoCandidate => HarnessOutcome::NoCandidate,
        PipelineOutcome::ProviderFailed(_) => HarnessOutcome::ProviderFailed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_shows_feedback_without_claiming_live_adapter_support() {
        let outcome = run_post_send_harness(HarnessScenario::discord_typo("그렇게 하면 되요"));
        match outcome {
            HarnessOutcome::FeedbackShown {
                result,
                overlay_within_target,
                support_claim,
            } => {
                assert_eq!(result.corrected, "그렇게 하면 돼요");
                assert!(overlay_within_target);
                assert_eq!(support_claim, SupportClaim::HarnessOnly);
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn harness_excludes_sensitive_payment_like_context() {
        let mut scenario = HarnessScenario::discord_typo("4111 1111 1111 1111");
        scenario.metadata = ElementMetadata {
            app_id: Some("browser".to_owned()),
            label: Some("카드 번호".to_owned()),
            control_type: Some("edit".to_owned()),
            is_password: Some(false),
            ..ElementMetadata::default()
        };
        match run_post_send_harness(scenario) {
            HarnessOutcome::Excluded { reasons, .. } => {
                assert!(reasons
                    .iter()
                    .any(|r| r.contains("카드") || r.contains("card")));
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
}
