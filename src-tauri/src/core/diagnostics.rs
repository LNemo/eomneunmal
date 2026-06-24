use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct Redacted<T>(pub T);

impl<T> fmt::Debug for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticEvent {
    pub app_id: Option<String>,
    pub window_title_hash: Option<String>,
    pub adapter_decision: String,
    pub permission_state: String,
    pub provider_status: Option<String>,
}

impl DiagnosticEvent {
    pub fn new(adapter_decision: impl Into<String>, permission_state: impl Into<String>) -> Self {
        Self {
            app_id: None,
            window_title_hash: None,
            adapter_decision: adapter_decision.into(),
            permission_state: permission_state.into(),
            provider_status: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacted_debug_never_prints_wrapped_value() {
        let value = Redacted("원문 메시지");
        let debug = format!("{:?}", value);
        assert_eq!(debug, "<redacted>");
        assert!(!debug.contains("원문"));
    }
}
