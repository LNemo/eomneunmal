#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ElementMetadata {
    pub app_id: Option<String>,
    pub window_title: Option<String>,
    pub role: Option<String>,
    pub control_type: Option<String>,
    pub label: Option<String>,
    pub placeholder: Option<String>,
    pub is_password: Option<bool>,
    pub is_protected: Option<bool>,
    pub user_denylisted: bool,
}

impl ElementMetadata {
    pub fn chat_input(app_id: &str, label: &str) -> Self {
        Self {
            app_id: Some(app_id.to_owned()),
            role: Some("text_input".to_owned()),
            control_type: Some("edit".to_owned()),
            label: Some(label.to_owned()),
            is_password: Some(false),
            is_protected: Some(false),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensitivityAction {
    Allow,
    Exclude,
    ManualReview,
}

impl SensitivityAction {
    pub fn permits_llm(self) -> bool {
        matches!(self, Self::Allow)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensitivityDecision {
    pub action: SensitivityAction,
    pub reasons: Vec<String>,
}

impl SensitivityDecision {
    fn allow(reason: impl Into<String>) -> Self {
        Self {
            action: SensitivityAction::Allow,
            reasons: vec![reason.into()],
        }
    }

    fn exclude(reasons: Vec<String>) -> Self {
        Self {
            action: SensitivityAction::Exclude,
            reasons,
        }
    }

    fn manual_review(reason: impl Into<String>) -> Self {
        Self {
            action: SensitivityAction::ManualReview,
            reasons: vec![reason.into()],
        }
    }
}

#[derive(Debug, Clone)]
pub struct SensitiveClassifier {
    sensitive_terms: Vec<&'static str>,
    chat_app_terms: Vec<&'static str>,
}

impl Default for SensitiveClassifier {
    fn default() -> Self {
        Self {
            sensitive_terms: vec![
                "password",
                "passcode",
                "pwd",
                "비밀번호",
                "암호",
                "패스워드",
                "카드",
                "card",
                "credit",
                "cvc",
                "cvv",
                "보안코드",
                "security code",
                "결제",
                "payment",
                "otp",
                "인증번호",
                "주민",
                "계좌",
                "account number",
                "login id",
                "로그인 id",
                "아이디",
            ],
            chat_app_terms: vec![
                "discord",
                "com.hnc.discord",
                "kakaotalk",
                "kakao",
                "카카오톡",
            ],
        }
    }
}

impl SensitiveClassifier {
    pub fn classify(&self, meta: &ElementMetadata) -> SensitivityDecision {
        let mut reasons = Vec::new();

        if meta.user_denylisted {
            reasons.push("user denylist matched".to_owned());
        }
        if meta.is_password == Some(true) {
            reasons.push("os password/protected field flag".to_owned());
        }
        if meta.is_protected == Some(true) {
            reasons.push("os protected field flag".to_owned());
        }

        let haystack = [
            meta.app_id.as_deref(),
            meta.window_title.as_deref(),
            meta.role.as_deref(),
            meta.control_type.as_deref(),
            meta.label.as_deref(),
            meta.placeholder.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

        for term in &self.sensitive_terms {
            if haystack.contains(term) {
                reasons.push(format!("sensitive metadata term matched: {term}"));
            }
        }

        if !reasons.is_empty() {
            return SensitivityDecision::exclude(reasons);
        }

        if self.looks_like_allowed_chat(meta, &haystack) {
            return SensitivityDecision::allow("known chat input with no sensitive signals");
        }

        if meta.app_id.is_none()
            && meta.role.is_none()
            && meta.control_type.is_none()
            && meta.label.is_none()
            && meta.placeholder.is_none()
        {
            return SensitivityDecision::exclude(vec!["unknown metadata; fail closed".to_owned()]);
        }

        SensitivityDecision::manual_review(
            "unrecognized editable context; do not submit to LLM in MVP",
        )
    }

    fn looks_like_allowed_chat(&self, meta: &ElementMetadata, haystack: &str) -> bool {
        let app_matches = meta
            .app_id
            .as_deref()
            .map(|app| {
                let app = app.to_lowercase();
                self.chat_app_terms.iter().any(|term| app.contains(term))
            })
            .unwrap_or(false)
            || self
                .chat_app_terms
                .iter()
                .any(|term| haystack.contains(term));

        let editable = [
            meta.role.as_deref(),
            meta.control_type.as_deref(),
            meta.label.as_deref(),
        ]
        .into_iter()
        .flatten()
        .map(str::to_lowercase)
        .any(|value| {
            value.contains("text")
                || value.contains("edit")
                || value.contains("message")
                || value.contains("메시지")
                || value.contains("입력")
        });

        app_matches && editable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_flag_excludes() {
        let classifier = SensitiveClassifier::default();
        let meta = ElementMetadata {
            is_password: Some(true),
            ..ElementMetadata::chat_input("discord", "message")
        };
        let decision = classifier.classify(&meta);
        assert_eq!(decision.action, SensitivityAction::Exclude);
        assert!(decision.reasons.iter().any(|r| r.contains("password")));
    }

    #[test]
    fn payment_label_excludes() {
        let classifier = SensitiveClassifier::default();
        let meta = ElementMetadata {
            label: Some("카드 CVC 입력".to_owned()),
            control_type: Some("edit".to_owned()),
            is_password: Some(false),
            ..ElementMetadata::default()
        };
        assert_eq!(
            classifier.classify(&meta).action,
            SensitivityAction::Exclude
        );
    }

    #[test]
    fn known_chat_input_allows_when_not_sensitive() {
        let classifier = SensitiveClassifier::default();
        let meta = ElementMetadata::chat_input("com.discordapp.Discord", "Message #general");
        assert_eq!(classifier.classify(&meta).action, SensitivityAction::Allow);
    }

    #[test]
    fn unknown_metadata_fails_closed() {
        let classifier = SensitiveClassifier::default();
        assert_eq!(
            classifier.classify(&ElementMetadata::default()).action,
            SensitivityAction::Exclude
        );
    }

    #[test]
    fn unrecognized_context_needs_manual_review_and_cannot_submit() {
        let classifier = SensitiveClassifier::default();
        let meta = ElementMetadata {
            app_id: Some("notes".to_owned()),
            role: Some("text input".to_owned()),
            is_password: Some(false),
            ..ElementMetadata::default()
        };
        let decision = classifier.classify(&meta);
        assert_eq!(decision.action, SensitivityAction::ManualReview);
        assert!(!decision.action.permits_llm());
    }
}
