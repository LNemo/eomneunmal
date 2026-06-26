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
            bundle_identifier: "com.kakao.KakaoTalkMac",
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacFocusedElementContext {
    pub app_id: String,
    pub window_title_hash: Option<String>,
    pub metadata: ElementMetadata,
}

impl MacFocusedElementContext {
    pub fn discord_chat(window_title: &str) -> Self {
        Self {
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
}

impl From<MacFocusedElementContext> for AdapterContext {
    fn from(value: MacFocusedElementContext) -> Self {
        AdapterContext::new(
            TargetApp::Discord,
            value.app_id,
            value.window_title_hash,
            value.metadata,
        )
    }
}

#[derive(Debug, Clone)]
pub struct MacDiscordAdapter {
    permissions: MacOsPermissionSnapshot,
    fallback: InMemoryCandidateFallback,
    classifier: SensitiveClassifier,
}

impl MacDiscordAdapter {
    pub fn from_probe(probe: &MacOsProbe) -> Self {
        Self {
            permissions: MacOsPermissionSnapshot::from_probe(probe),
            fallback: InMemoryCandidateFallback::new(Duration::from_secs(5)),
            classifier: SensitiveClassifier::default(),
        }
    }

    pub fn with_permissions(permissions: MacOsPermissionSnapshot) -> Self {
        Self {
            permissions,
            fallback: InMemoryCandidateFallback::new(Duration::from_secs(5)),
            classifier: SensitiveClassifier::default(),
        }
    }

    pub fn fallback_is_empty(&self) -> bool {
        self.fallback.is_empty()
    }

    fn permissions_allow_spike(&self) -> bool {
        matches!(
            self.permission_state(),
            PermissionState::Ready | PermissionState::Unknown
        )
    }
}

impl LivePostSendAdapter for MacDiscordAdapter {
    fn target_app(&self) -> TargetApp {
        TargetApp::Discord
    }

    fn permission_state(&self) -> PermissionState {
        self.permissions.adapter_permission_state()
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
                target: TargetApp::Discord,
                reason: format!("permission state is {}", self.permission_state().label()),
            };
        }

        if context.app_id != DISCORD_BUNDLE_ID {
            self.fallback.clear();
            return AdapterDecision::Unavailable {
                target: TargetApp::Discord,
                reason: "foreground app is not Discord".to_owned(),
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
                target: TargetApp::Discord,
                reason:
                    "no safe candidate text available; AX focused text capture not enabled in spike"
                        .to_owned(),
            };
        };

        AdapterDecision::Ready {
            event: SendLikeEvent {
                target: TargetApp::Discord,
                app_id: context.app_id.clone(),
                detected_at,
                source,
            },
            context,
            candidate,
        }
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
            "com.kakao.KakaoTalkMac"
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
}
