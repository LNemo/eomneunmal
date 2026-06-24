use std::time::Instant;

use super::candidate::{CandidateBuffer, ClearReason};
use super::provider::{CritiqueProvider, ProviderError};
use super::sensitivity::{ElementMetadata, SensitiveClassifier, SensitivityAction};
use super::types::{CritiqueRequest, CritiqueResult, SarcasmStrength, SpellingStrength};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostSendEvent {
    pub app_id: String,
    pub detected_at: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineOutcome {
    Submitted(CritiqueResult),
    Excluded { reasons: Vec<String> },
    NoCandidate,
    ProviderFailed(ProviderError),
}

#[derive(Debug)]
pub struct CritiquePipeline<P> {
    classifier: SensitiveClassifier,
    provider: P,
    pub candidate_buffer: CandidateBuffer,
}

impl<P: CritiqueProvider> CritiquePipeline<P> {
    pub fn new(provider: P, candidate_buffer: CandidateBuffer) -> Self {
        Self {
            classifier: SensitiveClassifier::default(),
            provider,
            candidate_buffer,
        }
    }

    pub fn ingest_candidate(
        &mut self,
        text: impl Into<String>,
        app_id: impl Into<String>,
        now: Instant,
    ) {
        self.candidate_buffer.replace(text, app_id, now);
    }

    pub fn handle_post_send(
        &mut self,
        event: PostSendEvent,
        metadata: &ElementMetadata,
        spelling_strength: SpellingStrength,
        sarcasm_strength: SarcasmStrength,
    ) -> PipelineOutcome {
        let decision = self.classifier.classify(metadata);
        if decision.action != SensitivityAction::Allow {
            self.candidate_buffer.clear(match decision.action {
                SensitivityAction::Allow => ClearReason::Completed,
                SensitivityAction::Exclude => ClearReason::Sensitive,
                SensitivityAction::ManualReview => ClearReason::Sensitive,
            });
            return PipelineOutcome::Excluded {
                reasons: decision.reasons,
            };
        }

        let Some(candidate) = self.candidate_buffer.take_current(event.detected_at) else {
            return PipelineOutcome::NoCandidate;
        };

        if candidate.app_id() != event.app_id {
            self.candidate_buffer.clear(ClearReason::AppChanged);
            return PipelineOutcome::NoCandidate;
        }

        let request = CritiqueRequest::new(candidate.text(), spelling_strength, sarcasm_strength);
        match self.provider.critique(&request) {
            Ok(result) => {
                self.candidate_buffer.clear(ClearReason::Completed);
                PipelineOutcome::Submitted(result)
            }
            Err(error) => {
                self.candidate_buffer.clear(ClearReason::Completed);
                PipelineOutcome::ProviderFailed(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::provider::MockProvider;
    use std::time::Duration;

    #[test]
    fn post_send_allows_chat_candidate_and_clears_after_submit() {
        let now = Instant::now();
        let mut pipeline =
            CritiquePipeline::new(MockProvider, CandidateBuffer::new(Duration::from_secs(5)));
        pipeline.ingest_candidate("그렇게 하면 되요", "discord", now);
        let outcome = pipeline.handle_post_send(
            PostSendEvent {
                app_id: "discord".to_owned(),
                detected_at: now,
            },
            &ElementMetadata::chat_input("discord", "Message #general"),
            SpellingStrength::Medium,
            SarcasmStrength::Medium,
        );
        match outcome {
            PipelineOutcome::Submitted(result) => assert_eq!(result.corrected, "그렇게 하면 돼요"),
            other => panic!("unexpected outcome: {other:?}"),
        }
        assert!(pipeline.candidate_buffer.is_empty());
    }

    #[test]
    fn sensitive_metadata_prevents_provider_submission_and_clears() {
        let now = Instant::now();
        let mut pipeline =
            CritiquePipeline::new(MockProvider, CandidateBuffer::new(Duration::from_secs(5)));
        pipeline.ingest_candidate("4111 1111 1111 1111", "browser", now);
        let outcome = pipeline.handle_post_send(
            PostSendEvent {
                app_id: "browser".to_owned(),
                detected_at: now,
            },
            &ElementMetadata {
                label: Some("카드 번호".to_owned()),
                control_type: Some("edit".to_owned()),
                ..ElementMetadata::default()
            },
            SpellingStrength::Weak,
            SarcasmStrength::Weak,
        );
        assert!(matches!(outcome, PipelineOutcome::Excluded { .. }));
        assert!(pipeline.candidate_buffer.is_empty());
    }
}
