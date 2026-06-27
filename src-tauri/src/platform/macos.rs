use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::core::sensitivity::{ElementMetadata, SensitiveClassifier, SensitivityAction};

use super::adapter::{
    AdapterContext, AdapterDecision, CandidateText, LivePostSendAdapter, SendLikeEvent,
    SendSignalSource, TextAcquisitionMethod,
};
use super::probe::{
    AdapterProbe, OperatingSystem, PermissionState, ProbeRow, ProbeStatus, TargetApp,
};

pub const DISCORD_BUNDLE_ID: &str = "com.hnc.Discord";
pub const KAKAOTALK_BUNDLE_ID: &str = "com.kakao.KakaoTalkMac";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacApplicationProbe {
    pub target: TargetApp,
    pub bundle_identifier: &'static str,
    pub candidate_paths: Vec<PathBuf>,
}

impl MacApplicationProbe {
    pub fn discord() -> Self {
        Self {
            target: TargetApp::Discord,
            bundle_identifier: DISCORD_BUNDLE_ID,
            candidate_paths: vec![PathBuf::from("/Applications/Discord.app")],
        }
    }

    pub fn kakaotalk() -> Self {
        Self {
            target: TargetApp::KakaoTalk,
            bundle_identifier: KAKAOTALK_BUNDLE_ID,
            candidate_paths: vec![PathBuf::from("/Applications/KakaoTalk.app")],
        }
    }

    pub fn installed_path(&self) -> Option<&Path> {
        self.candidate_paths
            .iter()
            .find(|path| path.join("Contents/Info.plist").exists())
            .map(PathBuf::as_path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacOsProbe {
    pub os_version: String,
    pub accessibility_enabled: PermissionState,
    pub input_monitoring: PermissionState,
    pub apps: Vec<MacApplicationProbe>,
}

impl Default for MacOsProbe {
    fn default() -> Self {
        Self {
            os_version: command_stdout("sw_vers", &["-productVersion"])
                .unwrap_or_else(|| "unknown".to_owned()),
            accessibility_enabled: accessibility_permission_state(),
            input_monitoring: PermissionState::Unknown,
            apps: vec![
                MacApplicationProbe::discord(),
                MacApplicationProbe::kakaotalk(),
            ],
        }
    }
}

impl AdapterProbe for MacOsProbe {
    fn probe_rows(&self) -> Vec<ProbeRow> {
        self.apps
            .iter()
            .map(|app| {
                let installed = app.installed_path();
                let app_version = installed
                    .and_then(read_bundle_short_version)
                    .unwrap_or_else(|| "not-installed".to_owned());
                let permissions = if installed.is_none() {
                    PermissionState::SetupRequired
                } else if self.accessibility_enabled == PermissionState::Ready {
                    // Input Monitoring cannot be fully proven from a non-interactive probe.
                    PermissionState::Unknown
                } else {
                    self.accessibility_enabled
                };
                ProbeRow {
                    os: OperatingSystem::MacOS,
                    app: app.target,
                    os_version: self.os_version.clone(),
                    app_version,
                    permissions,
                    input_method: "Korean IME".to_owned(),
                    send_signal: "Enter/send-button candidate; actual external send not automated".to_owned(),
                    text_acquisition_method: "Accessibility/AX focused text first; in-memory candidate fallback".to_owned(),
                    sensitive_exclusion_result: "core classifier harness pass; secure-field live probe pending".to_owned(),
                    status: if installed.is_some() {
                        ProbeStatus::Partial
                    } else {
                        ProbeStatus::Planned
                    },
                    evidence_notes: if installed.is_some() {
                        format!(
                            "bundle {} installed; AX={}; InputMonitoring=manual; no raw text captured; no external message sent",
                            app.bundle_identifier,
                            self.accessibility_enabled.label()
                        )
                    } else {
                        format!("bundle {} not installed on this host", app.bundle_identifier)
                    },
                }
            })
            .collect()
    }
}

pub fn run_macos_inventory_probe() -> Vec<ProbeRow> {
    MacOsProbe::default().probe_rows()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacOsPermissionSnapshot {
    pub accessibility: PermissionState,
    pub input_monitoring: PermissionState,
}

impl MacOsPermissionSnapshot {
    pub fn from_probe(probe: &MacOsProbe) -> Self {
        Self {
            accessibility: probe.accessibility_enabled,
            input_monitoring: probe.input_monitoring,
        }
    }

    pub fn adapter_permission_state(&self) -> PermissionState {
        match (self.accessibility, self.input_monitoring) {
            (PermissionState::Blocked, _) | (_, PermissionState::Blocked) => {
                PermissionState::Blocked
            }
            (PermissionState::SetupRequired, _) => PermissionState::SetupRequired,
            (PermissionState::Ready, PermissionState::Ready) => PermissionState::Ready,
            (PermissionState::Ready, PermissionState::Unknown) => PermissionState::Unknown,
            _ => PermissionState::Unknown,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct InMemoryCandidateFallback {
    candidate: Option<FallbackCandidate>,
    ttl: Duration,
}

impl fmt::Debug for InMemoryCandidateFallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InMemoryCandidateFallback")
            .field("candidate", &self.candidate.as_ref().map(|_| "<redacted>"))
            .field("ttl", &self.ttl)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
struct FallbackCandidate {
    app_id: String,
    text: CandidateText,
    observed_at: Instant,
}

impl fmt::Debug for FallbackCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FallbackCandidate")
            .field("app_id", &self.app_id)
            .field("text", &self.text)
            .field("observed_at", &self.observed_at)
            .finish()
    }
}

impl InMemoryCandidateFallback {
    fn new(ttl: Duration) -> Self {
        Self {
            candidate: None,
            ttl,
        }
    }

    fn replace(&mut self, text: String, app_id: String, observed_at: Instant) {
        self.candidate = Some(FallbackCandidate {
            app_id,
            text: CandidateText::new(text, TextAcquisitionMethod::InMemoryFallback),
            observed_at,
        });
    }

    fn take_for_app(&mut self, app_id: &str, now: Instant) -> Option<CandidateText> {
        let candidate = self.candidate.as_ref()?;
        let valid = candidate.app_id == app_id
            && now.saturating_duration_since(candidate.observed_at) <= self.ttl;
        if valid {
            self.candidate.take().map(|candidate| candidate.text)
        } else {
            self.clear();
            None
        }
    }

    fn clear(&mut self) {
        self.candidate = None;
    }

    fn is_empty(&self) -> bool {
        self.candidate.is_none()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct MacFocusedElementContext {
    pub target: TargetApp,
    pub app_id: String,
    pub window_title_hash: Option<String>,
    pub metadata: ElementMetadata,
}

impl fmt::Debug for MacFocusedElementContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MacFocusedElementContext")
            .field("target", &self.target)
            .field("app_id", &self.app_id)
            .field("window_title_hash", &self.window_title_hash)
            .field("role", &self.metadata.role)
            .field("control_type", &self.metadata.control_type)
            .field("label_present", &self.metadata.label.is_some())
            .field("placeholder_present", &self.metadata.placeholder.is_some())
            .field("is_password", &self.metadata.is_password)
            .field("is_protected", &self.metadata.is_protected)
            .finish()
    }
}

impl MacFocusedElementContext {
    pub fn discord_chat(window_title: &str) -> Self {
        Self {
            target: TargetApp::Discord,
            app_id: DISCORD_BUNDLE_ID.to_owned(),
            window_title_hash: Some(redacted_hash(window_title)),
            metadata: ElementMetadata {
                app_id: Some(DISCORD_BUNDLE_ID.to_owned()),
                window_title: Some("discord chat window".to_owned()),
                role: Some("AXTextArea".to_owned()),
                control_type: Some("edit".to_owned()),
                label: Some("Message".to_owned()),
                placeholder: Some("Message".to_owned()),
                is_password: Some(false),
                is_protected: Some(false),
                ..ElementMetadata::default()
            },
        }
    }

    pub fn protected_discord_field() -> Self {
        Self {
            target: TargetApp::Discord,
            app_id: DISCORD_BUNDLE_ID.to_owned(),
            window_title_hash: Some(redacted_hash("Discord Login")),
            metadata: ElementMetadata {
                app_id: Some(DISCORD_BUNDLE_ID.to_owned()),
                window_title: Some("discord login".to_owned()),
                role: Some("AXTextField".to_owned()),
                control_type: Some("edit".to_owned()),
                label: Some("password".to_owned()),
                is_password: Some(true),
                is_protected: Some(true),
                ..ElementMetadata::default()
            },
        }
    }

    pub fn kakaotalk_chat(window_title: &str) -> Self {
        Self {
            target: TargetApp::KakaoTalk,
            app_id: KAKAOTALK_BUNDLE_ID.to_owned(),
            window_title_hash: Some(redacted_hash(window_title)),
            metadata: ElementMetadata {
                app_id: Some(KAKAOTALK_BUNDLE_ID.to_owned()),
                window_title: Some("kakaotalk chat window".to_owned()),
                role: Some("AXTextArea".to_owned()),
                control_type: Some("edit".to_owned()),
                label: Some("KakaoTalk message".to_owned()),
                placeholder: Some("message".to_owned()),
                is_password: Some(false),
                is_protected: Some(false),
                ..ElementMetadata::default()
            },
        }
    }

    pub fn from_live_snapshot(snapshot: &MacLiveTextSnapshot) -> Option<Self> {
        if !snapshot.looks_like_editable_chat_input() {
            return None;
        }

        Some(Self {
            target: snapshot.target,
            app_id: snapshot.app_id.clone(),
            window_title_hash: snapshot.window_title_hash.clone(),
            metadata: ElementMetadata {
                app_id: Some(snapshot.app_id.clone()),
                window_title: None,
                role: Some(snapshot.role.clone()),
                control_type: Some("edit".to_owned()),
                label: snapshot.primary_label(),
                placeholder: snapshot.placeholder.clone(),
                is_password: Some(snapshot.is_password_like()),
                is_protected: Some(snapshot.is_protected),
                ..ElementMetadata::default()
            },
        })
    }
}

impl From<MacFocusedElementContext> for AdapterContext {
    fn from(value: MacFocusedElementContext) -> Self {
        AdapterContext::new(
            value.target,
            value.app_id,
            value.window_title_hash,
            value.metadata,
        )
    }
}

#[derive(Debug, Clone)]
struct MacChatAdapterCore {
    target: TargetApp,
    bundle_id: &'static str,
    permissions: MacOsPermissionSnapshot,
    fallback: InMemoryCandidateFallback,
    classifier: SensitiveClassifier,
}

impl MacChatAdapterCore {
    fn from_probe(target: TargetApp, bundle_id: &'static str, probe: &MacOsProbe) -> Self {
        Self::with_permissions(
            target,
            bundle_id,
            MacOsPermissionSnapshot::from_probe(probe),
        )
    }

    fn with_permissions(
        target: TargetApp,
        bundle_id: &'static str,
        permissions: MacOsPermissionSnapshot,
    ) -> Self {
        Self {
            target,
            bundle_id,
            permissions,
            fallback: InMemoryCandidateFallback::new(Duration::from_secs(5)),
            classifier: SensitiveClassifier::default(),
        }
    }

    fn fallback_is_empty(&self) -> bool {
        self.fallback.is_empty()
    }

    fn permission_state(&self) -> PermissionState {
        self.permissions.adapter_permission_state()
    }

    fn permissions_allow_spike(&self) -> bool {
        matches!(
            self.permission_state(),
            PermissionState::Ready | PermissionState::Unknown
        )
    }

    fn observe_candidate(&mut self, text: String, app_id: String, observed_at: Instant) {
        self.fallback.replace(text, app_id, observed_at);
    }

    fn prepare_post_send(
        &mut self,
        context: AdapterContext,
        source: SendSignalSource,
        detected_at: Instant,
    ) -> AdapterDecision {
        if !self.permissions_allow_spike() {
            self.fallback.clear();
            return AdapterDecision::Unavailable {
                target: self.target,
                reason: format!("permission state is {}", self.permission_state().label()),
            };
        }

        if context.app_id != self.bundle_id {
            self.fallback.clear();
            return AdapterDecision::Unavailable {
                target: self.target,
                reason: format!("foreground app is not {}", self.target.label()),
            };
        }

        let decision = self.classifier.classify(&context.metadata);
        if decision.action != SensitivityAction::Allow {
            self.fallback.clear();
            return AdapterDecision::Excluded {
                context,
                reasons: decision.reasons,
            };
        }

        let Some(candidate) = self.fallback.take_for_app(&context.app_id, detected_at) else {
            return AdapterDecision::Unavailable {
                target: self.target,
                reason: format!(
                    "no safe candidate text available from live {} watcher",
                    self.target.label()
                ),
            };
        };

        AdapterDecision::Ready {
            event: SendLikeEvent {
                target: self.target,
                app_id: context.app_id.clone(),
                detected_at,
                source,
            },
            context,
            candidate,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MacDiscordAdapter {
    core: MacChatAdapterCore,
}

impl MacDiscordAdapter {
    pub fn from_probe(probe: &MacOsProbe) -> Self {
        Self {
            core: MacChatAdapterCore::from_probe(TargetApp::Discord, DISCORD_BUNDLE_ID, probe),
        }
    }

    pub fn with_permissions(permissions: MacOsPermissionSnapshot) -> Self {
        Self {
            core: MacChatAdapterCore::with_permissions(
                TargetApp::Discord,
                DISCORD_BUNDLE_ID,
                permissions,
            ),
        }
    }

    pub fn fallback_is_empty(&self) -> bool {
        self.core.fallback_is_empty()
    }
}

impl LivePostSendAdapter for MacDiscordAdapter {
    fn target_app(&self) -> TargetApp {
        TargetApp::Discord
    }

    fn permission_state(&self) -> PermissionState {
        self.core.permission_state()
    }

    fn observe_candidate(&mut self, text: String, app_id: String, observed_at: Instant) {
        self.core.observe_candidate(text, app_id, observed_at);
    }

    fn prepare_post_send(
        &mut self,
        context: AdapterContext,
        source: SendSignalSource,
        detected_at: Instant,
    ) -> AdapterDecision {
        self.core.prepare_post_send(context, source, detected_at)
    }
}

#[derive(Debug, Clone)]
pub struct MacKakaoTalkAdapter {
    core: MacChatAdapterCore,
}

impl MacKakaoTalkAdapter {
    pub fn from_probe(probe: &MacOsProbe) -> Self {
        Self {
            core: MacChatAdapterCore::from_probe(TargetApp::KakaoTalk, KAKAOTALK_BUNDLE_ID, probe),
        }
    }

    pub fn with_permissions(permissions: MacOsPermissionSnapshot) -> Self {
        Self {
            core: MacChatAdapterCore::with_permissions(
                TargetApp::KakaoTalk,
                KAKAOTALK_BUNDLE_ID,
                permissions,
            ),
        }
    }

    pub fn fallback_is_empty(&self) -> bool {
        self.core.fallback_is_empty()
    }
}

impl LivePostSendAdapter for MacKakaoTalkAdapter {
    fn target_app(&self) -> TargetApp {
        TargetApp::KakaoTalk
    }

    fn permission_state(&self) -> PermissionState {
        self.core.permission_state()
    }

    fn observe_candidate(&mut self, text: String, app_id: String, observed_at: Instant) {
        self.core.observe_candidate(text, app_id, observed_at);
    }

    fn prepare_post_send(
        &mut self,
        context: AdapterContext,
        source: SendSignalSource,
        detected_at: Instant,
    ) -> AdapterDecision {
        self.core.prepare_post_send(context, source, detected_at)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct MacLiveTextSnapshot {
    pub target: TargetApp,
    pub app_id: String,
    pub role: String,
    pub is_protected: bool,
    pub text: String,
    pub captured_at: Instant,
    pub window_title_hash: Option<String>,
    pub description: Option<String>,
    pub title: Option<String>,
    pub help: Option<String>,
    pub placeholder: Option<String>,
    pub identifier_hash: Option<String>,
    pub chat_history_hash: Option<String>,
}

impl fmt::Debug for MacLiveTextSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MacLiveTextSnapshot")
            .field("target", &self.target)
            .field("app_id", &self.app_id)
            .field("role", &self.role)
            .field("is_protected", &self.is_protected)
            .field("text", &"<redacted>")
            .field("text_len", &self.text.chars().count())
            .field("captured_at", &self.captured_at)
            .field("window_title_hash", &self.window_title_hash)
            .field("description_present", &self.description.is_some())
            .field("title_present", &self.title.is_some())
            .field("help_present", &self.help.is_some())
            .field("placeholder_present", &self.placeholder.is_some())
            .field("identifier_hash", &self.identifier_hash)
            .field("chat_history_hash", &self.chat_history_hash)
            .finish()
    }
}

impl MacLiveTextSnapshot {
    pub fn new(
        target: TargetApp,
        app_id: impl Into<String>,
        role: impl Into<String>,
        is_protected: bool,
        text: impl Into<String>,
        captured_at: Instant,
    ) -> Self {
        Self {
            target,
            app_id: app_id.into(),
            role: role.into(),
            is_protected,
            text: text.into(),
            captured_at,
            window_title_hash: None,
            description: None,
            title: None,
            help: None,
            placeholder: None,
            identifier_hash: None,
            chat_history_hash: None,
        }
    }

    pub fn with_window_title_hash(mut self, window_title_hash: Option<String>) -> Self {
        self.window_title_hash = window_title_hash;
        self
    }

    pub fn with_accessibility_metadata(
        mut self,
        description: Option<String>,
        title: Option<String>,
        help: Option<String>,
        placeholder: Option<String>,
        identifier_hash: Option<String>,
        chat_history_hash: Option<String>,
    ) -> Self {
        self.description = description;
        self.title = title;
        self.help = help;
        self.placeholder = placeholder;
        self.identifier_hash = identifier_hash;
        self.chat_history_hash = chat_history_hash;
        self
    }

    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    pub fn looks_like_editable_chat_input(&self) -> bool {
        let role = self.role.to_lowercase();
        let metadata = [
            self.description.as_deref(),
            self.title.as_deref(),
            self.help.as_deref(),
            self.placeholder.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
        let message_context = ["message", "메시지", "chat", "채팅"]
            .iter()
            .any(|term| metadata.contains(term));
        !self.is_protected
            && matches!(self.target, TargetApp::Discord | TargetApp::KakaoTalk)
            && (role.contains("textarea") || role.contains("textfield") || role.contains("text"))
            && message_context
            && !self.is_password_like()
    }

    pub fn is_same_accessibility_field(&self, other: &Self) -> bool {
        self.target == other.target
            && self.app_id == other.app_id
            && self.role == other.role
            && self.window_title_hash == other.window_title_hash
            && self.description == other.description
            && self.title == other.title
            && self.help == other.help
            && self.placeholder == other.placeholder
            && self.identifier_hash == other.identifier_hash
    }

    fn primary_label(&self) -> Option<String> {
        self.description
            .clone()
            .or_else(|| self.title.clone())
            .or_else(|| self.help.clone())
    }

    fn is_password_like(&self) -> bool {
        let role = self.role.to_lowercase();
        if role.contains("secure") || role.contains("password") {
            return true;
        }
        [
            self.description.as_deref(),
            self.title.as_deref(),
            self.help.as_deref(),
            self.placeholder.as_deref(),
        ]
        .into_iter()
        .flatten()
        .map(str::to_lowercase)
        .any(|value| {
            value.contains("password") || value.contains("비밀번호") || value.contains("암호")
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct MacLivePostSendCandidate {
    pub target: TargetApp,
    pub app_id: String,
    pub text: String,
    pub context: MacFocusedElementContext,
    pub detected_at: Instant,
    pub source: SendSignalSource,
}

impl fmt::Debug for MacLivePostSendCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MacLivePostSendCandidate")
            .field("target", &self.target)
            .field("app_id", &self.app_id)
            .field("text", &"<redacted>")
            .field("text_len", &self.text.chars().count())
            .field("context", &self.context)
            .field("detected_at", &self.detected_at)
            .field("source", &self.source)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacLiveSendSignal {
    pub source: SendSignalSource,
    pub observed_at: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacPostSendTransitionDetector {
    pending: Option<MacLiveTextSnapshot>,
    latest_send_signal: Option<MacLiveSendSignal>,
    ttl: Duration,
    signal_ttl: Duration,
}

impl Default for MacPostSendTransitionDetector {
    fn default() -> Self {
        Self::new(Duration::from_secs(10))
    }
}

impl MacPostSendTransitionDetector {
    pub fn new(ttl: Duration) -> Self {
        Self {
            pending: None,
            latest_send_signal: None,
            ttl,
            signal_ttl: Duration::from_millis(1_500),
        }
    }

    pub fn has_pending_candidate(&self) -> bool {
        self.pending.is_some()
    }

    pub fn observe(
        &mut self,
        snapshot: Option<MacLiveTextSnapshot>,
        send_signal: Option<MacLiveSendSignal>,
    ) -> Option<MacLivePostSendCandidate> {
        if let Some(signal) = send_signal {
            self.latest_send_signal = Some(signal);
        }

        let snapshot = match snapshot {
            Some(snapshot) if snapshot.looks_like_editable_chat_input() => snapshot,
            _ => {
                self.pending = None;
                return None;
            }
        };

        if !snapshot.is_empty() {
            self.pending = Some(snapshot);
            return None;
        }

        let pending = self.pending.take()?;
        let same_field = pending.is_same_accessibility_field(&snapshot);
        let still_fresh = snapshot
            .captured_at
            .saturating_duration_since(pending.captured_at)
            <= self.ttl;
        let transcript_changed = pending.chat_history_hash.is_some()
            && snapshot.chat_history_hash.is_some()
            && pending.chat_history_hash != snapshot.chat_history_hash;
        let transcript_signal = transcript_changed.then_some(MacLiveSendSignal {
            source: SendSignalSource::ChatTranscriptChanged,
            observed_at: snapshot.captured_at,
        });
        let send_signal = self.latest_send_signal.take();
        let send_signal = send_signal
            .filter(|signal| {
                signal.observed_at >= pending.captured_at
                    && snapshot
                        .captured_at
                        .saturating_duration_since(signal.observed_at)
                        <= self.signal_ttl
            })
            .or(transcript_signal);
        if !same_field || !still_fresh || pending.is_empty() || send_signal.is_none() {
            return None;
        }
        let send_signal = send_signal.expect("checked is_some above");

        let context = MacFocusedElementContext::from_live_snapshot(&snapshot)?;
        Some(MacLivePostSendCandidate {
            target: pending.target,
            app_id: pending.app_id,
            text: pending.text,
            context,
            detected_at: snapshot.captured_at,
            source: send_signal.source,
        })
    }
}

pub fn capture_focused_text_snapshot(captured_at: Instant) -> Option<MacLiveTextSnapshot> {
    capture_focused_text_snapshot_impl(captured_at, false)
}

pub fn capture_focused_text_snapshot_with_chat_history(
    captured_at: Instant,
) -> Option<MacLiveTextSnapshot> {
    capture_focused_text_snapshot_impl(captured_at, true)
}

pub fn accessibility_trusted_for_current_process() -> bool {
    accessibility_trusted_for_current_process_impl()
}

pub fn request_accessibility_trust_prompt() -> bool {
    request_accessibility_trust_prompt_impl()
}

pub fn start_enter_key_event_monitor() -> bool {
    start_enter_key_event_monitor_impl()
}

pub fn take_enter_key_event_signal() -> Option<MacLiveSendSignal> {
    take_enter_key_event_signal_impl()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MacEnterKeyStateTracker {
    was_down: bool,
}

impl MacEnterKeyStateTracker {
    pub fn observe(&mut self, observed_at: Instant) -> Option<MacLiveSendSignal> {
        let is_down = enter_key_is_down_impl();
        let signal = if is_down && !self.was_down {
            Some(MacLiveSendSignal {
                source: SendSignalSource::EnterKey,
                observed_at,
            })
        } else {
            None
        };
        self.was_down = is_down;
        signal
    }
}

#[cfg(not(target_os = "macos"))]
fn capture_focused_text_snapshot_impl(
    _captured_at: Instant,
    _include_chat_history: bool,
) -> Option<MacLiveTextSnapshot> {
    None
}

#[cfg(not(target_os = "macos"))]
fn accessibility_trusted_for_current_process_impl() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
fn request_accessibility_trust_prompt_impl() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
fn start_enter_key_event_monitor_impl() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
fn take_enter_key_event_signal_impl() -> Option<MacLiveSendSignal> {
    None
}

#[cfg(not(target_os = "macos"))]
fn enter_key_is_down_impl() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn capture_focused_text_snapshot_impl(
    captured_at: Instant,
    include_chat_history: bool,
) -> Option<MacLiveTextSnapshot> {
    macos_ax::capture_focused_text_snapshot(captured_at, include_chat_history)
}

#[cfg(target_os = "macos")]
fn accessibility_trusted_for_current_process_impl() -> bool {
    macos_ax::accessibility_trusted()
}

#[cfg(target_os = "macos")]
fn request_accessibility_trust_prompt_impl() -> bool {
    macos_ax::request_accessibility_trust_prompt()
}

#[cfg(target_os = "macos")]
fn start_enter_key_event_monitor_impl() -> bool {
    macos_ax::start_enter_key_event_monitor()
}

#[cfg(target_os = "macos")]
fn take_enter_key_event_signal_impl() -> Option<MacLiveSendSignal> {
    macos_ax::take_enter_key_event_signal()
}

#[cfg(target_os = "macos")]
fn enter_key_is_down_impl() -> bool {
    macos_ax::enter_key_is_down()
}

#[cfg(target_os = "macos")]
mod macos_ax {
    use std::os::raw::{c_int, c_void};
    use std::ptr;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, OnceLock};

    use core_foundation::array::CFArray;
    use core_foundation::base::{CFType, CFTypeRef, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::string::{CFString, CFStringRef};
    use objc2_app_kit::NSWorkspace;

    use super::{
        redacted_hash, MacLiveSendSignal, MacLiveTextSnapshot, SendSignalSource, TargetApp,
        DISCORD_BUNDLE_ID, KAKAOTALK_BUNDLE_ID,
    };

    type AXError = c_int;
    type AXUIElementRef = *const c_void;
    type CFMachPortRef = *const c_void;
    type CFRunLoopRef = *const c_void;
    type CFRunLoopSourceRef = *const c_void;
    type CGEventRef = *mut c_void;
    type CGEventTapProxy = *mut c_void;
    type CGEventType = u32;
    type CGEventField = u32;
    type CGEventMask = u64;
    type CGEventTapCallBack = extern "C" fn(
        proxy: CGEventTapProxy,
        event_type: CGEventType,
        event: CGEventRef,
        user_info: *mut c_void,
    ) -> CGEventRef;

    const AX_ERROR_SUCCESS: AXError = 0;
    const KCG_EVENT_SOURCE_STATE_HID_SYSTEM_STATE: c_int = 1;
    const KCG_SESSION_EVENT_TAP: u32 = 1;
    const KCG_HEAD_INSERT_EVENT_TAP: u32 = 0;
    const KCG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
    const KCG_EVENT_KEY_DOWN: CGEventType = 10;
    const KCG_EVENT_TAP_DISABLED_BY_TIMEOUT: CGEventType = 0xFFFF_FFFE;
    const KCG_EVENT_TAP_DISABLED_BY_USER_INPUT: CGEventType = 0xFFFF_FFFF;
    const KCG_KEYBOARD_EVENT_KEYCODE: CGEventField = 9;
    const RETURN_KEY_CODE: u16 = 36;
    const KEYPAD_ENTER_KEY_CODE: u16 = 76;

    static ENTER_EVENT_MONITOR_STARTED: AtomicBool = AtomicBool::new(false);
    static ENTER_EVENT_MONITOR_AVAILABLE: AtomicBool = AtomicBool::new(false);
    static LATEST_ENTER_EVENT_SIGNAL: OnceLock<Mutex<Option<MacLiveSendSignal>>> = OnceLock::new();

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
        fn AXUIElementCreateApplication(pid: c_int) -> AXUIElementRef;
        fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> AXError;
        fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut c_int) -> AXError;
        fn CGEventSourceKeyState(state_id: c_int, key: u16) -> bool;
        fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events_of_interest: CGEventMask,
            callback: CGEventTapCallBack,
            user_info: *mut c_void,
        ) -> CFMachPortRef;
        fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
        fn CGEventGetIntegerValueField(event: CGEventRef, field: CGEventField) -> i64;
        fn CFMachPortCreateRunLoopSource(
            allocator: *const c_void,
            port: CFMachPortRef,
            order: isize,
        ) -> CFRunLoopSourceRef;
        fn CFRunLoopGetCurrent() -> CFRunLoopRef;
        fn CFRunLoopAddSource(
            run_loop: CFRunLoopRef,
            source: CFRunLoopSourceRef,
            mode: CFStringRef,
        );
        fn CFRunLoopRun();
    }

    pub(super) fn accessibility_trusted() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    pub(super) fn request_accessibility_trust_prompt() -> bool {
        let key = CFString::new("AXTrustedCheckOptionPrompt");
        let value = CFBoolean::true_value();
        let options = CFDictionary::from_CFType_pairs(&[(key, value)]);
        unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) }
    }

    pub(super) fn enter_key_is_down() -> bool {
        unsafe {
            CGEventSourceKeyState(KCG_EVENT_SOURCE_STATE_HID_SYSTEM_STATE, RETURN_KEY_CODE)
                || CGEventSourceKeyState(
                    KCG_EVENT_SOURCE_STATE_HID_SYSTEM_STATE,
                    KEYPAD_ENTER_KEY_CODE,
                )
        }
    }

    pub(super) fn start_enter_key_event_monitor() -> bool {
        if ENTER_EVENT_MONITOR_STARTED.swap(true, Ordering::SeqCst) {
            return ENTER_EVENT_MONITOR_AVAILABLE.load(Ordering::SeqCst);
        }
        let _ = LATEST_ENTER_EVENT_SIGNAL.get_or_init(|| Mutex::new(None));
        let _ = std::thread::Builder::new()
            .name("eomneunmal-enter-event-tap".to_owned())
            .spawn(run_enter_key_event_monitor);
        true
    }

    pub(super) fn take_enter_key_event_signal() -> Option<MacLiveSendSignal> {
        LATEST_ENTER_EVENT_SIGNAL
            .get()
            .and_then(|signal| signal.lock().ok()?.take())
    }

    fn run_enter_key_event_monitor() {
        let event_mask = 1_u64 << KCG_EVENT_KEY_DOWN;
        let tap = unsafe {
            CGEventTapCreate(
                KCG_SESSION_EVENT_TAP,
                KCG_HEAD_INSERT_EVENT_TAP,
                KCG_EVENT_TAP_OPTION_LISTEN_ONLY,
                event_mask,
                enter_key_event_callback,
                ptr::null_mut(),
            )
        };
        if tap.is_null() {
            ENTER_EVENT_MONITOR_AVAILABLE.store(false, Ordering::SeqCst);
            return;
        }

        let source = unsafe { CFMachPortCreateRunLoopSource(ptr::null(), tap, 0) };
        if source.is_null() {
            ENTER_EVENT_MONITOR_AVAILABLE.store(false, Ordering::SeqCst);
            return;
        }

        let mode = CFString::new("kCFRunLoopCommonModes");
        unsafe {
            CFRunLoopAddSource(CFRunLoopGetCurrent(), source, mode.as_concrete_TypeRef());
            CGEventTapEnable(tap, true);
        }
        ENTER_EVENT_MONITOR_AVAILABLE.store(true, Ordering::SeqCst);
        unsafe { CFRunLoopRun() };
    }

    extern "C" fn enter_key_event_callback(
        _proxy: CGEventTapProxy,
        event_type: CGEventType,
        event: CGEventRef,
        _user_info: *mut c_void,
    ) -> CGEventRef {
        if matches!(
            event_type,
            KCG_EVENT_TAP_DISABLED_BY_TIMEOUT | KCG_EVENT_TAP_DISABLED_BY_USER_INPUT
        ) {
            return event;
        }
        if event_type == KCG_EVENT_KEY_DOWN {
            let key_code =
                unsafe { CGEventGetIntegerValueField(event, KCG_KEYBOARD_EVENT_KEYCODE) };
            if key_code == i64::from(RETURN_KEY_CODE)
                || key_code == i64::from(KEYPAD_ENTER_KEY_CODE)
            {
                if let Some(signal) = LATEST_ENTER_EVENT_SIGNAL.get() {
                    if let Ok(mut signal) = signal.lock() {
                        *signal = Some(MacLiveSendSignal {
                            source: SendSignalSource::EnterKey,
                            observed_at: std::time::Instant::now(),
                        });
                    }
                }
            }
        }
        event
    }

    pub(super) fn capture_focused_text_snapshot(
        captured_at: std::time::Instant,
        include_chat_history: bool,
    ) -> Option<MacLiveTextSnapshot> {
        if !accessibility_trusted() {
            return None;
        }

        let (frontmost_pid, frontmost_bundle_id) = frontmost_application()?;
        let (target, app_id) = target_from_bundle_id(&frontmost_bundle_id)?;

        let application = unsafe {
            CFType::wrap_under_create_rule(AXUIElementCreateApplication(frontmost_pid) as CFTypeRef)
        };
        let focused = copy_attribute(
            application.as_CFTypeRef() as AXUIElementRef,
            "AXFocusedUIElement",
        )?;
        let focused_ref = focused.as_CFTypeRef() as AXUIElementRef;

        let mut pid = 0;
        if unsafe { AXUIElementGetPid(focused_ref, &mut pid) } != AX_ERROR_SUCCESS {
            return None;
        }
        if pid != frontmost_pid {
            return None;
        }

        let role = copy_string_attribute(focused_ref, "AXRole")?;
        let is_protected = copy_bool_attribute(focused_ref, "AXProtectedContent").unwrap_or(false)
            || copy_bool_attribute(focused_ref, "AXProtected").unwrap_or(false);
        let description = copy_string_attribute(focused_ref, "AXDescription");
        let title = copy_string_attribute(focused_ref, "AXTitle");
        let help = copy_string_attribute(focused_ref, "AXHelp");
        let placeholder = copy_string_attribute(focused_ref, "AXPlaceholderValue");
        let identifier_hash =
            copy_string_attribute(focused_ref, "AXIdentifier").map(|value| redacted_hash(&value));
        let window = copy_attribute(focused_ref, "AXWindow");
        let (window_title_hash, chat_history_hash) = window
            .as_ref()
            .map(|window| {
                let window_ref = window.as_CFTypeRef() as AXUIElementRef;
                (
                    copy_string_attribute(window_ref, "AXTitle").map(|title| redacted_hash(&title)),
                    include_chat_history
                        .then(|| chat_history_hash(window_ref))
                        .flatten(),
                )
            })
            .unwrap_or((None, None));

        let safe_probe =
            MacLiveTextSnapshot::new(target, app_id, role, is_protected, "", captured_at)
                .with_window_title_hash(window_title_hash)
                .with_accessibility_metadata(
                    description,
                    title,
                    help,
                    placeholder,
                    identifier_hash,
                    chat_history_hash,
                );
        if !safe_probe.looks_like_editable_chat_input() {
            return None;
        }

        let text = copy_string_attribute(focused_ref, "AXValue").unwrap_or_default();
        Some(MacLiveTextSnapshot { text, ..safe_probe })
    }

    fn copy_attribute(element: AXUIElementRef, attribute: &str) -> Option<CFType> {
        let attribute = CFString::new(attribute);
        let mut value: CFTypeRef = ptr::null();
        let error = unsafe {
            AXUIElementCopyAttributeValue(element, attribute.as_concrete_TypeRef(), &mut value)
        };
        if error == AX_ERROR_SUCCESS && !value.is_null() {
            Some(unsafe { CFType::wrap_under_create_rule(value) })
        } else {
            None
        }
    }

    fn copy_string_attribute(element: AXUIElementRef, attribute: &str) -> Option<String> {
        copy_attribute(element, attribute)
            .and_then(|value| value.downcast_into::<CFString>())
            .map(|value| value.to_string())
    }

    fn copy_bool_attribute(element: AXUIElementRef, attribute: &str) -> Option<bool> {
        copy_attribute(element, attribute)
            .and_then(|value| value.downcast_into::<CFBoolean>())
            .map(bool::from)
    }

    fn copy_array_attribute(element: AXUIElementRef, attribute: &str) -> Option<CFArray> {
        copy_attribute(element, attribute).and_then(|value| value.downcast_into::<CFArray>())
    }

    fn chat_history_hash(window: AXUIElementRef) -> Option<String> {
        let mut parts = Vec::new();
        collect_table_signature(window, 0, &mut parts);
        if parts.is_empty() {
            None
        } else {
            Some(redacted_hash(&parts.join("|")))
        }
    }

    fn collect_table_signature(element: AXUIElementRef, depth: usize, parts: &mut Vec<String>) {
        if depth > 4 || parts.len() > 64 {
            return;
        }

        let role = copy_string_attribute(element, "AXRole").unwrap_or_else(|| "unknown".to_owned());
        if role == "AXTable" {
            for attribute in ["AXChildren", "AXRows", "AXVisibleRows", "AXContents"] {
                if let Some(array) = copy_array_attribute(element, attribute) {
                    parts.push(format!("d{depth}:{role}:{attribute}:{}", array.len()));
                }
            }
            return;
        }

        for attribute in ["AXChildren", "AXRows", "AXVisibleRows", "AXContents"] {
            let Some(array) = copy_array_attribute(element, attribute) else {
                continue;
            };
            let values = array.get_all_values();
            for child in values.into_iter().take(32) {
                collect_table_signature(child as AXUIElementRef, depth + 1, parts);
                if parts.len() > 64 {
                    return;
                }
            }
        }
    }

    fn frontmost_application() -> Option<(c_int, String)> {
        let workspace = NSWorkspace::sharedWorkspace();
        let app = workspace.frontmostApplication()?;
        let pid = app.processIdentifier();
        let bundle_id = app.bundleIdentifier()?.to_string();
        Some((pid, bundle_id))
    }

    fn target_from_bundle_id(bundle_id: &str) -> Option<(TargetApp, &'static str)> {
        if bundle_id == KAKAOTALK_BUNDLE_ID {
            return Some((TargetApp::KakaoTalk, KAKAOTALK_BUNDLE_ID));
        }
        if bundle_id == DISCORD_BUNDLE_ID || bundle_id.to_lowercase().contains("discord") {
            return Some((TargetApp::Discord, DISCORD_BUNDLE_ID));
        }
        None
    }
}

fn redacted_hash(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn read_bundle_short_version(app_path: &Path) -> Option<String> {
    command_stdout(
        "defaults",
        &[
            "read",
            app_path.join("Contents/Info").to_str()?,
            "CFBundleShortVersionString",
        ],
    )
}

fn accessibility_permission_state() -> PermissionState {
    match Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to get UI elements enabled",
        ])
        .output()
    {
        Ok(output) if output.status.success() => {
            if String::from_utf8_lossy(&output.stdout).trim() == "true" {
                PermissionState::Ready
            } else {
                PermissionState::SetupRequired
            }
        }
        Ok(_) => PermissionState::Unknown,
        Err(_) => PermissionState::Unknown,
    }
}

fn command_stdout(command: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(command).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_bundle_ids_are_stable() {
        assert_eq!(
            MacApplicationProbe::discord().bundle_identifier,
            DISCORD_BUNDLE_ID
        );
        assert_eq!(
            MacApplicationProbe::kakaotalk().bundle_identifier,
            KAKAOTALK_BUNDLE_ID
        );
    }

    #[test]
    fn probe_rows_do_not_claim_live_pass_without_actual_send() {
        let probe = MacOsProbe {
            os_version: "test".to_owned(),
            accessibility_enabled: PermissionState::Ready,
            input_monitoring: PermissionState::Unknown,
            apps: vec![],
        };
        assert!(probe.probe_rows().is_empty());
    }

    #[test]
    fn discord_adapter_uses_in_memory_fallback_for_safe_chat_context() {
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
            MacFocusedElementContext::discord_chat("없는말 safe test channel").into(),
            SendSignalSource::EnterKey,
            now + Duration::from_millis(20),
        );

        match &decision {
            AdapterDecision::Ready {
                event, candidate, ..
            } => {
                assert_eq!(event.source, SendSignalSource::EnterKey);
                assert_eq!(candidate.as_str(), "그렇게 하면 되요");
                assert_eq!(candidate.method(), TextAcquisitionMethod::InMemoryFallback);
            }
            other => panic!("unexpected adapter decision: {other:?}"),
        }
        assert!(adapter.fallback_is_empty());
        let debug = format!("{decision:?}");
        assert!(!debug.contains("그렇게 하면 되요"));
        assert!(!debug.contains("safe test channel"));
    }

    #[test]
    fn discord_adapter_fails_closed_for_protected_context_and_clears_candidate() {
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

        match decision {
            AdapterDecision::Excluded { reasons, .. } => {
                assert!(reasons
                    .iter()
                    .any(|reason| reason.contains("password") || reason.contains("protected")));
            }
            other => panic!("expected protected-field exclusion, got {other:?}"),
        }
        assert!(adapter.fallback_is_empty());
    }

    #[test]
    fn discord_adapter_never_returns_ready_for_non_discord_foreground() {
        let now = Instant::now();
        let mut adapter = MacDiscordAdapter::with_permissions(MacOsPermissionSnapshot {
            accessibility: PermissionState::Ready,
            input_monitoring: PermissionState::Ready,
        });
        adapter.observe_candidate(
            "그렇게 하면 되요".to_owned(),
            DISCORD_BUNDLE_ID.to_owned(),
            now,
        );
        let context = AdapterContext::new(
            TargetApp::Discord,
            "com.apple.TextEdit",
            Some(redacted_hash("notes")),
            ElementMetadata::chat_input("com.apple.TextEdit", "Message"),
        );

        let decision = adapter.prepare_post_send(context, SendSignalSource::EnterKey, now);
        assert!(matches!(decision, AdapterDecision::Unavailable { .. }));
        assert!(adapter.fallback_is_empty());
    }

    #[test]
    fn kakaotalk_adapter_uses_in_memory_fallback_for_safe_chat_context() {
        let now = Instant::now();
        let mut adapter = MacKakaoTalkAdapter::with_permissions(MacOsPermissionSnapshot {
            accessibility: PermissionState::Ready,
            input_monitoring: PermissionState::Unknown,
        });
        adapter.observe_candidate(
            "그렇게 하면 되요".to_owned(),
            KAKAOTALK_BUNDLE_ID.to_owned(),
            now,
        );

        let decision = adapter.prepare_post_send(
            MacFocusedElementContext::kakaotalk_chat("나와의 채팅").into(),
            SendSignalSource::EnterKey,
            now + Duration::from_millis(20),
        );

        match &decision {
            AdapterDecision::Ready {
                event, candidate, ..
            } => {
                assert_eq!(event.target, TargetApp::KakaoTalk);
                assert_eq!(event.source, SendSignalSource::EnterKey);
                assert_eq!(candidate.as_str(), "그렇게 하면 되요");
                assert_eq!(candidate.method(), TextAcquisitionMethod::InMemoryFallback);
            }
            other => panic!("unexpected adapter decision: {other:?}"),
        }
        assert!(adapter.fallback_is_empty());
        let debug = format!("{decision:?}");
        assert!(!debug.contains("그렇게 하면 되요"));
        assert!(!debug.contains("나와의 채팅"));
    }

    #[test]
    fn live_transition_detector_fires_when_kakao_text_clears() {
        let now = Instant::now();
        let mut detector = MacPostSendTransitionDetector::default();
        assert!(detector
            .observe(Some(kakao_message_snapshot("그렇게 하면 되요", now)), None,)
            .is_none());

        let candidate = detector
            .observe(
                Some(kakao_message_snapshot("", now + Duration::from_millis(120))),
                Some(MacLiveSendSignal {
                    source: SendSignalSource::EnterKey,
                    observed_at: now + Duration::from_millis(100),
                }),
            )
            .expect("text clear after enter should become a post-send candidate");

        assert_eq!(candidate.target, TargetApp::KakaoTalk);
        assert_eq!(candidate.app_id, KAKAOTALK_BUNDLE_ID);
        assert_eq!(candidate.text, "그렇게 하면 되요");
        assert_eq!(candidate.context.target, TargetApp::KakaoTalk);
        assert_eq!(candidate.source, SendSignalSource::EnterKey);
    }

    #[test]
    fn live_transition_detector_ignores_protected_or_stale_text() {
        let now = Instant::now();
        let mut detector = MacPostSendTransitionDetector::new(Duration::from_secs(1));
        assert!(detector
            .observe(
                Some(
                    MacLiveTextSnapshot::new(
                        TargetApp::KakaoTalk,
                        KAKAOTALK_BUNDLE_ID,
                        "AXTextArea",
                        true,
                        "비밀번호123",
                        now,
                    )
                    .with_accessibility_metadata(
                        Some("메시지 입력".to_owned()),
                        None,
                        None,
                        None,
                        None,
                        None,
                    ),
                ),
                None,
            )
            .is_none());
        assert!(detector
            .observe(
                Some(kakao_message_snapshot("", now + Duration::from_millis(100),)),
                Some(MacLiveSendSignal {
                    source: SendSignalSource::EnterKey,
                    observed_at: now + Duration::from_millis(80),
                }),
            )
            .is_none());

        assert!(detector
            .observe(Some(kakao_message_snapshot("그렇게 하면 되요", now)), None,)
            .is_none());
        assert!(detector
            .observe(
                Some(kakao_message_snapshot("", now + Duration::from_secs(2))),
                Some(MacLiveSendSignal {
                    source: SendSignalSource::EnterKey,
                    observed_at: now + Duration::from_secs(1),
                }),
            )
            .is_none());
    }

    #[test]
    fn live_transition_detector_ignores_clear_without_send_signal() {
        let now = Instant::now();
        let mut detector = MacPostSendTransitionDetector::default();
        assert!(detector
            .observe(Some(kakao_message_snapshot("그렇게 하면 되요", now)), None,)
            .is_none());
        assert!(detector
            .observe(
                Some(kakao_message_snapshot("", now + Duration::from_millis(120),)),
                None,
            )
            .is_none());
    }

    #[test]
    fn live_transition_detector_accepts_chat_transcript_change_as_send_confirmation() {
        let now = Instant::now();
        let mut detector = MacPostSendTransitionDetector::default();
        assert!(detector
            .observe(
                Some(kakao_message_snapshot_with_history(
                    "그렇게 하면 되요",
                    now,
                    "history-before",
                )),
                None,
            )
            .is_none());

        let candidate = detector
            .observe(
                Some(kakao_message_snapshot_with_history(
                    "",
                    now + Duration::from_millis(180),
                    "history-after",
                )),
                None,
            )
            .expect("chat transcript mutation should confirm a post-send candidate");

        assert_eq!(candidate.target, TargetApp::KakaoTalk);
        assert_eq!(candidate.source, SendSignalSource::ChatTranscriptChanged);
    }

    #[test]
    fn live_transition_detector_requires_real_message_metadata() {
        let now = Instant::now();
        let mut detector = MacPostSendTransitionDetector::default();
        assert!(detector
            .observe(
                Some(MacLiveTextSnapshot::new(
                    TargetApp::KakaoTalk,
                    KAKAOTALK_BUNDLE_ID,
                    "AXTextArea",
                    false,
                    "그렇게 하면 되요",
                    now,
                )),
                None,
            )
            .is_none());
        assert!(detector
            .observe(
                Some(MacLiveTextSnapshot::new(
                    TargetApp::KakaoTalk,
                    KAKAOTALK_BUNDLE_ID,
                    "AXTextArea",
                    false,
                    "",
                    now + Duration::from_millis(120),
                )),
                Some(MacLiveSendSignal {
                    source: SendSignalSource::EnterKey,
                    observed_at: now + Duration::from_millis(100),
                }),
            )
            .is_none());
    }

    #[test]
    fn live_snapshot_debug_redacts_text() {
        let snapshot = MacLiveTextSnapshot::new(
            TargetApp::KakaoTalk,
            KAKAOTALK_BUNDLE_ID,
            "AXTextArea",
            false,
            "그렇게 하면 되요",
            Instant::now(),
        );
        let debug = format!("{snapshot:?}");
        assert!(debug.contains("<redacted>"));
        assert!(debug.contains("text_len"));
        assert!(!debug.contains("그렇게 하면 되요"));
    }

    #[test]
    fn live_candidate_and_context_debug_redact_text_and_metadata_labels() {
        let now = Instant::now();
        let candidate = MacLivePostSendCandidate {
            target: TargetApp::KakaoTalk,
            app_id: KAKAOTALK_BUNDLE_ID.to_owned(),
            text: "그렇게 하면 되요".to_owned(),
            context: MacFocusedElementContext::from_live_snapshot(&kakao_message_snapshot("", now))
                .expect("fixture is a safe KakaoTalk message field"),
            detected_at: now,
            source: SendSignalSource::ChatTranscriptChanged,
        };

        let debug = format!("{candidate:?}");
        assert!(debug.contains("<redacted>"));
        assert!(debug.contains("text_len"));
        assert!(debug.contains("label_present"));
        assert!(!debug.contains("그렇게 하면 되요"));
        assert!(!debug.contains("메시지 입력"));
    }

    fn kakao_message_snapshot(text: &str, captured_at: Instant) -> MacLiveTextSnapshot {
        kakao_message_snapshot_with_history(text, captured_at, "chat-history-v1")
    }

    fn kakao_message_snapshot_with_history(
        text: &str,
        captured_at: Instant,
        history_hash: &str,
    ) -> MacLiveTextSnapshot {
        MacLiveTextSnapshot::new(
            TargetApp::KakaoTalk,
            KAKAOTALK_BUNDLE_ID,
            "AXTextArea",
            false,
            text,
            captured_at,
        )
        .with_accessibility_metadata(
            Some("메시지 입력".to_owned()),
            None,
            None,
            None,
            Some("kakao-message-field-hash".to_owned()),
            Some(history_hash.to_owned()),
        )
    }
}
