use super::types::{SarcasmStrength, SpellingStrength};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Mock,
    OfficialApiKey,
    ExperimentalByoOAuth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTarget {
    Discord,
    KakaoTalk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppTargeting {
    pub discord_enabled: bool,
    pub kakaotalk_enabled: bool,
}

impl Default for AppTargeting {
    fn default() -> Self {
        Self {
            discord_enabled: true,
            kakaotalk_enabled: true,
        }
    }
}

impl AppTargeting {
    pub fn is_enabled(&self, target: AppTarget) -> bool {
        match target {
            AppTarget::Discord => self.discord_enabled,
            AppTarget::KakaoTalk => self.kakaotalk_enabled,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyControls {
    pub fail_closed_unknown_contexts: bool,
    pub redact_diagnostics: bool,
    pub persist_raw_text: bool,
}

impl Default for PrivacyControls {
    fn default() -> Self {
        Self {
            fail_closed_unknown_contexts: true,
            redact_diagnostics: true,
            persist_raw_text: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSettings {
    pub spelling_strength: SpellingStrength,
    pub sarcasm_strength: SarcasmStrength,
    pub provider: ProviderKind,
    pub experimental_byo_oauth_enabled: bool,
    pub app_targeting: AppTargeting,
    pub privacy: PrivacyControls,
    pub overlay_auto_dismiss_ms: u64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            spelling_strength: SpellingStrength::Medium,
            sarcasm_strength: SarcasmStrength::Weak,
            provider: ProviderKind::Mock,
            experimental_byo_oauth_enabled: false,
            app_targeting: AppTargeting::default(),
            privacy: PrivacyControls::default(),
            overlay_auto_dismiss_ms: 8_000,
        }
    }
}

impl AppSettings {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.provider == ProviderKind::ExperimentalByoOAuth
            && !self.experimental_byo_oauth_enabled
        {
            errors.push("experimental BYO OAuth provider requires explicit opt-in".to_owned());
        }
        if !self.privacy.fail_closed_unknown_contexts {
            errors.push("unknown contexts must fail closed".to_owned());
        }
        if !self.privacy.redact_diagnostics {
            errors.push("diagnostics must stay redacted by default".to_owned());
        }
        if self.privacy.persist_raw_text {
            errors.push("raw text persistence is not allowed in MVP defaults".to_owned());
        }
        if !self.app_targeting.discord_enabled && !self.app_targeting.kakaotalk_enabled {
            errors.push("at least one MVP app target must remain enabled".to_owned());
        }
        if self.overlay_auto_dismiss_ms < 1_000 {
            errors.push("overlay auto-dismiss must leave enough time to read".to_owned());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_privacy_preserving_and_mvp_targeted() {
        let settings = AppSettings::default();
        assert_eq!(settings.spelling_strength, SpellingStrength::Medium);
        assert_eq!(settings.sarcasm_strength, SarcasmStrength::Weak);
        assert_eq!(settings.provider, ProviderKind::Mock);
        assert!(!settings.experimental_byo_oauth_enabled);
        assert!(settings.app_targeting.is_enabled(AppTarget::Discord));
        assert!(settings.app_targeting.is_enabled(AppTarget::KakaoTalk));
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn byo_oauth_requires_explicit_opt_in() {
        let settings = AppSettings {
            provider: ProviderKind::ExperimentalByoOAuth,
            experimental_byo_oauth_enabled: false,
            ..AppSettings::default()
        };
        let errors = settings.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("explicit opt-in")));
    }

    #[test]
    fn raw_text_persistence_is_rejected() {
        let settings = AppSettings {
            privacy: PrivacyControls {
                persist_raw_text: true,
                ..PrivacyControls::default()
            },
            ..AppSettings::default()
        };
        assert!(settings
            .validate()
            .unwrap_err()
            .iter()
            .any(|e| e.contains("raw text")));
    }
}
