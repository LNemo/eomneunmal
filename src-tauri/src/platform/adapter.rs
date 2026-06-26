use std::fmt;
use std::time::Instant;

use crate::core::sensitivity::ElementMetadata;

use super::probe::{PermissionState, TargetApp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendSignalSource {
    EnterKey,
    SendButton,
    SyntheticHarness,
}

impl SendSignalSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::EnterKey => "enter-key",
            Self::SendButton => "send-button",
            Self::SyntheticHarness => "synthetic-harness",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAcquisitionMethod {
    AxFocusedText,
    InMemoryFallback,
}

impl TextAcquisitionMethod {
    pub fn label(self) -> &'static str {
        match self {
            Self::AxFocusedText => "ax-focused-text",
            Self::InMemoryFallback => "in-memory-fallback",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CandidateText {
    text: String,
    method: TextAcquisitionMethod,
}

impl CandidateText {
    pub fn new(text: impl Into<String>, method: TextAcquisitionMethod) -> Self {
        Self {
            text: text.into(),
            method,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn into_text(self) -> String {
        self.text
    }

    pub fn method(&self) -> TextAcquisitionMethod {
        self.method
    }
}

impl fmt::Debug for CandidateText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CandidateText")
            .field("text", &"<redacted>")
            .field("method", &self.method)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendLikeEvent {
    pub target: TargetApp,
    pub app_id: String,
    pub detected_at: Instant,
    pub source: SendSignalSource,
}

#[derive(Clone, PartialEq, Eq)]
pub struct AdapterContext {
    pub target: TargetApp,
    pub app_id: String,
    pub window_title_hash: Option<String>,
    pub metadata: ElementMetadata,
}

impl AdapterContext {
    pub fn new(
        target: TargetApp,
        app_id: impl Into<String>,
        window_title_hash: Option<String>,
        metadata: ElementMetadata,
    ) -> Self {
        Self {
            target,
            app_id: app_id.into(),
            window_title_hash,
            metadata,
        }
    }
}

impl fmt::Debug for AdapterContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdapterContext")
            .field("target", &self.target)
            .field("app_id", &self.app_id)
            .field("window_title_hash", &self.window_title_hash)
            .field("metadata", &metadata_debug_summary(&self.metadata))
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AdapterDecision {
    Ready {
        event: SendLikeEvent,
        context: AdapterContext,
        candidate: CandidateText,
    },
    Excluded {
        context: AdapterContext,
        reasons: Vec<String>,
    },
    Unavailable {
        target: TargetApp,
        reason: String,
    },
}

impl AdapterDecision {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    pub fn candidate(&self) -> Option<&CandidateText> {
        match self {
            Self::Ready { candidate, .. } => Some(candidate),
            _ => None,
        }
    }
}

impl fmt::Debug for AdapterDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready {
                event,
                context,
                candidate,
            } => f
                .debug_struct("AdapterDecision::Ready")
                .field("event", event)
                .field("context", context)
                .field("candidate", candidate)
                .finish(),
            Self::Excluded { context, reasons } => f
                .debug_struct("AdapterDecision::Excluded")
                .field("context", context)
                .field("reasons", reasons)
                .finish(),
            Self::Unavailable { target, reason } => f
                .debug_struct("AdapterDecision::Unavailable")
                .field("target", target)
                .field("reason", reason)
                .finish(),
        }
    }
}

pub trait LivePostSendAdapter {
    fn target_app(&self) -> TargetApp;
    fn permission_state(&self) -> PermissionState;
    fn observe_candidate(&mut self, text: String, app_id: String, observed_at: Instant);
    fn prepare_post_send(
        &mut self,
        context: AdapterContext,
        source: SendSignalSource,
        detected_at: Instant,
    ) -> AdapterDecision;
}

fn metadata_debug_summary(metadata: &ElementMetadata) -> String {
    format!(
        "app={:?}; role={:?}; control={:?}; password={:?}; protected={:?}; denylisted={}",
        metadata.app_id,
        metadata.role,
        metadata.control_type,
        metadata.is_password,
        metadata.is_protected,
        metadata.user_denylisted
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_debug_redacts_text() {
        let candidate =
            CandidateText::new("그렇게 하면 되요", TextAcquisitionMethod::InMemoryFallback);
        let debug = format!("{candidate:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("그렇게"));
    }

    #[test]
    fn context_debug_does_not_print_label_or_placeholder() {
        let context = AdapterContext::new(
            TargetApp::Discord,
            "com.hnc.Discord",
            Some("title-hash".to_owned()),
            ElementMetadata {
                app_id: Some("com.hnc.Discord".to_owned()),
                label: Some("Message #private-name".to_owned()),
                placeholder: Some("개인 채널 메시지".to_owned()),
                role: Some("text_input".to_owned()),
                control_type: Some("edit".to_owned()),
                is_password: Some(false),
                is_protected: Some(false),
                ..ElementMetadata::default()
            },
        );
        let debug = format!("{context:?}");
        assert!(debug.contains("title-hash"));
        assert!(!debug.contains("private-name"));
        assert!(!debug.contains("개인 채널"));
    }
}
