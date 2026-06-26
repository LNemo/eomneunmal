use std::fmt;
use std::time::Instant;

use crate::core::overlay_controller::{
    fallback_result_view, OverlayController, OverlayPresenter, OverlayRunReport,
};
use crate::core::pipeline::{CritiquePipeline, PipelineOutcome, PostSendEvent};
use crate::core::provider::CritiqueProvider;
use crate::core::types::{SarcasmStrength, SpellingStrength};

use super::adapter::AdapterDecision;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterSupportClaim {
    SimulatedAdapterOnly,
}

#[derive(Clone, PartialEq, Eq)]
pub enum LivePipelineReport {
    FeedbackShown {
        overlay: Box<OverlayRunReport>,
        provider_submitted: bool,
        support_claim: AdapterSupportClaim,
    },
    Excluded {
        reasons: Vec<String>,
        provider_submitted: bool,
        support_claim: AdapterSupportClaim,
    },
    Unavailable {
        reason: String,
        provider_submitted: bool,
        support_claim: AdapterSupportClaim,
    },
    ProviderFailed {
        provider_submitted: bool,
        support_claim: AdapterSupportClaim,
    },
}

impl fmt::Debug for LivePipelineReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FeedbackShown {
                overlay,
                provider_submitted,
                support_claim,
            } => f
                .debug_struct("LivePipelineReport::FeedbackShown")
                .field("overlay", overlay)
                .field("provider_submitted", provider_submitted)
                .field("support_claim", support_claim)
                .finish(),
            Self::Excluded {
                reasons,
                provider_submitted,
                support_claim,
            } => f
                .debug_struct("LivePipelineReport::Excluded")
                .field("reasons", reasons)
                .field("provider_submitted", provider_submitted)
                .field("support_claim", support_claim)
                .finish(),
            Self::Unavailable {
                reason,
                provider_submitted,
                support_claim,
            } => f
                .debug_struct("LivePipelineReport::Unavailable")
                .field("reason", reason)
                .field("provider_submitted", provider_submitted)
                .field("support_claim", support_claim)
                .finish(),
            Self::ProviderFailed {
                provider_submitted,
                support_claim,
            } => f
                .debug_struct("LivePipelineReport::ProviderFailed")
                .field("provider_submitted", provider_submitted)
                .field("support_claim", support_claim)
                .finish(),
        }
    }
}

impl LivePipelineReport {
    pub fn overlay(&self) -> Option<&OverlayRunReport> {
        match self {
            Self::FeedbackShown { overlay, .. } => Some(overlay.as_ref()),
            _ => None,
        }
    }
}

pub fn run_adapter_decision_with_overlay<P: CritiqueProvider>(
    pipeline: &mut CritiquePipeline<P>,
    overlay: &mut OverlayController,
    presenter: &mut impl OverlayPresenter,
    decision: AdapterDecision,
    spelling_strength: SpellingStrength,
    sarcasm_strength: SarcasmStrength,
    timing: LivePipelineTiming,
) -> Result<LivePipelineReport, String> {
    let support_claim = AdapterSupportClaim::SimulatedAdapterOnly;

    let (event, context, candidate) = match decision {
        AdapterDecision::Ready {
            event,
            context,
            candidate,
        } => (event, context, candidate),
        AdapterDecision::Excluded { reasons, .. } => {
            return Ok(LivePipelineReport::Excluded {
                reasons,
                provider_submitted: false,
                support_claim,
            });
        }
        AdapterDecision::Unavailable { reason, .. } => {
            return Ok(LivePipelineReport::Unavailable {
                reason,
                provider_submitted: false,
                support_claim,
            });
        }
    };

    let render = overlay.show_loading(presenter, event.detected_at, timing.shell_rendered_at)?;
    pipeline.ingest_candidate(candidate.as_str(), event.app_id.clone(), event.detected_at);

    let outcome = pipeline.handle_post_send(
        PostSendEvent {
            app_id: event.app_id,
            detected_at: event.detected_at,
        },
        &context.metadata,
        spelling_strength,
        sarcasm_strength,
    );

    match outcome {
        PipelineOutcome::Submitted(result) => {
            let result_view = overlay.apply_result(presenter, result, timing.result_rendered_at)?;
            Ok(LivePipelineReport::FeedbackShown {
                overlay: Box::new(OverlayRunReport {
                    loading_view: render.loading_view,
                    rendered_loading_view: render.rendered_loading_view,
                    result_view: Some(result_view),
                    within_render_target: render.within_render_target,
                }),
                provider_submitted: true,
                support_claim,
            })
        }
        PipelineOutcome::Excluded { reasons } => {
            let view = fallback_result_view(
                "민감하거나 알 수 없는 입력이라 제외했습니다.",
                reasons.join(" / "),
                overlay.auto_dismiss_ms(),
            );
            presenter.update(&view)?;
            Ok(LivePipelineReport::Excluded {
                reasons,
                provider_submitted: false,
                support_claim,
            })
        }
        PipelineOutcome::NoCandidate => Ok(LivePipelineReport::Unavailable {
            reason: "pipeline candidate buffer was empty".to_owned(),
            provider_submitted: false,
            support_claim,
        }),
        PipelineOutcome::ProviderFailed(_) => Ok(LivePipelineReport::ProviderFailed {
            provider_submitted: true,
            support_claim,
        }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LivePipelineTiming {
    pub shell_rendered_at: Instant,
    pub result_rendered_at: Instant,
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Duration;

    use crate::core::candidate::CandidateBuffer;
    use crate::core::overlay_controller::OverlayViewModel;
    use crate::core::provider::{MockProvider, ProviderError};
    use crate::core::types::{CritiqueRequest, CritiqueResult};
    use crate::platform::adapter::{LivePostSendAdapter, SendSignalSource};
    use crate::platform::macos::{
        MacDiscordAdapter, MacFocusedElementContext, MacOsPermissionSnapshot, DISCORD_BUNDLE_ID,
    };
    use crate::platform::probe::PermissionState;

    use super::*;

    #[derive(Default)]
    struct RecordingPresenter {
        actions: Vec<(&'static str, OverlayViewModel)>,
    }

    impl OverlayPresenter for RecordingPresenter {
        fn show(&mut self, view: &OverlayViewModel) -> Result<(), String> {
            self.actions.push(("show", view.clone()));
            Ok(())
        }

        fn update(&mut self, view: &OverlayViewModel) -> Result<(), String> {
            self.actions.push(("update", view.clone()));
            Ok(())
        }

        fn hide(&mut self, view: &OverlayViewModel) -> Result<(), String> {
            self.actions.push(("hide", view.clone()));
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct SpyProvider {
        requests: Rc<RefCell<Vec<CritiqueRequest>>>,
    }

    impl SpyProvider {
        fn requests(&self) -> Vec<CritiqueRequest> {
            self.requests.borrow().clone()
        }
    }

    impl CritiqueProvider for SpyProvider {
        fn critique(&self, request: &CritiqueRequest) -> Result<CritiqueResult, ProviderError> {
            self.requests.borrow_mut().push(request.clone());
            MockProvider.critique(request)
        }
    }

    #[test]
    fn simulated_discord_adapter_runs_pipeline_and_overlay_without_public_pass_claim() {
        let now = Instant::now();
        let mut adapter = MacDiscordAdapter::with_permissions(MacOsPermissionSnapshot {
            accessibility: PermissionState::Ready,
            input_monitoring: PermissionState::Unknown,
        });
        adapter.observe_candidate(
            "그렇게 하면 되요".to_owned(),
            DISCORD_BUNDLE_ID.to_owned(),
            now,
        );
        let decision = adapter.prepare_post_send(
            MacFocusedElementContext::discord_chat("없는말 safe channel").into(),
            SendSignalSource::EnterKey,
            now,
        );

        let provider = SpyProvider::default();
        let spy = provider.clone();
        let mut pipeline =
            CritiquePipeline::new(provider, CandidateBuffer::new(Duration::from_secs(5)));
        let mut overlay = OverlayController::new(Duration::from_secs(8));
        let mut presenter = RecordingPresenter::default();

        let report = run_adapter_decision_with_overlay(
            &mut pipeline,
            &mut overlay,
            &mut presenter,
            decision,
            SpellingStrength::Medium,
            SarcasmStrength::Weak,
            LivePipelineTiming {
                shell_rendered_at: now + Duration::from_millis(10),
                result_rendered_at: now + Duration::from_millis(60),
            },
        )
        .unwrap();

        match &report {
            LivePipelineReport::FeedbackShown {
                overlay,
                provider_submitted,
                support_claim,
            } => {
                assert!(*provider_submitted);
                assert_eq!(*support_claim, AdapterSupportClaim::SimulatedAdapterOnly);
                assert!(overlay.within_render_target);
                assert!(overlay.result_view.is_some());
            }
            other => panic!("unexpected live pipeline report: {other:?}"),
        }

        let requests = spy.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].message, "그렇게 하면 되요");
        assert!(!requests[0].message.contains("safe channel"));
        assert_eq!(presenter.actions.len(), 3);
        let debug = format!("{report:?}");
        assert!(!debug.contains("그렇게 하면 되요"));
        assert!(!debug.contains("safe channel"));
        assert!(debug.contains("SimulatedAdapterOnly"));
    }

    #[test]
    fn protected_adapter_decision_does_not_reach_provider_or_overlay() {
        let now = Instant::now();
        let mut adapter = MacDiscordAdapter::with_permissions(MacOsPermissionSnapshot {
            accessibility: PermissionState::Ready,
            input_monitoring: PermissionState::Ready,
        });
        adapter.observe_candidate("비밀번호123".to_owned(), DISCORD_BUNDLE_ID.to_owned(), now);
        let decision = adapter.prepare_post_send(
            MacFocusedElementContext::protected_discord_field().into(),
            SendSignalSource::EnterKey,
            now,
        );

        let provider = SpyProvider::default();
        let spy = provider.clone();
        let mut pipeline =
            CritiquePipeline::new(provider, CandidateBuffer::new(Duration::from_secs(5)));
        let mut overlay = OverlayController::new(Duration::from_secs(8));
        let mut presenter = RecordingPresenter::default();

        let report = run_adapter_decision_with_overlay(
            &mut pipeline,
            &mut overlay,
            &mut presenter,
            decision,
            SpellingStrength::Medium,
            SarcasmStrength::Weak,
            LivePipelineTiming {
                shell_rendered_at: now,
                result_rendered_at: now,
            },
        )
        .unwrap();

        match report {
            LivePipelineReport::Excluded {
                provider_submitted, ..
            } => assert!(!provider_submitted),
            other => panic!("expected exclusion, got {other:?}"),
        }
        assert!(spy.requests().is_empty());
        assert!(presenter.actions.is_empty());
    }
}
