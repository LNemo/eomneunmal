use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use crate::core::candidate::CandidateBuffer;
use crate::core::diagnostics::DiagnosticEvent;
use crate::core::overlay_controller::{
    OverlayController, OverlayPresenter, OverlayRunReport, OverlayViewModel, OVERLAY_STATE_EVENT,
};
use crate::core::pipeline::CritiquePipeline;
use crate::core::provider::MockProvider;
use crate::core::settings::{AppSettings, AppTargeting, PrivacyControls, ProviderKind};
use crate::core::types::{SarcasmStrength, SpellingStrength};
use crate::platform::adapter::{LivePostSendAdapter, SendSignalSource};
use crate::platform::integration::{
    run_adapter_decision_with_overlay, LivePipelineReport, LivePipelineTiming,
};
use crate::platform::macos::{
    accessibility_trusted_for_current_process, capture_focused_text_snapshot,
    capture_focused_text_snapshot_with_chat_history, request_accessibility_trust_prompt,
    start_enter_key_event_monitor, take_enter_key_event_signal, MacDiscordAdapter,
    MacEnterKeyStateTracker, MacFocusedElementContext, MacKakaoTalkAdapter,
    MacLivePostSendCandidate, MacOsPermissionSnapshot, MacPostSendTransitionDetector,
    DISCORD_BUNDLE_ID, KAKAOTALK_BUNDLE_ID,
};
use crate::platform::probe::{PermissionState, TargetApp};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSettingsDto {
    #[serde(default = "default_spelling_strength")]
    pub spelling_strength: String,
    #[serde(default = "default_sarcasm_strength")]
    pub sarcasm_strength: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub experimental_byo_oauth_enabled: bool,
    #[serde(default)]
    pub app_targeting: AppTargetingDto,
    #[serde(default)]
    pub privacy: PrivacyControlsDto,
    #[serde(default = "default_overlay_auto_dismiss_ms")]
    pub overlay_auto_dismiss_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppTargetingDto {
    #[serde(default = "default_true")]
    pub discord: bool,
    #[serde(default = "default_true")]
    pub kakaotalk: bool,
}

impl Default for AppTargetingDto {
    fn default() -> Self {
        Self {
            discord: true,
            kakaotalk: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyControlsDto {
    #[serde(default = "default_true")]
    pub fail_closed_unknown_contexts: bool,
    #[serde(default = "default_true")]
    pub redact_diagnostics: bool,
    #[serde(default)]
    pub persist_raw_text: bool,
}

impl Default for PrivacyControlsDto {
    fn default() -> Self {
        Self {
            fail_closed_unknown_contexts: true,
            redact_diagnostics: true,
            persist_raw_text: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub ok: bool,
    pub errors: Vec<String>,
}

impl ValidationReport {
    fn ok() -> Self {
        Self {
            ok: true,
            errors: Vec::new(),
        }
    }

    fn from_errors(errors: Vec<String>) -> Self {
        Self {
            ok: errors.is_empty(),
            errors,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RedactedDiagnosticsDto {
    pub app_id: Option<String>,
    pub window_title_hash: Option<String>,
    pub adapter_decision: String,
    pub permission_state: String,
    pub provider_status: Option<String>,
    pub raw_text_exposed: bool,
    pub secrets_exposed: bool,
}

#[derive(Debug)]
pub struct RuntimeState {
    settings: Mutex<RuntimeSettingsDto>,
    overlay: Mutex<OverlayController>,
    mock_pipeline: Mutex<CritiquePipeline<MockProvider>>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        let settings = AppSettings::default();
        Self {
            overlay: Mutex::new(OverlayController::new(Duration::from_millis(
                settings.overlay_auto_dismiss_ms,
            ))),
            settings: Mutex::new(RuntimeSettingsDto::from(settings)),
            mock_pipeline: Mutex::new(CritiquePipeline::new(
                MockProvider,
                CandidateBuffer::new(Duration::from_secs(5)),
            )),
        }
    }
}

#[tauri::command]
pub fn get_default_settings() -> RuntimeSettingsDto {
    RuntimeSettingsDto::from(AppSettings::default())
}

#[tauri::command]
pub fn get_saved_settings(state: tauri::State<'_, RuntimeState>) -> RuntimeSettingsDto {
    state
        .settings
        .lock()
        .expect("runtime settings mutex poisoned")
        .clone()
}

#[tauri::command]
pub fn validate_settings(settings: RuntimeSettingsDto) -> ValidationReport {
    validate_runtime_settings(&settings)
}

#[tauri::command]
pub fn save_settings(
    settings: RuntimeSettingsDto,
    state: tauri::State<'_, RuntimeState>,
) -> ValidationReport {
    let report = validate_runtime_settings(&settings);
    if report.ok {
        if let Ok(app_settings) = AppSettings::try_from(settings.clone()) {
            state
                .overlay
                .lock()
                .expect("runtime overlay mutex poisoned")
                .set_auto_dismiss_after(Duration::from_millis(
                    app_settings.overlay_auto_dismiss_ms,
                ));
        }
        *state
            .settings
            .lock()
            .expect("runtime settings mutex poisoned") = settings;
    }
    report
}

#[tauri::command]
pub fn redacted_diagnostics() -> RedactedDiagnosticsDto {
    let event = DiagnosticEvent::new("runtime-ready", "not-requested");
    let permission_state = if accessibility_trusted_for_current_process() {
        PermissionState::Ready.label().to_owned()
    } else {
        PermissionState::SetupRequired.label().to_owned()
    };
    RedactedDiagnosticsDto {
        app_id: event.app_id,
        window_title_hash: event.window_title_hash,
        adapter_decision: event.adapter_decision,
        permission_state,
        provider_status: event.provider_status,
        raw_text_exposed: false,
        secrets_exposed: false,
    }
}

#[tauri::command]
pub fn simulate_mock_post_send(
    app: tauri::AppHandle,
    state: tauri::State<'_, RuntimeState>,
) -> Result<OverlayRunReport, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "runtime settings mutex poisoned".to_owned())?
        .clone();
    let app_settings = AppSettings::try_from(settings.clone())
        .map_err(|errors| errors.join(" / "))
        .and_then(|app_settings| {
            app_settings
                .validate()
                .map(|()| app_settings)
                .map_err(|errors| errors.join(" / "))
        })?;

    if !app_settings.app_targeting.discord_enabled {
        return Err(
            "Discord 대상이 비활성화되어 mock post-send proof를 실행하지 않았습니다.".to_owned(),
        );
    }
    if app_settings.provider != ProviderKind::Mock {
        return Err(
            "현재 local proof는 Mock provider만 실행합니다. 공식 API 키/BYO OAuth는 설정 검증 경계만 준비된 상태입니다."
                .to_owned(),
        );
    }

    let detected_at = Instant::now();
    let mut adapter = MacDiscordAdapter::with_permissions(MacOsPermissionSnapshot {
        accessibility: PermissionState::Ready,
        input_monitoring: PermissionState::Unknown,
    });
    adapter.observe_candidate(
        "그렇게 하면 되요".to_owned(),
        DISCORD_BUNDLE_ID.to_owned(),
        detected_at,
    );
    let adapter_decision = adapter.prepare_post_send(
        MacFocusedElementContext::discord_chat("없는말 local proof").into(),
        SendSignalSource::SyntheticHarness,
        detected_at,
    );

    let mut presenter = TauriOverlayPresenter { app: &app };
    let mut overlay = state
        .overlay
        .lock()
        .map_err(|_| "runtime overlay mutex poisoned".to_owned())?;
    let mut pipeline = state
        .mock_pipeline
        .lock()
        .map_err(|_| "runtime mock pipeline mutex poisoned".to_owned())?;
    let report = run_adapter_decision_with_overlay(
        &mut pipeline,
        &mut overlay,
        &mut presenter,
        adapter_decision,
        app_settings.spelling_strength,
        app_settings.sarcasm_strength,
        LivePipelineTiming {
            shell_rendered_at: Instant::now(),
            result_rendered_at: Instant::now(),
        },
    )?;

    match report {
        LivePipelineReport::FeedbackShown { overlay, .. } => Ok(*overlay),
        LivePipelineReport::Excluded { reasons, .. } => Err(format!(
            "mock adapter excluded input: {}",
            reasons.join(" / ")
        )),
        LivePipelineReport::Unavailable { reason, .. } => {
            Err(format!("mock adapter unavailable: {reason}"))
        }
        LivePipelineReport::ProviderFailed { .. } => {
            Err("mock provider failed unexpectedly".to_owned())
        }
    }
}

#[tauri::command]
pub fn dismiss_overlay(
    app: tauri::AppHandle,
    state: tauri::State<'_, RuntimeState>,
) -> Result<(), String> {
    let mut presenter = TauriOverlayPresenter { app: &app };
    state
        .overlay
        .lock()
        .map_err(|_| "runtime overlay mutex poisoned".to_owned())?
        .dismiss(&mut presenter)
}

struct TauriOverlayPresenter<'a> {
    app: &'a tauri::AppHandle,
}

impl OverlayPresenter for TauriOverlayPresenter<'_> {
    fn show(&mut self, view: &OverlayViewModel) -> Result<(), String> {
        let window = self
            .app
            .get_webview_window("overlay")
            .ok_or_else(|| "overlay window not found".to_owned())?;
        window
            .set_focusable(false)
            .map_err(|error| format!("overlay set_focusable failed: {error}"))?;
        window
            .set_always_on_top(true)
            .map_err(|error| format!("overlay set_always_on_top failed: {error}"))?;
        window
            .show()
            .map_err(|error| format!("overlay show failed: {error}"))?;
        window
            .emit(OVERLAY_STATE_EVENT, view.clone())
            .map_err(|error| format!("overlay state emit failed: {error}"))
    }

    fn update(&mut self, view: &OverlayViewModel) -> Result<(), String> {
        let window = self
            .app
            .get_webview_window("overlay")
            .ok_or_else(|| "overlay window not found".to_owned())?;
        window
            .emit(OVERLAY_STATE_EVENT, view.clone())
            .map_err(|error| format!("overlay state emit failed: {error}"))
    }

    fn hide(&mut self, view: &OverlayViewModel) -> Result<(), String> {
        let window = self
            .app
            .get_webview_window("overlay")
            .ok_or_else(|| "overlay window not found".to_owned())?;
        window
            .emit(OVERLAY_STATE_EVENT, view.clone())
            .map_err(|error| format!("overlay state emit failed: {error}"))?;
        window
            .hide()
            .map_err(|error| format!("overlay hide failed: {error}"))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(RuntimeState::default())
        .setup(|app| {
            spawn_live_watcher(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_default_settings,
            get_saved_settings,
            validate_settings,
            save_settings,
            redacted_diagnostics,
            simulate_mock_post_send,
            dismiss_overlay
        ])
        .run(tauri::generate_context!())
        .expect("error while running 없는말 Tauri application");
}

fn spawn_live_watcher(app: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let _ = thread::Builder::new()
            .name("eomneunmal-macos-live-watcher".to_owned())
            .spawn(move || run_macos_live_watcher(app));
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
    }
}

#[cfg(target_os = "macos")]
fn run_macos_live_watcher(app: tauri::AppHandle) {
    let _ = request_accessibility_trust_prompt();
    let _ = start_enter_key_event_monitor();
    let mut detector = MacPostSendTransitionDetector::default();
    let mut enter_key_tracker = MacEnterKeyStateTracker::default();
    loop {
        thread::sleep(Duration::from_millis(60));
        let now = Instant::now();
        let send_signal = take_enter_key_event_signal().or_else(|| enter_key_tracker.observe(now));
        let snapshot = capture_focused_text_snapshot(now);
        let should_include_history = snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot.looks_like_editable_chat_input()
                    && (!snapshot.is_empty() || detector.has_pending_candidate())
            })
            .unwrap_or(false);
        let snapshot = if should_include_history {
            capture_focused_text_snapshot_with_chat_history(now).or(snapshot)
        } else {
            snapshot
        };
        let Some(candidate) = detector.observe(snapshot, send_signal) else {
            continue;
        };
        let _ = handle_live_post_send_candidate(&app, candidate);
    }
}

#[cfg(target_os = "macos")]
fn handle_live_post_send_candidate(
    app: &tauri::AppHandle,
    candidate: MacLivePostSendCandidate,
) -> Result<OverlayRunReport, String> {
    let state = app.state::<RuntimeState>();
    let settings = state
        .settings
        .lock()
        .map_err(|_| "runtime settings mutex poisoned".to_owned())?
        .clone();
    let app_settings = AppSettings::try_from(settings)
        .map_err(|errors| errors.join(" / "))
        .and_then(|app_settings| {
            app_settings
                .validate()
                .map(|()| app_settings)
                .map_err(|errors| errors.join(" / "))
        })?;

    if app_settings.provider != ProviderKind::Mock {
        return Err(
            "live local proof only executes Mock provider; official/BYO providers are settings-only"
                .to_owned(),
        );
    }

    let enabled = match candidate.target {
        TargetApp::Discord => app_settings.app_targeting.discord_enabled,
        TargetApp::KakaoTalk => app_settings.app_targeting.kakaotalk_enabled,
    };
    if !enabled {
        return Err(format!("{} target is disabled", candidate.target.label()));
    }

    let permissions = MacOsPermissionSnapshot {
        accessibility: PermissionState::Ready,
        input_monitoring: PermissionState::Unknown,
    };
    let adapter_decision = match candidate.target {
        TargetApp::Discord => {
            let mut adapter = MacDiscordAdapter::with_permissions(permissions);
            adapter.observe_candidate(
                candidate.text,
                DISCORD_BUNDLE_ID.to_owned(),
                candidate.detected_at,
            );
            adapter.prepare_post_send(
                candidate.context.into(),
                candidate.source,
                candidate.detected_at,
            )
        }
        TargetApp::KakaoTalk => {
            let mut adapter = MacKakaoTalkAdapter::with_permissions(permissions);
            adapter.observe_candidate(
                candidate.text,
                KAKAOTALK_BUNDLE_ID.to_owned(),
                candidate.detected_at,
            );
            adapter.prepare_post_send(
                candidate.context.into(),
                candidate.source,
                candidate.detected_at,
            )
        }
    };

    let mut presenter = TauriOverlayPresenter { app };
    let mut overlay = state
        .overlay
        .lock()
        .map_err(|_| "runtime overlay mutex poisoned".to_owned())?;
    let mut pipeline = state
        .mock_pipeline
        .lock()
        .map_err(|_| "runtime mock pipeline mutex poisoned".to_owned())?;
    let report = run_adapter_decision_with_overlay(
        &mut pipeline,
        &mut overlay,
        &mut presenter,
        adapter_decision,
        app_settings.spelling_strength,
        app_settings.sarcasm_strength,
        LivePipelineTiming {
            shell_rendered_at: Instant::now(),
            result_rendered_at: Instant::now(),
        },
    )?;

    match report {
        LivePipelineReport::FeedbackShown { overlay, .. } => Ok(*overlay),
        LivePipelineReport::Excluded { reasons, .. } => Err(format!(
            "live adapter excluded input: {}",
            reasons.join(" / ")
        )),
        LivePipelineReport::Unavailable { reason, .. } => {
            Err(format!("live adapter unavailable: {reason}"))
        }
        LivePipelineReport::ProviderFailed { .. } => {
            Err("mock provider failed unexpectedly".to_owned())
        }
    }
}

fn validate_runtime_settings(settings: &RuntimeSettingsDto) -> ValidationReport {
    match AppSettings::try_from(settings.clone()) {
        Ok(app_settings) => match app_settings.validate() {
            Ok(()) => ValidationReport::ok(),
            Err(errors) => ValidationReport::from_errors(errors),
        },
        Err(errors) => ValidationReport::from_errors(errors),
    }
}

impl From<AppSettings> for RuntimeSettingsDto {
    fn from(value: AppSettings) -> Self {
        Self {
            spelling_strength: spelling_strength_label(value.spelling_strength).to_owned(),
            sarcasm_strength: sarcasm_strength_label(value.sarcasm_strength).to_owned(),
            provider: provider_label(value.provider).to_owned(),
            experimental_byo_oauth_enabled: value.experimental_byo_oauth_enabled,
            app_targeting: AppTargetingDto {
                discord: value.app_targeting.discord_enabled,
                kakaotalk: value.app_targeting.kakaotalk_enabled,
            },
            privacy: PrivacyControlsDto {
                fail_closed_unknown_contexts: value.privacy.fail_closed_unknown_contexts,
                redact_diagnostics: value.privacy.redact_diagnostics,
                persist_raw_text: value.privacy.persist_raw_text,
            },
            overlay_auto_dismiss_ms: value.overlay_auto_dismiss_ms,
        }
    }
}

impl TryFrom<RuntimeSettingsDto> for AppSettings {
    type Error = Vec<String>;

    fn try_from(value: RuntimeSettingsDto) -> Result<Self, Self::Error> {
        let mut errors = Vec::new();

        let spelling_strength =
            parse_spelling_strength(&value.spelling_strength).unwrap_or_else(|| {
                errors.push(format!(
                    "unknown spelling strength: {}",
                    value.spelling_strength
                ));
                SpellingStrength::Medium
            });
        let sarcasm_strength =
            parse_sarcasm_strength(&value.sarcasm_strength).unwrap_or_else(|| {
                errors.push(format!(
                    "unknown sarcasm strength: {}",
                    value.sarcasm_strength
                ));
                SarcasmStrength::Weak
            });
        let provider = parse_provider(&value.provider).unwrap_or_else(|| {
            errors.push(format!("unknown provider: {}", value.provider));
            ProviderKind::Mock
        });

        if !errors.is_empty() {
            return Err(errors);
        }

        Ok(Self {
            spelling_strength,
            sarcasm_strength,
            provider,
            experimental_byo_oauth_enabled: value.experimental_byo_oauth_enabled,
            app_targeting: AppTargeting {
                discord_enabled: value.app_targeting.discord,
                kakaotalk_enabled: value.app_targeting.kakaotalk,
            },
            privacy: PrivacyControls {
                fail_closed_unknown_contexts: value.privacy.fail_closed_unknown_contexts,
                redact_diagnostics: value.privacy.redact_diagnostics,
                persist_raw_text: value.privacy.persist_raw_text,
            },
            overlay_auto_dismiss_ms: value.overlay_auto_dismiss_ms,
        })
    }
}

fn parse_spelling_strength(value: &str) -> Option<SpellingStrength> {
    match value {
        "weak" => Some(SpellingStrength::Weak),
        "medium" => Some(SpellingStrength::Medium),
        "strong" => Some(SpellingStrength::Strong),
        _ => None,
    }
}

fn parse_sarcasm_strength(value: &str) -> Option<SarcasmStrength> {
    match value {
        "weak" => Some(SarcasmStrength::Weak),
        "medium" => Some(SarcasmStrength::Medium),
        "strong" => Some(SarcasmStrength::Strong),
        _ => None,
    }
}

fn parse_provider(value: &str) -> Option<ProviderKind> {
    match value {
        "mock" => Some(ProviderKind::Mock),
        "official-api-key" => Some(ProviderKind::OfficialApiKey),
        "experimental-byo-oauth" => Some(ProviderKind::ExperimentalByoOAuth),
        _ => None,
    }
}

fn spelling_strength_label(value: SpellingStrength) -> &'static str {
    match value {
        SpellingStrength::Weak => "weak",
        SpellingStrength::Medium => "medium",
        SpellingStrength::Strong => "strong",
    }
}

fn sarcasm_strength_label(value: SarcasmStrength) -> &'static str {
    match value {
        SarcasmStrength::Weak => "weak",
        SarcasmStrength::Medium => "medium",
        SarcasmStrength::Strong => "strong",
    }
}

fn provider_label(value: ProviderKind) -> &'static str {
    match value {
        ProviderKind::Mock => "mock",
        ProviderKind::OfficialApiKey => "official-api-key",
        ProviderKind::ExperimentalByoOAuth => "experimental-byo-oauth",
    }
}

fn default_spelling_strength() -> String {
    "medium".to_owned()
}

fn default_sarcasm_strength() -> String {
    "weak".to_owned()
}

fn default_provider() -> String {
    "mock".to_owned()
}

fn default_overlay_auto_dismiss_ms() -> u64 {
    8_000
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_runtime_settings_match_privacy_preserving_core_defaults() {
        let settings = get_default_settings();
        assert_eq!(settings.provider, "mock");
        assert_eq!(settings.spelling_strength, "medium");
        assert_eq!(settings.sarcasm_strength, "weak");
        assert!(settings.app_targeting.discord);
        assert!(settings.app_targeting.kakaotalk);
        assert!(settings.privacy.fail_closed_unknown_contexts);
        assert!(settings.privacy.redact_diagnostics);
        assert!(!settings.privacy.persist_raw_text);
        assert!(validate_settings(settings).ok);
    }

    #[test]
    fn rust_side_settings_validation_rejects_unsafe_frontend_payloads() {
        let mut settings = get_default_settings();
        settings.privacy.fail_closed_unknown_contexts = false;
        settings.privacy.persist_raw_text = true;
        let report = validate_settings(settings);
        assert!(!report.ok);
        assert!(report.errors.iter().any(|e| e.contains("unknown")));
        assert!(report.errors.iter().any(|e| e.contains("raw text")));
    }

    #[test]
    fn byo_oauth_requires_explicit_opt_in_across_tauri_bridge() {
        let mut settings = get_default_settings();
        settings.provider = "experimental-byo-oauth".to_owned();
        settings.experimental_byo_oauth_enabled = false;
        let report = validate_settings(settings);
        assert!(!report.ok);
        assert!(report.errors.iter().any(|e| e.contains("opt-in")));
    }

    #[test]
    fn redacted_diagnostics_exposes_no_raw_text_or_secrets() {
        let diagnostics = redacted_diagnostics();
        assert!(!diagnostics.raw_text_exposed);
        assert!(!diagnostics.secrets_exposed);
        assert_eq!(diagnostics.adapter_decision, "runtime-ready");
    }
}
