//! Privacy-first critique pipeline modules.

pub mod candidate;
pub mod diagnostics;
pub mod pipeline;
pub mod prompt;
pub mod provider;
pub mod sensitivity;
pub mod types;

pub use candidate::{CandidateBuffer, CandidateSnapshot, ClearReason};
pub use diagnostics::{DiagnosticEvent, Redacted};
pub use pipeline::{CritiquePipeline, PipelineOutcome, PostSendEvent};
pub use prompt::PromptBuilder;
pub use provider::{
    CritiqueProvider, ExperimentalByoOAuthConnector, InMemorySecretStore, MockProvider,
    OfficialApiKeyProvider, ProviderError, SecretStore,
};
pub use sensitivity::{
    ElementMetadata, SensitiveClassifier, SensitivityAction, SensitivityDecision,
};
pub use types::{CritiqueRequest, CritiqueResult, SarcasmStrength, SpellingStrength};
