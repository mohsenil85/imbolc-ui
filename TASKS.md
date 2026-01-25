# Tasks

Task queue for parallel agent execution. See `docs/phase2-spec.md` for detailed specifications.

## Status Legend

- `[ ]` - Not started
- `[~]` - In progress (assigned to agent)
- `[x]` - Completed and merged
- `[!]` - Blocked (waiting on dependency)

---

## Phase 2: State & Views

### Task 1: State Types
**Status:** `[x]`
**Branch:** `task-1`
**Files:** `src/state/mod.rs`, `src/state/module.rs`, `src/state/rack.rs`
**Dependencies:** None
**Conflicts with:** None

Create the core state types for modules and rack.

**Deliverables:**
1. `src/state/mod.rs` - Module exports
2. `src/state/module.rs`:
   - `ModuleId` type alias (u32)
   - `ModuleType` enum (SawOsc, SinOsc, SqrOsc, TriOsc, Lpf, Hpf, Bpf, AdsrEnv, Lfo, Delay, Reverb, Output)
   - `ModuleType::name()`, `ModuleType::default_params()`, `ModuleType::all_types()`
   - `Param` struct (name, value, min, max)
   - `ParamValue` enum (Float, Int, Bool)
   - `Module` struct (id, module_type, name, params)
3. `src/state/rack.rs`:
   - `RackState` struct (modules HashMap, order Vec, selected Option, next_id)
   - `RackState::new()`, `add_module()`, `remove_module()`
   - `selected_module()`, `selected_module_mut()`
   - `move_up()`, `move_down()` for reordering

**Tests:**
- Create module with default params
- Add/remove modules from rack
- Selection navigation
- Move module up/down

**Reference:** `docs/phase2-spec.md` Task 1

---

### Task 2: Action & Effect Enums
**Status:** `[x]`
**Branch:** `task-2`
**Files:** `src/core/mod.rs`, `src/core/action.rs`, `src/core/effect.rs`
**Dependencies:** None
**Conflicts with:** None

Create the action and effect enums for the command pattern.

**Deliverables:**
1. `src/core/mod.rs` - Module exports
2. `src/core/action.rs`:
   - `Action` enum with variants:
     - Navigation: `MoveUp`, `MoveDown`, `SelectNext`, `SelectPrev`
     - Module ops: `AddModule(ModuleType)`, `DeleteSelected`, `EditSelected`
     - Param editing: `ParamIncrement`, `ParamDecrement`, `ParamSet(f32)`, `NextParam`, `PrevParam`
     - View switching: `OpenAddView`, `OpenEditView`, `CloseView`, `Confirm`
     - System: `Quit`, `Save`, `Undo`, `Redo`
3. `src/core/effect.rs`:
   - `Effect` enum with variants:
     - Audio (future): `CreateSynth`, `FreeSynth`, `SetParam`
     - Persistence: `Save`, `Load`
     - System: `Quit`

**Note:** Import `ModuleType` from `crate::state` (will exist after Task 1 merges, but define the import anyway)

**Reference:** `docs/phase2-spec.md` Task 2

---

### Task 3: Rack View Pane
**Status:** `[x]`
**Branch:** `task-3`
**Files:** `src/panes/mod.rs`, `src/panes/rack_pane.rs`, `src/main.rs` (minor)
**Dependencies:** Task 1, Task 2
**Conflicts with:** Task 4, Task 5 (all modify main.rs)

Create the main rack view pane showing module list.

**Deliverables:**
1. `src/panes/mod.rs` - Module exports
2. `src/panes/rack_pane.rs`:
   - `RackPane` struct with `RackState` and `Keymap`
   - Implements `Pane` trait
   - Keymap: q=quit, n/p/j/k=navigate, a=add, d=delete, e=edit, g/G=top/bottom
   - Renders module list with selection indicator `>`
   - Shows module name, type, key params
3. Update `src/main.rs`:
   - Import `panes::RackPane`
   - Replace `DemoPane` with `RackPane` as main pane

**UI Layout:**
```
┌ Rack ─────────────────────────────────────────────────────────────────────────┐
│  Modules:                                                                     │
│                                                                               │
│  > saw-1         SawOsc       freq: 440.0  amp: 0.5                          │
│    lpf-1         Lpf          cutoff: 1000.0  res: 0.5                       │
│    out-1         Output                                                       │
│                                                                               │
│  a: add | d: delete | e: edit | q: quit                                      │
└───────────────────────────────────────────────────────────────────────────────┘
```

**Reference:** `docs/phase2-spec.md` Task 3

---

### Task 4: Add View Pane
**Status:** `[x]`
**Branch:** `task-4`
**Files:** `src/panes/add_pane.rs`, `src/panes/mod.rs` (add export)
**Dependencies:** Task 1, Task 2, Task 3
**Conflicts with:** Task 5 (if both modify panes/mod.rs simultaneously)

Create modal pane for selecting module type to add.

**Deliverables:**
1. `src/panes/add_pane.rs`:
   - `AddPane` struct with `SelectList` of module types and `Keymap`
   - Implements `Pane` trait
   - Keymap: Enter=confirm (returns AddModule action), Escape=cancel, n/p=navigate
   - Groups modules by category (Oscillators, Filters, Effects, Output)
   - Returns `Action::AddModule(selected_type)` on Enter
   - Returns `Action::PopPane` on Escape
2. Update `src/panes/mod.rs` to export `AddPane`

**UI Layout:**
```
┌ Add Module ───────────────────────────────────────────────────────────────────┐
│  Select module type:                                                          │
│                                                                               │
│  Oscillators:                                                                 │
│  > SawOsc        Sawtooth oscillator                                         │
│    SinOsc        Sine oscillator                                             │
│                                                                               │
│  Filters:                                                                     │
│    Lpf           Low-pass filter                                             │
│                                                                               │
│  Enter: add | Escape: cancel                                                 │
└───────────────────────────────────────────────────────────────────────────────┘
```

**Reference:** `docs/phase2-spec.md` Task 4

---

### Task 5: Edit View Pane
**Status:** `[x]`
**Branch:** `task-5`
**Files:** `src/panes/edit_pane.rs`, `src/panes/mod.rs` (add export)
**Dependencies:** Task 1, Task 2, Task 3
**Conflicts with:** Task 4 (if both modify panes/mod.rs simultaneously)

Create modal pane for editing module parameters.

**Deliverables:**
1. `src/panes/edit_pane.rs`:
   - `EditPane` struct with module reference, selected param index, `Keymap`
   - Implements `Pane` trait
   - Keymap: Left/Right=adjust value, n/p=select param, Escape=done
   - Shows param name, slider visualization, current value
   - Coarse adjustment with Left/Right
2. Update `src/panes/mod.rs` to export `EditPane`

**UI Layout:**
```
┌ Edit: saw-1 (SawOsc) ─────────────────────────────────────────────────────────┐
│  Parameters:                                                                  │
│                                                                               │
│  > freq      [━━━━━━━━━━━━━━━━━━━━━━━●━━━━━━━]  440.0 Hz                     │
│    amp       [━━━━━━━━━━●━━━━━━━━━━━━━━━━━━━━]  0.5                          │
│                                                                               │
│  Left/Right: adjust | n/p: select param | Escape: done                       │
└───────────────────────────────────────────────────────────────────────────────┘
```

**Reference:** `docs/phase2-spec.md` Task 5

---

## Execution Plan

### Wave 1 (Parallel - No Dependencies)
- Task 1: State Types
- Task 2: Action/Effect Enums

### Wave 2 (After Wave 1)
- Task 3: Rack View Pane

### Wave 3 (Parallel - After Wave 2)
- Task 4: Add View Pane
- Task 5: Edit View Pane

---

## Completed Tasks

(Move tasks here after merge)

---

## Notes for Agents

1. **Read existing code first**: Check `src/ui/pane.rs` for `Pane` trait, `src/ui/keymap.rs` for `Keymap` builder
2. **Follow existing patterns**: Look at `src/main.rs` DemoPane for reference implementation
3. **Use existing widgets**: `SelectList` and `TextInput` are in `src/ui/widgets/`
4. **Keybinding style**: Use n/p (emacs) + j/k (vim) + arrows. See `docs/keybindings.md`
5. **Test with tmux**: `TMUX= cargo run` to test in nested tmux
6. **Run cargo test**: Ensure existing tests pass before committing
