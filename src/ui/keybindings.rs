use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

use super::keymap::{KeyBinding, KeyPattern, Keymap};
use super::layer::Layer;
use super::KeyCode;

/// Raw TOML structure for the v2 keybindings config file
#[derive(Deserialize)]
struct KeybindingConfig {
    #[allow(dead_code)]
    version: u32,
    layers: HashMap<String, LayerConfig>,
}

#[derive(Deserialize)]
struct LayerConfig {
    #[serde(default = "default_transparent")]
    transparent: bool,
    bindings: Vec<RawBinding>,
}

fn default_transparent() -> bool {
    true
}

/// A single binding entry from TOML
#[derive(Deserialize)]
struct RawBinding {
    key: String,
    action: String,
    description: String,
}

/// Intern a String into a &'static str.
/// These are loaded once at startup and never freed.
fn intern(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// Parse a key notation string into a KeyPattern.
///
/// Supported formats:
/// - `"q"` → Char('q')
/// - `"Up"` → Key(KeyCode::Up)
/// - `"Ctrl+s"` → Ctrl('s')
/// - `"Alt+x"` → Alt('x')
/// - `"Ctrl+Left"` → CtrlKey(KeyCode::Left)
/// - `"Shift+Right"` → ShiftKey(KeyCode::Right)
/// - `"F1"` → Key(KeyCode::F(1))
fn parse_key(s: &str) -> KeyPattern {
    // Check for modifier prefixes
    if let Some(rest) = s.strip_prefix("Ctrl+") {
        if rest.len() == 1 {
            KeyPattern::Ctrl(rest.chars().next().unwrap())
        } else {
            KeyPattern::CtrlKey(parse_named_key(rest))
        }
    } else if let Some(rest) = s.strip_prefix("Alt+") {
        KeyPattern::Alt(rest.chars().next().unwrap())
    } else if let Some(rest) = s.strip_prefix("Shift+") {
        KeyPattern::ShiftKey(parse_named_key(rest))
    } else if s.len() == 1 {
        KeyPattern::Char(s.chars().next().unwrap())
    } else if s == "Space" {
        KeyPattern::Char(' ')
    } else {
        KeyPattern::Key(parse_named_key(s))
    }
}

/// Parse a named key string (e.g., "Up", "Enter", "F1") into a KeyCode
fn parse_named_key(s: &str) -> KeyCode {
    match s {
        "Up" => KeyCode::Up,
        "Down" => KeyCode::Down,
        "Left" => KeyCode::Left,
        "Right" => KeyCode::Right,
        "Enter" => KeyCode::Enter,
        "Escape" => KeyCode::Escape,
        "Backspace" => KeyCode::Backspace,
        "Tab" => KeyCode::Tab,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        "Insert" => KeyCode::Insert,
        "Delete" => KeyCode::Delete,
        _ if s.starts_with('F') => {
            if let Ok(n) = s[1..].parse::<u8>() {
                KeyCode::F(n)
            } else {
                panic!("Unknown key: {}", s);
            }
        }
        _ => panic!("Unknown key: {}", s),
    }
}

/// Embedded default keybindings TOML
const DEFAULT_KEYBINDINGS: &str = include_str!("../../keybindings.toml");

/// Mode layer names that are not pane layers
const MODE_LAYERS: &[&str] = &["global", "piano_mode", "pad_mode", "text_edit"];

/// Load keybindings: embedded default, optionally merged with user override.
/// Returns (Vec<Layer> for LayerStack, pane keymaps for pane construction).
pub fn load_keybindings() -> (Vec<Layer>, HashMap<String, Keymap>) {
    let mut config: KeybindingConfig =
        toml::from_str(DEFAULT_KEYBINDINGS).expect("Failed to parse embedded keybindings.toml");

    // Try to load user override
    let user_path = user_keybindings_path();
    if let Some(path) = user_path {
        if path.exists() {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                if let Ok(user_config) = toml::from_str::<KeybindingConfig>(&contents) {
                    merge_config(&mut config, user_config);
                }
            }
        }
    }

    let layers = build_layers(&config.layers);
    let pane_keymaps = build_pane_keymaps(&config.layers);

    (layers, pane_keymaps)
}

fn user_keybindings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("imbolc").join("keybindings.toml"))
}

/// Merge user config into the base config.
/// User layer entries fully replace the default layer entries.
fn merge_config(base: &mut KeybindingConfig, user: KeybindingConfig) {
    for (layer_id, layer_config) in user.layers {
        base.layers.insert(layer_id, layer_config);
    }
}

fn build_bindings(raw: &[RawBinding]) -> Vec<KeyBinding> {
    raw.iter()
        .map(|b| KeyBinding {
            pattern: parse_key(&b.key),
            action: intern(b.action.clone()),
            description: intern(b.description.clone()),
        })
        .collect()
}

fn build_layers(layers: &HashMap<String, LayerConfig>) -> Vec<Layer> {
    layers
        .iter()
        .map(|(name, config)| Layer {
            name: intern(name.clone()),
            keymap: Keymap::from_bindings(build_bindings(&config.bindings)),
            transparent: config.transparent,
        })
        .collect()
}

/// Build pane keymaps (excluding mode layers) for pane construction.
fn build_pane_keymaps(layers: &HashMap<String, LayerConfig>) -> HashMap<String, Keymap> {
    layers
        .iter()
        .filter(|(name, _)| !MODE_LAYERS.contains(&name.as_str()))
        .map(|(name, config)| {
            (
                name.clone(),
                Keymap::from_bindings(build_bindings(&config.bindings)),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_char() {
        assert_eq!(parse_key("q"), KeyPattern::Char('q'));
        assert_eq!(parse_key("+"), KeyPattern::Char('+'));
    }

    #[test]
    fn test_parse_key_named() {
        assert_eq!(parse_key("Up"), KeyPattern::Key(KeyCode::Up));
        assert_eq!(parse_key("Enter"), KeyPattern::Key(KeyCode::Enter));
        assert_eq!(parse_key("Space"), KeyPattern::Char(' '));
    }

    #[test]
    fn test_parse_key_modifiers() {
        assert_eq!(parse_key("Ctrl+s"), KeyPattern::Ctrl('s'));
        assert_eq!(parse_key("Alt+x"), KeyPattern::Alt('x'));
        assert_eq!(parse_key("Ctrl+Left"), KeyPattern::CtrlKey(KeyCode::Left));
        assert_eq!(parse_key("Shift+Right"), KeyPattern::ShiftKey(KeyCode::Right));
    }

    #[test]
    fn test_parse_key_f_keys() {
        assert_eq!(parse_key("F1"), KeyPattern::Key(KeyCode::F(1)));
        assert_eq!(parse_key("F12"), KeyPattern::Key(KeyCode::F(12)));
    }

    #[test]
    fn test_load_embedded_keybindings() {
        let (layers, pane_keymaps) = load_keybindings();
        // Should have layers
        assert!(layers.len() > 5);
        // Should have pane keymaps
        assert!(pane_keymaps.contains_key("instrument"));
        assert!(pane_keymaps.contains_key("mixer"));
        assert!(pane_keymaps.contains_key("piano_roll"));
    }
}
