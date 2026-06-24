use super::probe::{
    AdapterProbe, OperatingSystem, PermissionState, ProbeRow, ProbeStatus, TargetApp,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsProbeDesign {
    pub send_event_hook: &'static str,
    pub text_patterns: Vec<&'static str>,
    pub password_signal: &'static str,
}

impl Default for WindowsProbeDesign {
    fn default() -> Self {
        Self {
            send_event_hook: "WH_KEYBOARD_LL via SetWindowsHookEx",
            text_patterns: vec!["TextPattern", "ValuePattern"],
            password_signal: "UI Automation IsPassword property",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsProbe {
    pub design: WindowsProbeDesign,
    pub current_host: bool,
}

impl Default for WindowsProbe {
    fn default() -> Self {
        Self {
            design: WindowsProbeDesign::default(),
            current_host: cfg!(target_os = "windows"),
        }
    }
}

impl AdapterProbe for WindowsProbe {
    fn probe_rows(&self) -> Vec<ProbeRow> {
        [TargetApp::Discord, TargetApp::KakaoTalk]
            .into_iter()
            .map(|app| ProbeRow {
                os: OperatingSystem::Windows,
                app,
                os_version: if self.current_host {
                    "current Windows host".to_owned()
                } else {
                    "not-current-host".to_owned()
                },
                app_version: "TBD on Windows host".to_owned(),
                permissions: if self.current_host {
                    PermissionState::Unknown
                } else {
                    PermissionState::NotCurrentHost
                },
                input_method: "Korean IME".to_owned(),
                send_signal: self.design.send_event_hook.to_owned(),
                text_acquisition_method: format!(
                    "UI Automation {} with {} exclusion",
                    self.design.text_patterns.join("/"),
                    self.design.password_signal
                ),
                sensitive_exclusion_result:
                    "contract specified; live Windows secure-field probe pending".to_owned(),
                status: if self.current_host {
                    ProbeStatus::Planned
                } else {
                    ProbeStatus::Blocked
                },
                evidence_notes: if self.current_host {
                    "run eomneunmal-probe on Windows test host before MVP go".to_owned()
                } else {
                    "blocked in this session: current host is not Windows".to_owned()
                },
            })
            .collect()
    }
}

pub fn planned_windows_rows() -> Vec<ProbeRow> {
    WindowsProbe::default().probe_rows()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_design_includes_required_hook_uia_and_password_signal() {
        let design = WindowsProbeDesign::default();
        assert!(design.send_event_hook.contains("WH_KEYBOARD_LL"));
        assert!(design.text_patterns.contains(&"TextPattern"));
        assert!(design.text_patterns.contains(&"ValuePattern"));
        assert!(design.password_signal.contains("IsPassword"));
    }

    #[test]
    fn non_windows_host_rows_are_blocked_not_passed() {
        let probe = WindowsProbe {
            design: WindowsProbeDesign::default(),
            current_host: false,
        };
        assert!(probe
            .probe_rows()
            .iter()
            .all(|row| row.status == ProbeStatus::Blocked));
    }
}
