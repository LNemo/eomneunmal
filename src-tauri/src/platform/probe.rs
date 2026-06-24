#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatingSystem {
    MacOS,
    Windows,
}

impl OperatingSystem {
    pub fn label(self) -> &'static str {
        match self {
            Self::MacOS => "macOS",
            Self::Windows => "Windows",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetApp {
    Discord,
    KakaoTalk,
}

impl TargetApp {
    pub fn label(self) -> &'static str {
        match self {
            Self::Discord => "Discord",
            Self::KakaoTalk => "KakaoTalk",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    Ready,
    SetupRequired,
    Blocked,
    Unknown,
    NotCurrentHost,
}

impl PermissionState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::SetupRequired => "setup-required",
            Self::Blocked => "blocked",
            Self::Unknown => "unknown",
            Self::NotCurrentHost => "not-current-host",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeStatus {
    Planned,
    Pass,
    Partial,
    Blocked,
    Disabled,
}

impl ProbeStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Pass => "pass",
            Self::Partial => "partial",
            Self::Blocked => "blocked",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeRow {
    pub os: OperatingSystem,
    pub app: TargetApp,
    pub os_version: String,
    pub app_version: String,
    pub permissions: PermissionState,
    pub input_method: String,
    pub send_signal: String,
    pub text_acquisition_method: String,
    pub sensitive_exclusion_result: String,
    pub status: ProbeStatus,
    pub evidence_notes: String,
}

impl ProbeRow {
    pub fn markdown_row(&self) -> String {
        format!(
            "| {} | {} | {} / {} | {} | {} | {} | {} | {} | {} | {} |",
            self.os.label(),
            self.app.label(),
            escape(&self.os_version),
            escape(&self.app_version),
            self.permissions.label(),
            escape(&self.input_method),
            escape(&self.send_signal),
            escape(&self.text_acquisition_method),
            escape(&self.sensitive_exclusion_result),
            self.status.label(),
            escape(&self.evidence_notes),
        )
    }
}

pub trait AdapterProbe {
    fn probe_rows(&self) -> Vec<ProbeRow>;
}

fn escape(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_row_contains_status_and_redacted_notes() {
        let row = ProbeRow {
            os: OperatingSystem::MacOS,
            app: TargetApp::Discord,
            os_version: "27.0".to_owned(),
            app_version: "0.0.389".to_owned(),
            permissions: PermissionState::Ready,
            input_method: "Korean IME".to_owned(),
            send_signal: "Enter".to_owned(),
            text_acquisition_method: "AX focused text".to_owned(),
            sensitive_exclusion_result: "harness pass".to_owned(),
            status: ProbeStatus::Partial,
            evidence_notes: "no raw text | no send".to_owned(),
        };
        let md = row.markdown_row();
        assert!(md.contains("partial"));
        assert!(md.contains("no raw text \\| no send"));
    }
}
