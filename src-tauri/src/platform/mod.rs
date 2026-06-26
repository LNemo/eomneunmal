//! Platform adapter probe boundaries.
//!
//! These modules are feasibility probes, not always-on capture. They expose the
//! evidence needed to decide whether a KakaoTalk/Discord pair is safe to promote
//! into the MVP adapter path.

pub mod adapter;
pub mod integration;
pub mod macos;
pub mod probe;
pub mod windows;

pub use adapter::{
    AdapterContext, AdapterDecision, CandidateText, LivePostSendAdapter, SendLikeEvent,
    SendSignalSource, TextAcquisitionMethod,
};
pub use integration::{
    run_adapter_decision_with_overlay, AdapterSupportClaim, LivePipelineReport, LivePipelineTiming,
};
pub use probe::{AdapterProbe, OperatingSystem, PermissionState, ProbeRow, ProbeStatus, TargetApp};
