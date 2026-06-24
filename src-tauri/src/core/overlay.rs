use std::time::{Duration, Instant};

use super::types::CritiqueResult;

pub const OVERLAY_RENDER_TARGET: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayPhase {
    Idle,
    Loading {
        detected_at: Instant,
        render_deadline: Instant,
    },
    RenderedLoading {
        detected_at: Instant,
        rendered_at: Instant,
        within_target: bool,
    },
    Result {
        result: CritiqueResult,
        rendered_at: Instant,
    },
    Dismissed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayStateMachine {
    phase: OverlayPhase,
}

impl Default for OverlayStateMachine {
    fn default() -> Self {
        Self {
            phase: OverlayPhase::Idle,
        }
    }
}

impl OverlayStateMachine {
    pub fn phase(&self) -> &OverlayPhase {
        &self.phase
    }

    pub fn on_send_detected(&mut self, detected_at: Instant) {
        self.phase = OverlayPhase::Loading {
            detected_at,
            render_deadline: detected_at + OVERLAY_RENDER_TARGET,
        };
    }

    pub fn mark_shell_rendered(&mut self, rendered_at: Instant) -> bool {
        let OverlayPhase::Loading {
            detected_at,
            render_deadline,
        } = self.phase
        else {
            return false;
        };
        let within_target = rendered_at <= render_deadline;
        self.phase = OverlayPhase::RenderedLoading {
            detected_at,
            rendered_at,
            within_target,
        };
        within_target
    }

    pub fn apply_result(&mut self, result: CritiqueResult, rendered_at: Instant) {
        self.phase = OverlayPhase::Result {
            result,
            rendered_at,
        };
    }

    pub fn dismiss(&mut self) {
        self.phase = OverlayPhase::Dismissed;
    }

    pub fn should_auto_dismiss(&self, now: Instant, auto_dismiss_after: Duration) -> bool {
        match &self.phase {
            OverlayPhase::Result { rendered_at, .. } => {
                now.saturating_duration_since(*rendered_at) >= auto_dismiss_after
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_render_within_300ms_passes() {
        let now = Instant::now();
        let mut overlay = OverlayStateMachine::default();
        overlay.on_send_detected(now);
        assert!(overlay.mark_shell_rendered(now + Duration::from_millis(250)));
        assert!(matches!(
            overlay.phase(),
            OverlayPhase::RenderedLoading {
                within_target: true,
                ..
            }
        ));
    }

    #[test]
    fn shell_render_after_300ms_fails_measurement() {
        let now = Instant::now();
        let mut overlay = OverlayStateMachine::default();
        overlay.on_send_detected(now);
        assert!(!overlay.mark_shell_rendered(now + Duration::from_millis(301)));
    }

    #[test]
    fn provider_result_updates_after_loading_and_auto_dismisses() {
        let now = Instant::now();
        let mut overlay = OverlayStateMachine::default();
        overlay.on_send_detected(now);
        overlay.mark_shell_rendered(now + Duration::from_millis(10));
        overlay.apply_result(
            CritiqueResult::new("돼요", "되요→돼요", "그 정도는 외우자."),
            now + Duration::from_secs(2),
        );
        assert!(!overlay.should_auto_dismiss(now + Duration::from_secs(3), Duration::from_secs(8)));
        assert!(overlay.should_auto_dismiss(now + Duration::from_secs(11), Duration::from_secs(8)));
    }
}
