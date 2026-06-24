use std::collections::HashMap;

use super::prompt::PromptBuilder;
use super::types::{CritiqueRequest, CritiqueResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    MissingSecret { namespace: String },
    Disabled { feature: &'static str },
    Timeout,
    Transport(String),
}

pub trait SecretStore {
    fn get_secret(&self, namespace: &str) -> Option<String>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemorySecretStore {
    values: HashMap<String, String>,
}

impl InMemorySecretStore {
    pub fn with_secret(mut self, namespace: impl Into<String>, value: impl Into<String>) -> Self {
        self.values.insert(namespace.into(), value.into());
        self
    }
}

impl SecretStore for InMemorySecretStore {
    fn get_secret(&self, namespace: &str) -> Option<String> {
        self.values.get(namespace).cloned()
    }
}

pub trait CritiqueProvider {
    fn critique(&self, request: &CritiqueRequest) -> Result<CritiqueResult, ProviderError>;
}

#[derive(Debug, Clone)]
pub struct MockProvider;

impl CritiqueProvider for MockProvider {
    fn critique(&self, request: &CritiqueRequest) -> Result<CritiqueResult, ProviderError> {
        let corrected = request
            .message
            .replace("되요", "돼요")
            .replace("안됀", "안 된");
        Ok(CritiqueResult::new(
            corrected,
            "mock: 대표 맞춤법 후보를 교정했습니다.",
            match request.sarcasm_strength {
                super::types::SarcasmStrength::Weak => "가볍게 말하면, 이건 좀 아쉽네.",
                super::types::SarcasmStrength::Medium => "세종대왕님이 살짝 한숨 쉬셨다.",
                super::types::SarcasmStrength::Strong => "돼지가 아니라 되지겠지, 돼지야.",
            },
        ))
    }
}

#[derive(Debug, Clone)]
pub struct OfficialApiKeyProvider<S> {
    secret_store: S,
    secret_namespace: String,
    prompt_builder: PromptBuilder,
}

impl<S: SecretStore> OfficialApiKeyProvider<S> {
    pub fn new(secret_store: S, secret_namespace: impl Into<String>) -> Self {
        Self {
            secret_store,
            secret_namespace: secret_namespace.into(),
            prompt_builder: PromptBuilder,
        }
    }

    pub fn authorization_header(&self) -> Result<String, ProviderError> {
        let secret = self
            .secret_store
            .get_secret(&self.secret_namespace)
            .ok_or_else(|| ProviderError::MissingSecret {
                namespace: self.secret_namespace.clone(),
            })?;
        Ok(format!("Bearer {secret}"))
    }

    pub fn request_blueprint(
        &self,
        request: &CritiqueRequest,
    ) -> Result<ProviderRequestBlueprint, ProviderError> {
        Ok(ProviderRequestBlueprint {
            authorization_header: self.authorization_header()?,
            prompt: self.prompt_builder.build(request),
            locale: request.locale,
        })
    }
}

impl<S: SecretStore> CritiqueProvider for OfficialApiKeyProvider<S> {
    fn critique(&self, request: &CritiqueRequest) -> Result<CritiqueResult, ProviderError> {
        let _blueprint = self.request_blueprint(request)?;
        Err(ProviderError::Transport(
            "network call not implemented in core MVP; use request_blueprint or MockProvider"
                .to_owned(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRequestBlueprint {
    pub authorization_header: String,
    pub prompt: String,
    pub locale: &'static str,
}

#[derive(Debug, Clone)]
pub struct ExperimentalByoOAuthConnector<S> {
    secret_store: S,
    enabled: bool,
    access_namespace: String,
}

impl<S: SecretStore> ExperimentalByoOAuthConnector<S> {
    pub fn new(secret_store: S, enabled: bool) -> Self {
        Self {
            secret_store,
            enabled,
            access_namespace: "eomneunmal.provider.experimental.oauth.access".to_owned(),
        }
    }

    pub fn access_token(&self) -> Result<String, ProviderError> {
        if !self.enabled {
            return Err(ProviderError::Disabled {
                feature: "experimental_byo_oauth",
            });
        }
        self.secret_store
            .get_secret(&self.access_namespace)
            .ok_or_else(|| ProviderError::MissingSecret {
                namespace: self.access_namespace.clone(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{SarcasmStrength, SpellingStrength};

    #[test]
    fn mock_provider_returns_correction_without_secret() {
        let request = CritiqueRequest::new(
            "그렇게 하면 되요",
            SpellingStrength::Medium,
            SarcasmStrength::Weak,
        );
        let result = MockProvider.critique(&request).unwrap();
        assert_eq!(result.corrected, "그렇게 하면 돼요");
    }

    #[test]
    fn official_provider_signs_with_bearer_from_secret_store() {
        let store = InMemorySecretStore::default().with_secret("api", "test-secret");
        let provider = OfficialApiKeyProvider::new(store, "api");
        assert_eq!(
            provider.authorization_header().unwrap(),
            "Bearer test-secret"
        );
    }

    #[test]
    fn official_provider_blueprint_contains_prompt_but_not_secret_in_prompt() {
        let store = InMemorySecretStore::default().with_secret("api", "test-secret");
        let provider = OfficialApiKeyProvider::new(store, "api");
        let request =
            CritiqueRequest::new("되요", SpellingStrength::Strong, SarcasmStrength::Strong);
        let blueprint = provider.request_blueprint(&request).unwrap();
        assert!(blueprint.authorization_header.contains("test-secret"));
        assert!(!blueprint.prompt.contains("test-secret"));
        assert!(blueprint.prompt.contains("되요"));
    }

    #[test]
    fn byo_oauth_is_off_by_default() {
        let store = InMemorySecretStore::default().with_secret(
            "eomneunmal.provider.experimental.oauth.access",
            "access-token",
        );
        let connector = ExperimentalByoOAuthConnector::new(store, false);
        assert_eq!(
            connector.access_token().unwrap_err(),
            ProviderError::Disabled {
                feature: "experimental_byo_oauth"
            }
        );
    }
}
