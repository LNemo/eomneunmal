#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellingStrength {
    Weak,
    Medium,
    Strong,
}

impl SpellingStrength {
    pub fn instruction(self) -> &'static str {
        match self {
            Self::Weak => "치명적이거나 자주 틀리는 한국어 맞춤법만 짧게 잡아라.",
            Self::Medium => "일반 맞춤법과 띄어쓰기를 균형 있게 교정하라.",
            Self::Strong => {
                "맞춤법, 띄어쓰기, 문장부호, 외래어 표기까지 가능한 한 엄격하게 교정하라."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SarcasmStrength {
    Weak,
    Medium,
    Strong,
}

impl SarcasmStrength {
    pub fn instruction(self) -> &'static str {
        match self {
            Self::Weak => "가벼운 농담처럼 놀려라.",
            Self::Medium => "비꼬는 느낌이 분명하게 드러나게 지적하라.",
            Self::Strong => "동의한 로컬 사용자에게 욕설과 직접 모욕을 섞어 강하게 비꼬아라.",
        }
    }
}

use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct CritiqueRequest {
    pub message: String,
    pub spelling_strength: SpellingStrength,
    pub sarcasm_strength: SarcasmStrength,
    pub locale: &'static str,
}

impl fmt::Debug for CritiqueRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CritiqueRequest")
            .field("message", &"<redacted>")
            .field("spelling_strength", &self.spelling_strength)
            .field("sarcasm_strength", &self.sarcasm_strength)
            .field("locale", &self.locale)
            .finish()
    }
}

impl CritiqueRequest {
    pub fn new(
        message: impl Into<String>,
        spelling_strength: SpellingStrength,
        sarcasm_strength: SarcasmStrength,
    ) -> Self {
        Self {
            message: message.into(),
            spelling_strength,
            sarcasm_strength,
            locale: "ko-KR",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CritiqueResult {
    pub corrected: String,
    pub explanation: String,
    pub roast: String,
}

impl fmt::Debug for CritiqueResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CritiqueResult")
            .field("corrected", &"<redacted>")
            .field("explanation", &"<redacted>")
            .field("roast", &"<redacted>")
            .finish()
    }
}

impl CritiqueResult {
    pub fn new(
        corrected: impl Into<String>,
        explanation: impl Into<String>,
        roast: impl Into<String>,
    ) -> Self {
        Self {
            corrected: corrected.into(),
            explanation: explanation.into(),
            roast: roast.into(),
        }
    }
}
