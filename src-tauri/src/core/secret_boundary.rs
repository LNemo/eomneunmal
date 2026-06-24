#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    OfficialApiKey,
    ExperimentalByoOAuthAccess,
    ExperimentalByoOAuthRefresh,
}

impl SecretKind {
    pub fn namespace(self) -> &'static str {
        match self {
            Self::OfficialApiKey => "eomneunmal.provider.official.api_key",
            Self::ExperimentalByoOAuthAccess => "eomneunmal.provider.experimental.oauth.access",
            Self::ExperimentalByoOAuthRefresh => "eomneunmal.provider.experimental.oauth.refresh",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretAccessBoundary;

impl SecretAccessBoundary {
    pub fn may_expose_to_webview(_kind: SecretKind) -> bool {
        false
    }

    pub fn may_log_secret_value(_kind: SecretKind) -> bool {
        false
    }

    pub fn requires_os_secret_store(_kind: SecretKind) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespaces_are_separate_for_official_and_experimental_auth() {
        assert_ne!(
            SecretKind::OfficialApiKey.namespace(),
            SecretKind::ExperimentalByoOAuthAccess.namespace()
        );
        assert_ne!(
            SecretKind::ExperimentalByoOAuthAccess.namespace(),
            SecretKind::ExperimentalByoOAuthRefresh.namespace()
        );
    }

    #[test]
    fn secrets_are_never_webview_or_log_exposed() {
        for kind in [
            SecretKind::OfficialApiKey,
            SecretKind::ExperimentalByoOAuthAccess,
            SecretKind::ExperimentalByoOAuthRefresh,
        ] {
            assert!(!SecretAccessBoundary::may_expose_to_webview(kind));
            assert!(!SecretAccessBoundary::may_log_secret_value(kind));
            assert!(SecretAccessBoundary::requires_os_secret_store(kind));
        }
    }
}
