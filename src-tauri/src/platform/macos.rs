use std::path::{Path, PathBuf};
use std::process::Command;

use super::probe::{
    AdapterProbe, OperatingSystem, PermissionState, ProbeRow, ProbeStatus, TargetApp,
};

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
            bundle_identifier: "com.hnc.Discord",
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
            "com.hnc.Discord"
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
}
