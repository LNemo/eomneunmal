use super::types::{CritiqueRequest, SarcasmStrength, SpellingStrength};

#[derive(Debug, Clone, Default)]
pub struct PromptBuilder;

impl PromptBuilder {
    pub fn build(&self, request: &CritiqueRequest) -> String {
        format!(
            "너는 한국어 맞춤법 지적 캐릭터 '없는말'이다. locale={locale}.\n\
             맞춤법 강도: {spelling}\n\
             비꼼 강도: {sarcasm}\n\
             출력 JSON 필드: corrected, explanation, roast.\n\
             메시지:\n{message}",
            locale = request.locale,
            spelling = request.spelling_strength.instruction(),
            sarcasm = request.sarcasm_strength.instruction(),
            message = request.message
        )
    }

    pub fn request(
        &self,
        message: impl Into<String>,
        spelling_strength: SpellingStrength,
        sarcasm_strength: SarcasmStrength,
    ) -> CritiqueRequest {
        CritiqueRequest::new(message, spelling_strength, sarcasm_strength)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strictness_changes_correction_scope() {
        let builder = PromptBuilder;
        let weak =
            builder.build(&builder.request("되요", SpellingStrength::Weak, SarcasmStrength::Weak));
        let strong = builder.build(&builder.request(
            "되요",
            SpellingStrength::Strong,
            SarcasmStrength::Weak,
        ));
        assert!(weak.contains("치명적"));
        assert!(strong.contains("외래어"));
        assert_ne!(weak, strong);
    }

    #[test]
    fn sarcasm_strength_changes_tone() {
        let builder = PromptBuilder;
        let weak = builder.build(&builder.request(
            "되요",
            SpellingStrength::Medium,
            SarcasmStrength::Weak,
        ));
        let strong = builder.build(&builder.request(
            "되요",
            SpellingStrength::Medium,
            SarcasmStrength::Strong,
        ));
        assert!(weak.contains("가벼운 농담"));
        assert!(strong.contains("욕설") && strong.contains("직접 모욕"));
        assert!(strong.contains("로컬 사용자"));
    }
}
