//! Privacy-first critique pipeline modules.

pub mod candidate;
pub mod diagnostics;
pub mod harness;
pub mod overlay;
pub mod overlay_controller;
pub mod pipeline;
pub mod prompt;
pub mod provider;
pub mod secret_boundary;
pub mod sensitivity;
pub mod settings;
pub mod types;

pub use candidate::{CandidateBuffer, CandidateSnapshot, ClearReason};
pub use diagnostics::{DiagnosticEvent, Redacted};
pub use harness::{run_post_send_harness, HarnessOutcome, HarnessScenario, SupportClaim};
pub use overlay::{OverlayPhase, OverlayStateMachine, OVERLAY_RENDER_TARGET};
pub use overlay_controller::{
    OverlayController, OverlayPresenter, OverlayRunReport, OverlayViewModel, OVERLAY_STATE_EVENT,
};
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
