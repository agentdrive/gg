use crate::error::GrepAppError;
use std::sync::OnceLock;

const LANGUAGES_JSON: &str = include_str!("languages.json");
static LANGUAGES: OnceLock<Vec<String>> = OnceLock::new();

pub fn languages() -> Result<&'static [String], GrepAppError> {
    if let Some(langs) = LANGUAGES.get() {
        return Ok(langs.as_slice());
    }
    let languages: Vec<String> = serde_json::from_str(LANGUAGES_JSON)?;
    let _ = LANGUAGES.set(languages);
    Ok(LANGUAGES
        .get()
        .expect("languages set")
        .as_slice())
}

pub fn is_language_supported(name: &str) -> Result<bool, GrepAppError> {
    let langs = languages()?;
    Ok(langs.iter().any(|lang| lang == name))
}

#[cfg(test)]
mod tests {
    use super::{is_language_supported, languages};

    #[test]
    fn language_list_contains_common_values() {
        let langs = languages().expect("languages should parse");
        assert!(langs.iter().any(|lang| lang == "Rust"));
        assert!(langs.iter().any(|lang| lang == "TypeScript"));
        assert!(langs.iter().any(|lang| lang == "JavaScript"));
        assert!(langs.iter().any(|lang| lang == "C++"));
    }

    #[test]
    fn language_lookup_is_exact_match() {
        assert!(is_language_supported("Rust").expect("lookup should work"));
        assert!(!is_language_supported("rust").expect("lookup should work"));
    }
}
