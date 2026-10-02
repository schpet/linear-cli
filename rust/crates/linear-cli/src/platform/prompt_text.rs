//! Answer rules for free-text prompts that may offer a default.
#[derive(Clone, Copy, Debug)]
pub struct TextOptions<'a> {
    /// Reject answers that are blank after trimming.
    pub required: bool,
    pub default: Option<&'a str>,
}
impl TextOptions<'_> {
    /// Checked before the prompt switches the terminal into raw mode.
    pub fn preflight(self) -> Result<(), String> {
        if self
            .default
            .is_some_and(|text| text.chars().any(char::is_control))
        {
            return Err("prompt default contains a control character".to_owned());
        }
        Ok(())
    }
    /// An empty answer takes the default when there is one; otherwise the
    /// answer is trimmed.
    pub fn answer(self, raw: &str) -> Result<String, String> {
        if raw.is_empty()
            && let Some(default) = self.default
        {
            return Ok(default.to_owned());
        }
        let answer = raw.trim();
        if self.required && answer.is_empty() {
            return Err("an answer is required".to_owned());
        }
        Ok(answer.to_owned())
    }
}
