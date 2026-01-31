# Keybindings Philosophy

ilex tries to use a "normie" keybinding scheme. The inspiration is
Dwarf Fortress or those 80's tui's that you used to see at the airport
where an experienced operator can fly through the menus at the speed
of thought. Numbers keys are used for navigation, ? to view help.

## Design Principles

1. **No Ctrl for common actions** - Single keys for frequent operations
2. **Mnemonic** - Keys should relate to their action (n=next, p=prev, etc.)
3. **Context-sensitive** - Same key can do different things in different panes
4. **Introspectable** - Every pane's keymap can be queried for help


## Global Keys

These work across all panes (when not captured by a widget):
(C-q means Control-q)

| Key | Action | Mnemonic |
|-----|--------|----------|
| `C-q` | Quit | quit |
| `?` | Help | question |
| `1-9` | Switch to pane N | number |

## Navigation Keys

Standard navigation (when a list/menu is focused):
Arrow keys work for navigation.

## Selection & Action Keys

| Key | Action | Mnemonic |
|-----|--------|----------|
| `Enter` | Select/confirm | - |
| `Space` | Toggle/select | - |
| `Escape` | Cancel/back | - |
| `Tab` | Next field | - |
| `a` | Add | add |
| `d` | Delete | delete |
| `e` | Edit | edit |
| `r` | Rename | rename |
| `s` | Save | save |
| `u` | Undo | undo |

## Text Input Mode

When a text input is focused, all keys type characters except:

| Key | Action |
|-----|--------|
| `Enter` | Confirm input |
| `Escape` | Cancel input |
| `Tab` | Next field |
| `Backspace` | Delete char before cursor |
| `Delete` | Delete char at cursor |
| `Left/Right` | Move cursor |
| `Home/End` | Start/end of input |

## Pane-Specific Keys

Each pane can define additional keys. Use `?` to see the current pane's keymap.

### Rack Pane
| Key | Action |
|-----|--------|
| `a` | Add module |
| `d` | Delete module |
| `.` | Panic (silence all) |

### Mixer Pane
| Key | Action |
|-----|--------|
| `m` | toggle Mute channel |
| `M` | unmute all channels |
| `s` | toggle Solo channel |
| `S` | unsolo all channel |
| `</>` | Pan left/right |
| `+/-` | Volume up/down |

### Sequencer Pane
| Key | Action |
|-----|--------|
| `Space` | Play/pause |
| `r` | Record |
| `l` | Loop toggle |
| `[/]` | Loop start/end |

## Rationale

3. **Accessibility** - Single keys are easier to press
4. **Testability** - Easier to send keys via tmux for E2E testing


### When to use modifiers?

Ctrl/Alt are reserved for:
- Destructive actions (Ctrl+D for force delete)
- System integration (Ctrl+C for copy, if supported)
- Disambiguation when single key is taken
