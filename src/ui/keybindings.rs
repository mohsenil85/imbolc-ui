use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

use super::keymap::{KeyBinding, KeyPattern, Keymap};
use super::KeyCode;

/// Raw JSON structure for the keybindings config file
#[derive(Deserialize)]
struct KeybindingConfig {
    #[allow(dead_code)]
    version: u32,
    global: Vec<RawBinding>,
    panes: HashMap<String, Vec<RawBinding>>,
}

/// A single binding entry from JSON
#[derive(Deserialize)]
struct RawBinding {
    key: String,
    action: String,
    description: String,
    #[serde(default)]
    always_active: bool,
}

/// Parsed global keybindings for use in main.rs
pub struct GlobalBindings {
    bindings: Vec<GlobalBinding>,
}

struct GlobalBinding {
    pattern: KeyPattern,
    action: &'static str,
    #[allow(dead_code)]
    description: &'static str,
    always_active: bool,
}

impl GlobalBindings {
    /// Look up a global action for an input event.
    /// If `exclusive_mode` is true, only returns actions marked `always_active`.
    pub fn lookup(&self, event: &super::InputEvent, exclusive_mode: bool) -> Option<&'static str> {
        self.bindings
            .iter()
            .find(|b| {
                b.pattern.matches(event) && (!exclusive_mode || b.always_active)
            })
            .map(|b| b.action)
    }
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

/// Embedded default keybindings JSON
const DEFAULT_KEYBINDINGS: &str = include_str!("../../keybindings.json");

/// Load keybindings: embedded default, optionally merged with user override.
/// Returns (GlobalBindings, pane keymaps).
pub fn load_keybindings() -> (GlobalBindings, HashMap<String, Keymap>) {
    let mut config: KeybindingConfig =
        serde_json::from_str(DEFAULT_KEYBINDINGS).expect("Failed to parse embedded keybindings.json");

    // Try to load user override
    let user_path = user_keybindings_path();
    if let Some(path) = user_path {
        if path.exists() {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                if let Ok(user_config) = serde_json::from_str::<KeybindingConfig>(&contents) {
                    merge_config(&mut config, user_config);
                }
            }
        }
    }

    let global = build_global_bindings(&config.global);
    let pane_keymaps = build_pane_keymaps(&config.panes);

    (global, pane_keymaps)
}

fn user_keybindings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("ilex").join("keybindings.json"))
}

/// Merge user config into the base config.
/// User pane entries fully replace the default pane entries.
/// User globals fully replace default globals.
fn merge_config(base: &mut KeybindingConfig, user: KeybindingConfig) {
    if !user.global.is_empty() {
        base.global = user.global;
    }
    for (pane_id, bindings) in user.panes {
        base.panes.insert(pane_id, bindings);
    }
}

fn build_global_bindings(raw: &[RawBinding]) -> GlobalBindings {
    let bindings = raw
        .iter()
        .map(|b| GlobalBinding {
            pattern: parse_key(&b.key),
            action: intern(b.action.clone()),
            description: intern(b.description.clone()),
            always_active: b.always_active,
        })
        .collect();
    GlobalBindings { bindings }
}

fn build_pane_keymaps(panes: &HashMap<String, Vec<RawBinding>>) -> HashMap<String, Keymap> {
    panes
        .iter()
        .map(|(pane_id, bindings)| {
            let key_bindings: Vec<KeyBinding> = bindings
                .iter()
                .map(|b| KeyBinding {
                    pattern: parse_key(&b.key),
                    action: intern(b.action.clone()),
                    description: intern(b.description.clone()),
                })
                .collect();
            (pane_id.clone(), Keymap::from_bindings(key_bindings))
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
        let (global, panes) = load_keybindings();
        // Global bindings should exist
        assert!(global.bindings.len() > 5);
        // Should have pane keymaps
        assert!(panes.contains_key("instrument"));
        assert!(panes.contains_key("mixer"));
        assert!(panes.contains_key("piano_roll"));
    }
}
