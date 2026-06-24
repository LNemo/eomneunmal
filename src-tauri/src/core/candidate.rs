use std::fmt;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearReason {
    Completed,
    Timeout,
    AppChanged,
    Disabled,
    Sensitive,
    Manual,
}

#[derive(Clone, PartialEq, Eq)]
pub struct CandidateSnapshot {
    text: String,
    app_id: String,
    created_at: Instant,
}

impl CandidateSnapshot {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    pub fn age_at(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.created_at)
    }
}

impl fmt::Debug for CandidateSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CandidateSnapshot")
            .field("text", &"<redacted>")
            .field("app_id", &self.app_id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub struct CandidateBuffer {
    candidate: Option<CandidateSnapshot>,
    ttl: Duration,
    last_clear_reason: Option<ClearReason>,
}

impl CandidateBuffer {
    pub fn new(ttl: Duration) -> Self {
        Self {
            candidate: None,
            ttl,
            last_clear_reason: None,
        }
    }

    pub fn replace(&mut self, text: impl Into<String>, app_id: impl Into<String>, now: Instant) {
        self.candidate = Some(CandidateSnapshot {
            text: text.into(),
            app_id: app_id.into(),
            created_at: now,
        });
        self.last_clear_reason = None;
    }

    pub fn current(&self, now: Instant) -> Option<&CandidateSnapshot> {
        self.candidate
            .as_ref()
            .filter(|candidate| candidate.age_at(now) <= self.ttl)
    }

    pub fn take_current(&mut self, now: Instant) -> Option<CandidateSnapshot> {
        if self.current(now).is_some() {
            self.candidate.take()
        } else {
            if self.candidate.is_some() {
                self.clear(ClearReason::Timeout);
            }
            None
        }
    }

    pub fn clear(&mut self, reason: ClearReason) {
        self.candidate = None;
        self.last_clear_reason = Some(reason);
    }

    pub fn last_clear_reason(&self) -> Option<ClearReason> {
        self.last_clear_reason
    }

    pub fn is_empty(&self) -> bool {
        self.candidate.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_debug_redacts_raw_text() {
        let now = Instant::now();
        let mut buffer = CandidateBuffer::new(Duration::from_secs(1));
        buffer.replace("비밀 아닌 메시지", "discord", now);
        let debug = format!("{:?}", buffer.current(now).unwrap());
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("비밀 아닌 메시지"));
    }

    #[test]
    fn candidate_expires_and_clears() {
        let now = Instant::now();
        let mut buffer = CandidateBuffer::new(Duration::from_millis(10));
        buffer.replace("안뇽", "discord", now);
        assert!(buffer.current(now + Duration::from_millis(5)).is_some());
        assert!(buffer
            .take_current(now + Duration::from_millis(11))
            .is_none());
        assert_eq!(buffer.last_clear_reason(), Some(ClearReason::Timeout));
        assert!(buffer.is_empty());
    }

    #[test]
    fn take_current_removes_raw_text_after_use() {
        let now = Instant::now();
        let mut buffer = CandidateBuffer::new(Duration::from_secs(1));
        buffer.replace("되요", "kakaotalk", now);
        let candidate = buffer.take_current(now).unwrap();
        assert_eq!(candidate.text(), "되요");
        assert!(buffer.is_empty());
    }
}
