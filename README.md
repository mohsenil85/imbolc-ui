# TUI DAW

Terminal-based Digital Audio Workstation written in Rust.

## Features

- Modular synthesizer rack in the terminal
- Module types: Oscillators, Filters, Envelopes, LFO, Effects, Output
- Signal routing with connections between module ports
- Parameter editing with visual sliders
- SQLite-based persistence

## Prerequisites

- Rust 1.70 or later
- Cargo

## Build

```bash
cargo build --release
```

## Run

```bash
cargo run
```

## Module Types

| Category | Type | Ports |
|----------|------|-------|
| Oscillators | Saw, Sine, Square, Triangle | `out` (audio) |
| Filters | Low-Pass, High-Pass, Band-Pass | `in` (audio), `out` (audio), `cutoff_mod` (control) |
| Envelopes | ADSR | `gate` (gate), `out` (control) |
| Modulation | LFO | `out` (control) |
| Effects | Delay, Reverb | `in` (audio), `out` (audio) |
| Output | Output | `in` (audio) |

## Keybindings

### Rack View (Normal Mode)

| Key | Action |
|-----|--------|
| `j` / `k` / Arrow keys | Navigate modules |
| `g` / `G` | Go to top / bottom |
| `a` | Add module |
| `d` | Delete selected module |
| `e` | Edit module parameters |
| `c` | Enter connect mode |
| `x` | Disconnect (remove connection from selected module) |
| `w` | Save rack |
| `o` | Load rack |
| `q` | Quit |

### Connect Mode

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate modules |
| `Tab` / `h` / `l` | Cycle through ports on selected module |
| `Enter` | Confirm port selection |
| `Esc` | Cancel and return to normal mode |

Connection workflow:
1. Press `c` to enter connect mode
2. Navigate to source module and select output port
3. Press `Enter` to confirm source
4. Navigate to destination module and select input port
5. Press `Enter` to create connection

### Edit Mode

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate parameters |
| `h` / `l` | Decrease / increase value |
| `Esc` | Save and return to rack |

## Persistence

Racks are saved to `~/.config/tuidaw/rack.tuidaw` (SQLite format).

The save file includes:
- All modules with their parameters
- All connections between modules
- Module ordering

## Example Signal Chain

```
Saw Oscillator (saw-0)
    out ────────────────> in
                    Low-Pass Filter (lpf-1)
                        out ────────> in
                                  Output (out-2)

LFO (lfo-3)
    out ────────────────> cutoff_mod
                    Low-Pass Filter (lpf-1)
```

This creates a saw wave filtered by an LPF with LFO-modulated cutoff, routed to the output.

## Testing

```bash
cargo test
```

## Architecture

```
src/
├── main.rs          # Application entry, event loop
├── panes/           # UI panes (Rack, Add, Edit)
├── state/           # State management
│   ├── module.rs    # Module types, ports, parameters
│   ├── connection.rs # Connections between ports
│   └── rack.rs      # RackState with persistence
└── ui/              # UI framework
    ├── pane.rs      # Pane trait, Action enum
    ├── graphics.rs  # Drawing primitives
    └── keymap.rs    # Keybinding system
```
