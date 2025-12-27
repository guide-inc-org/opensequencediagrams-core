//! WebAssembly bindings for OpenSequenceDiagrams

use osd_core::{Config, Theme};
use wasm_bindgen::prelude::*;

/// Render a sequence diagram to SVG
///
/// # Arguments
/// * `input` - The sequence diagram source code
///
/// # Returns
/// The rendered SVG as a string, or an error message
#[wasm_bindgen]
pub fn render(input: &str) -> Result<String, String> {
    match osd_core::parse(input) {
        Ok(diagram) => Ok(osd_core::render(&diagram)),
        Err(e) => Err(e.to_string()),
    }
}

/// Render a sequence diagram to SVG with a specific theme
///
/// # Arguments
/// * `input` - The sequence diagram source code
/// * `theme_name` - The name of the theme to use (e.g., "modern-blue", "rose", "napkin")
///
/// # Returns
/// The rendered SVG as a string, or an error message
#[wasm_bindgen]
pub fn render_with_theme(input: &str, theme_name: &str) -> Result<String, String> {
    let theme = Theme::by_name(theme_name).unwrap_or_else(Theme::default);
    let config = Config::default().with_theme(theme);

    match osd_core::parse(input) {
        Ok(diagram) => Ok(osd_core::render_with_config(&diagram, config)),
        Err(e) => Err(e.to_string()),
    }
}

/// Get a list of available theme names
#[wasm_bindgen]
pub fn available_themes() -> Vec<String> {
    Theme::available_themes()
        .into_iter()
        .map(|s| s.to_string())
        .collect()
}

/// Parse a sequence diagram and return JSON representation
///
/// # Arguments
/// * `input` - The sequence diagram source code
///
/// # Returns
/// The parsed diagram as JSON, or an error message
#[wasm_bindgen]
pub fn parse_to_json(input: &str) -> Result<String, String> {
    match osd_core::parse(input) {
        Ok(diagram) => {
            // Simple JSON serialization
            let mut json = String::from("{");

            if let Some(title) = &diagram.title {
                json.push_str(&format!(r#""title":"{}","#, escape_json(title)));
            }

            let participants = diagram.participants();
            json.push_str(r#""participants":["#);
            for (i, p) in participants.iter().enumerate() {
                if i > 0 {
                    json.push(',');
                }
                json.push_str(&format!(
                    r#"{{"name":"{}","kind":"{}"}}"#,
                    escape_json(&p.name),
                    match p.kind {
                        osd_core::ParticipantKind::Participant => "participant",
                        osd_core::ParticipantKind::Actor => "actor",
                    }
                ));
            }
            json.push_str("],");

            json.push_str(&format!(r#""itemCount":{}"#, diagram.items.len()));
            json.push('}');

            Ok(json)
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Get version information
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render() {
        let result = render("Alice->Bob: Hello");
        assert!(result.is_ok());
        let svg = result.unwrap();
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn test_parse_to_json() {
        let result = parse_to_json("Alice->Bob: Hello");
        assert!(result.is_ok());
        let json = result.unwrap();
        assert!(json.contains("Alice"));
    }
}
