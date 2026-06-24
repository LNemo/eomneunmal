//! Privacy-first critique pipeline modules.

pub mod candidate;
pub mod diagnostics;
pub mod overlay;
pub mod pipeline;
pub mod prompt;
pub mod provider;
pub mod secret_boundary;
pub mod sensitivity;
pub mod settings;
pub mod types;

pub use candidate::{CandidateBuffer, CandidateSnapshot, ClearReason};
pub use diagnostics::{DiagnosticEvent, Redacted};
pub use overlay::{OverlayPhase, OverlayStateMachine, OVERLAY_RENDER_TARGET};
pub use pipeline::{CritiquePipeline, PipelineOutcome, PostSendEvent};
pub use prompt::PromptBuilder;
pub use provider::{
    CritiqueProvider, ExperimentalByoOAuthConnector, InMemorySecretStore, MockProvider,
    OfficialApiKeyProvider, ProviderError, SecretStore,
};
pub use secret_boundary::{SecretAccessBoundary, SecretKind};
pub use sensitivity::{
    ElementMetadata, SensitiveClassifier, SensitivityAction, SensitivityDecision,
};
pub use settings::{AppSettings, AppTarget, AppTargeting, PrivacyControls, ProviderKind};
pub use types::{CritiqueRequest, CritiqueResult, SarcasmStrength, SpellingStrength};
