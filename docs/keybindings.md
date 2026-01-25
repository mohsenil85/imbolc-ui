# Keybindings Philosophy

tuidaw uses an emacs-inspired keybinding scheme, but without requiring Ctrl modifiers for common operations. This makes the app usable in terminals where Ctrl keys may be intercepted or awkward.

## Design Principles

1. **No Ctrl for common actions** - Single keys for frequent operations
2. **Mnemonic** - Keys should relate to their action (n=next, p=prev, etc.)
3. **Context-sensitive** - Same key can do different things in different panes
4. **Introspectable** - Every pane's keymap can be queried for help
5. **Vim-compatible alternatives** - j/k work alongside n/p where sensible

## Global Keys

These work across all panes (when not captured by a widget):

| Key | Action | Mnemonic |
|-----|--------|----------|
| `q` | Quit | quit |
| `?` | Help | question |
| `1-9` | Switch to pane N | number |

## Navigation Keys

Standard navigation (when a list/menu is focused):

| Key | Action | Mnemonic |
|-----|--------|----------|
| `n` | Next item | next |
| `p` | Previous item | previous |
| `j` | Next item (vim) | down |
| `k` | Previous item (vim) | up |
| `g` | Go to top | go |
| `G` | Go to bottom | Go (shifted) |
| `f` | Forward page | forward |
| `b` | Backward page | backward |

Arrow keys also work for navigation.

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

### Rack Pane (planned)
| Key | Action |
|-----|--------|
| `a` | Add module |
| `d` | Delete module |
| `m` | Open mixer |
| `.` | Panic (silence all) |

### Mixer Pane (planned)
| Key | Action |
|-----|--------|
| `m` | Mute channel |
| `s` | Solo channel |
| `</>` | Pan left/right |
| `+/-` | Volume up/down |

### Sequencer Pane (planned)
| Key | Action |
|-----|--------|
| `Space` | Play/pause |
| `r` | Record |
| `l` | Loop toggle |
| `[/]` | Loop start/end |

## Rationale

### Why not Ctrl keys?

1. **Terminal compatibility** - Some terminals intercept Ctrl+S, Ctrl+Q, etc.
2. **tmux/screen conflicts** - Ctrl+A, Ctrl+B are common prefixes
3. **Accessibility** - Single keys are easier to press
4. **Testability** - Easier to send keys via tmux for E2E testing

### Why emacs-style?

1. **Consistency** - n/p is a well-known pattern
2. **Text editing** - Cursor movement follows readline conventions
3. **Discoverability** - Mnemonics help users remember

### When to use modifiers?

Ctrl/Alt are reserved for:
- Destructive actions (Ctrl+D for force delete)
- System integration (Ctrl+C for copy, if supported)
- Disambiguation when single key is taken
