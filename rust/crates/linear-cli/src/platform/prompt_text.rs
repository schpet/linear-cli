//! Opt-in document text parsing; existing PromptSession::text stays unchanged.
use crate::text::js_space;
#[derive(Clone, Copy, Debug)]
pub struct TextOptions<'a> {
    pub minimum_utf16_length: usize,
    pub default: Option<&'a str>,
}
impl TextOptions<'_> {
    /// Called by the document command BEFORE entering raw PromptSession mode.
    pub fn preflight(self) -> Result<(), String> {
        if self
            .default
            .is_some_and(|text| text.chars().any(char::is_control))
        {
            return Err("prompt default contains a control character".to_owned());
        }
        Ok(())
    }
    /// Cliffy minLength inspects raw UTF16 units, then Input.transform trims JS
    /// whitespace. Exact empty alone accepts default, bypassing minLength.
    pub fn answer(self, raw: &str) -> Result<String, String> {
        if raw.is_empty()
            && let Some(default) = self.default
        {
            return Ok(default.to_owned());
        }
        if raw.encode_utf16().count() < self.minimum_utf16_length {
            return Err(format!(
                "answer must contain at least {} UTF16 unit(s)",
                self.minimum_utf16_length
            ));
        }
        Ok(raw.trim_matches(js_space).to_owned())
    }
}
