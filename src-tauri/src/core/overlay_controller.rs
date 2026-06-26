use std::fmt;
use std::time::{Duration, Instant};

use serde::Serialize;

use super::overlay::OverlayStateMachine;
use super::types::CritiqueResult;

pub const OVERLAY_STATE_EVENT: &str = "overlay://state";

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayViewModel {
    pub phase: String,
    pub title: String,
    pub body: String,
    pub within_render_target: Option<bool>,
    pub auto_dismiss_ms: u64,
}

impl fmt::Debug for OverlayViewModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OverlayViewModel")
            .field("phase", &self.phase)
            .field("title", &"<redacted>")
            .field("body", &"<redacted>")
            .field("within_render_target", &self.within_render_target)
            .field("auto_dismiss_ms", &self.auto_dismiss_ms)
            .finish()
    }
}

impl OverlayViewModel {
    fn loading(auto_dismiss_ms: u64) -> Self {
        Self {
            phase: "loading".to_owned(),
            title: "전송 감지".to_owned(),
            body: "없는말이 맞춤법을 씹을 준비 중...".to_owned(),
            within_render_target: None,
            auto_dismiss_ms,
        }
    }

    fn rendered_loading(within_target: bool, auto_dismiss_ms: u64) -> Self {
        Self {
            phase: "loading".to_owned(),
            title: "검사 중".to_owned(),
            body: if within_target {
                "300ms 목표 안에 표시됨".to_owned()
            } else {
                "300ms 목표 초과".to_owned()
            },
            within_render_target: Some(within_target),
            auto_dismiss_ms,
        }
    }

    fn result(result: &CritiqueResult, auto_dismiss_ms: u64) -> Self {
        Self {
            phase: "result".to_owned(),
            title: "맞춤법 지적".to_owned(),
            body: format!("{} — {}", result.explanation, result.roast),
            within_render_target: None,
            auto_dismiss_ms,
        }
    }

    fn dismissed(auto_dismiss_ms: u64) -> Self {
        Self {
            phase: "dismissed".to_owned(),
            title: "닫힘".to_owned(),
            body: String::new(),
            within_render_target: None,
            auto_dismiss_ms,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayRunReport {
    pub loading_view: OverlayViewModel,
    pub rendered_loading_view: OverlayViewModel,
    pub result_view: Option<OverlayViewModel>,
    pub within_render_target: bool,
}

impl fmt::Debug for OverlayRunReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OverlayRunReport")
            .field("loading_view", &self.loading_view)
            .field("rendered_loading_view", &self.rendered_loading_view)
            .field("result_view", &self.result_view)
            .field("within_render_target", &self.within_render_target)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayRenderReport {
    pub loading_view: OverlayViewModel,
    pub rendered_loading_view: OverlayViewModel,
    pub within_render_target: bool,
}

pub trait OverlayPresenter {
    fn show(&mut self, view: &OverlayViewModel) -> Result<(), String>;
    fn update(&mut self, view: &OverlayViewModel) -> Result<(), String>;
    fn hide(&mut self, view: &OverlayViewModel) -> Result<(), String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayController {
    state: OverlayStateMachine,
    auto_dismiss_after: Duration,
}

impl OverlayController {
    pub fn new(auto_dismiss_after: Duration) -> Self {
        Self {
            state: OverlayStateMachine::default(),
            auto_dismiss_after,
        }
    }

    pub fn set_auto_dismiss_after(&mut self, auto_dismiss_after: Duration) {
        self.auto_dismiss_after = auto_dismiss_after;
    }

    pub fn auto_dismiss_ms(&self) -> u64 {
        self.auto_dismiss_after
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX)
    }

    pub fn show_loading(
        &mut self,
        presenter: &mut impl OverlayPresenter,
        detected_at: Instant,
        rendered_at: Instant,
    ) -> Result<OverlayRenderReport, String> {
        self.state.on_send_detected(detected_at);

        let loading_view = OverlayViewModel::loading(self.auto_dismiss_ms());
        presenter.show(&loading_view)?;

        let within_render_target = self.state.mark_shell_rendered(rendered_at);
        let rendered_loading_view =
            OverlayViewModel::rendered_loading(within_render_target, self.auto_dismiss_ms());
        presenter.update(&rendered_loading_view)?;

        Ok(OverlayRenderReport {
            loading_view,
            rendered_loading_view,
            within_render_target,
        })
    }

    pub fn apply_result(
        &mut self,
        presenter: &mut impl OverlayPresenter,
        result: CritiqueResult,
        rendered_at: Instant,
    ) -> Result<OverlayViewModel, String> {
        self.state.apply_result(result.clone(), rendered_at);
        let view = OverlayViewModel::result(&result, self.auto_dismiss_ms());
        presenter.update(&view)?;
        Ok(view)
    }

    pub fn dismiss(&mut self, presenter: &mut impl OverlayPresenter) -> Result<(), String> {
        self.state.dismiss();
        presenter.hide(&OverlayViewModel::dismissed(self.auto_dismiss_ms()))
    }

    pub fn dismiss_if_due(
        &mut self,
        presenter: &mut impl OverlayPresenter,
        now: Instant,
    ) -> Result<bool, String> {
        if self.state.should_auto_dismiss(now, self.auto_dismiss_after) {
            self.dismiss(presenter)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl Default for OverlayController {
    fn default() -> Self {
        Self::new(Duration::from_millis(8000))
    }
}

pub fn fallback_result_view(
    explanation: impl Into<String>,
    roast: impl Into<String>,
    auto_dismiss_ms: u64,
) -> OverlayViewModel {
    OverlayViewModel::result(
        &CritiqueResult::new("", explanation, roast),
        auto_dismiss_ms,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::candidate::CandidateBuffer;
    use crate::core::overlay::OVERLAY_RENDER_TARGET;
    use crate::core::pipeline::{CritiquePipeline, PipelineOutcome, PostSendEvent};
    use crate::core::provider::MockProvider;
    use crate::core::sensitivity::ElementMetadata;
    use crate::core::types::{SarcasmStrength, SpellingStrength};

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

    #[test]
    fn controller_shows_updates_and_hides_without_focus_command() {
        let now = Instant::now();
        let mut presenter = RecordingPresenter::default();
        let mut controller = OverlayController::new(Duration::from_secs(8));

        let report = controller
            .show_loading(
                &mut presenter,
                now,
                now + OVERLAY_RENDER_TARGET - Duration::from_millis(1),
            )
            .unwrap();
        assert!(report.within_render_target);

        controller
            .apply_result(
                &mut presenter,
                CritiqueResult::new("돼요", "되요 → 돼요", "그 정도는 외우자."),
                now + Duration::from_secs(1),
            )
            .unwrap();
        assert!(!controller
            .dismiss_if_due(&mut presenter, now + Duration::from_secs(4))
            .unwrap());
        assert!(controller
            .dismiss_if_due(&mut presenter, now + Duration::from_secs(10))
            .unwrap());

        let actions: Vec<_> = presenter
            .actions
            .iter()
            .map(|(action, _)| *action)
            .collect();
        assert_eq!(actions, vec!["show", "update", "update", "hide"]);
    }

    #[test]
    fn mock_post_send_drives_pipeline_to_overlay_result() {
        let now = Instant::now();
        let mut presenter = RecordingPresenter::default();
        let mut controller = OverlayController::new(Duration::from_secs(8));
        let mut pipeline =
            CritiquePipeline::new(MockProvider, CandidateBuffer::new(Duration::from_secs(5)));

        pipeline.ingest_candidate("그렇게 하면 되요", "discord", now);
        let render = controller
            .show_loading(&mut presenter, now, now + Duration::from_millis(12))
            .unwrap();
        assert!(render.within_render_target);

        let outcome = pipeline.handle_post_send(
            PostSendEvent {
                app_id: "discord".to_owned(),
                detected_at: now,
            },
            &ElementMetadata::chat_input("discord", "Message #general"),
            SpellingStrength::Medium,
            SarcasmStrength::Medium,
        );
        let PipelineOutcome::Submitted(result) = outcome else {
            panic!("unexpected outcome: {outcome:?}");
        };

        let view = controller
            .apply_result(&mut presenter, result, now + Duration::from_millis(30))
            .unwrap();
        assert_eq!(view.phase, "result");
        assert!(view.body.contains("세종대왕"));
        assert!(pipeline.candidate_buffer.is_empty());
    }

    #[test]
    fn overlay_debug_redacts_user_text() {
        let raw = "그렇게 하면 되요";
        let view = OverlayViewModel::result(
            &CritiqueResult::new("그렇게 하면 돼요", "mock: 교정했습니다.", "살짝 한숨."),
            8000,
        );
        let report = OverlayRunReport {
            loading_view: OverlayViewModel::loading(8000),
            rendered_loading_view: OverlayViewModel::rendered_loading(true, 8000),
            result_view: Some(view),
            within_render_target: true,
        };

        let debug = format!("{report:?}");
        assert!(!debug.contains(raw));
        assert!(!debug.contains("그렇게 하면 돼요"));
        assert!(debug.contains("<redacted>"));
    }
}
