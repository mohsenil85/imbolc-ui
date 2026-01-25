# SQLite Persistence Layer

Replace JSON file persistence with SQLite for better sharing, querying, and atomic operations.

## Motivation

- **Session sharing**: Send a single `.tuidaw` file to share entire session (instruments, tracks, mixer, window layout)
- **Atomic saves**: No partial writes or corruption
- **Query capability**: Find modules, search patterns, list presets
- **Version history**: Store undo/redo stack efficiently with deltas
- **Future-proof**: Easy to add new tables without breaking old files

## Schema

```sql
-- File format version for migrations
CREATE TABLE schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

-- Core session metadata
CREATE TABLE session (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    version TEXT NOT NULL  -- app version that created this
);

-- Modules in the rack
CREATE TABLE modules (
    id TEXT PRIMARY KEY,           -- "saw-1", "lpf-2"
    type TEXT NOT NULL,            -- "SAW_OSC", "LPF"
    mixer_channel INTEGER,         -- nullable, 1-128
    position INTEGER NOT NULL,     -- order in rack
    created_at TEXT NOT NULL
);

-- Module parameters (normalized)
CREATE TABLE module_params (
    module_id TEXT NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
    param_name TEXT NOT NULL,
    param_value REAL NOT NULL,
    PRIMARY KEY (module_id, param_name)
);

-- Signal routing between modules
CREATE TABLE patches (
    id INTEGER PRIMARY KEY,
    src_module TEXT NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
    src_port TEXT NOT NULL,
    dst_module TEXT NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
    dst_port TEXT NOT NULL,
    UNIQUE (src_module, src_port, dst_module, dst_port)
);

-- Mixer channels (only store non-default values)
CREATE TABLE mixer_channels (
    id INTEGER PRIMARY KEY,        -- 1-128
    module_id TEXT REFERENCES modules(id) ON DELETE SET NULL,
    level REAL NOT NULL DEFAULT 0.8,
    pan REAL NOT NULL DEFAULT 0.0,
    mute INTEGER NOT NULL DEFAULT 0,
    solo INTEGER NOT NULL DEFAULT 0,
    output_mode TEXT NOT NULL DEFAULT 'STEREO',
    output_target TEXT NOT NULL DEFAULT 'MASTER'
);

-- Mixer buses
CREATE TABLE mixer_buses (
    id INTEGER PRIMARY KEY,        -- 1-64
    name TEXT NOT NULL,
    level REAL NOT NULL DEFAULT 0.8,
    pan REAL NOT NULL DEFAULT 0.0,
    mute INTEGER NOT NULL DEFAULT 0,
    solo INTEGER NOT NULL DEFAULT 0
);

-- Mixer master
CREATE TABLE mixer_master (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    level REAL NOT NULL DEFAULT 0.8,
    pan REAL NOT NULL DEFAULT 0.0
);

-- Sequencer tracks
CREATE TABLE tracks (
    id INTEGER PRIMARY KEY,
    name TEXT,
    target_module TEXT REFERENCES modules(id) ON DELETE SET NULL,
    target_param TEXT,
    key_override_root TEXT,        -- nullable, Note enum
    key_override_scale TEXT        -- nullable, Scale enum
);

-- Sequencer steps (sparse - only store non-empty steps)
CREATE TABLE steps (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    step_index INTEGER NOT NULL,
    pitch INTEGER NOT NULL,
    velocity REAL NOT NULL,
    gate INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (track_id, step_index)
);

-- Musical settings
CREATE TABLE musical_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    key_root TEXT NOT NULL DEFAULT 'A',
    key_scale TEXT NOT NULL DEFAULT 'MINOR',
    time_sig_num INTEGER NOT NULL DEFAULT 4,
    time_sig_denom INTEGER NOT NULL DEFAULT 4,
    tempo_bpm REAL NOT NULL DEFAULT 120.0,
    tuning_hz REAL NOT NULL DEFAULT 440.0,
    grid_division TEXT NOT NULL DEFAULT 'SIXTEENTH',
    zoom_level TEXT NOT NULL DEFAULT 'BARS_2',
    snap_to_grid INTEGER NOT NULL DEFAULT 1,
    snap_to_key INTEGER NOT NULL DEFAULT 1
);

-- Click track settings
CREATE TABLE click_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    enabled INTEGER NOT NULL DEFAULT 0,
    volume REAL NOT NULL DEFAULT 0.5,
    accent_volume REAL NOT NULL DEFAULT 0.8
);

-- UI state (view, selections, scroll positions)
CREATE TABLE ui_state (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL
);

-- Undo history (optional, for session recovery)
CREATE TABLE undo_history (
    id INTEGER PRIMARY KEY,
    timestamp TEXT NOT NULL,
    action_type TEXT NOT NULL,
    state_delta_json TEXT NOT NULL  -- JSON patch format
);
```

## File Format

- Extension: `.tuidaw`
- Actually a SQLite database file
- Can be opened with any SQLite tool for inspection
- Compressed with SQLite's built-in page compression

## Java Implementation

### Dependencies

Add to `pom.xml`:
```xml
<dependency>
    <groupId>org.xerial</groupId>
    <artifactId>sqlite-jdbc</artifactId>
    <version>3.45.1.0</version>
</dependency>
```

### Classes

```
src/main/java/com/tuidaw/persistence/
├── Database.java           # Connection management, migrations
├── SessionRepository.java  # Save/load entire session
├── ModuleRepository.java   # CRUD for modules
├── PatchRepository.java    # CRUD for patches
├── MixerRepository.java    # Mixer state persistence
├── SequencerRepository.java # Tracks and steps
├── Migration.java          # Schema version upgrades
└── migrations/
    ├── V001_Initial.java
    ├── V002_AddMixer.java
    └── ...
```

### Usage

```java
// Save session
Database db = Database.open("mysession.tuidaw");
SessionRepository.save(db, rackState);
db.close();

// Load session
Database db = Database.open("mysession.tuidaw");
RackState state = SessionRepository.load(db);
db.close();

// Auto-save with WAL mode for performance
Database db = Database.openWithWAL("mysession.tuidaw");
// ... work ...
db.checkpoint();  // periodic flush
```

## Migration Strategy

1. Keep `RackSerializer` (JSON) for backwards compatibility
2. Add `Database` class for SQLite
3. Detect file type on load (JSON vs SQLite)
4. Default to SQLite for new files
5. Offer "Export to SQLite" for old JSON files

## Sharing & Portability

### Module Presets

Self-contained parameter snapshots that can be shared between projects:

```sql
-- Presets are portable: module_type + params, no foreign keys to specific modules
CREATE TABLE presets (
    id TEXT PRIMARY KEY,              -- content-addressable hash or UUID
    name TEXT NOT NULL,
    module_type TEXT NOT NULL,        -- "SAW_OSC", "LPF"
    params_json TEXT NOT NULL,        -- {"freq": 55, "amp": 0.8, "attack": 0.01}
    tags TEXT,                        -- comma-separated: "bass,lead,pad"
    author TEXT,
    created_at TEXT NOT NULL
);

-- Example: export a preset
SELECT * FROM presets WHERE name = 'fat bass';
-- Import into another project without ID conflicts
```

**Key design point:** Presets reference `module_type` not `module_id`, making them portable across projects.

### Rack Templates

Export a subgraph of modules + patches as a reusable template:

```sql
CREATE TABLE rack_templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    author TEXT,
    created_at TEXT NOT NULL
);

-- Template modules use local IDs (0, 1, 2...) not project IDs
CREATE TABLE template_modules (
    template_id TEXT NOT NULL REFERENCES rack_templates(id) ON DELETE CASCADE,
    local_id INTEGER NOT NULL,        -- 0, 1, 2... within this template
    module_type TEXT NOT NULL,
    params_json TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY (template_id, local_id)
);

-- Template patches reference local_ids, remapped on import
CREATE TABLE template_patches (
    template_id TEXT NOT NULL REFERENCES rack_templates(id) ON DELETE CASCADE,
    src_local_id INTEGER NOT NULL,
    src_port TEXT NOT NULL,
    dst_local_id INTEGER NOT NULL,
    dst_port TEXT NOT NULL,
    FOREIGN KEY (template_id, src_local_id) REFERENCES template_modules(template_id, local_id),
    FOREIGN KEY (template_id, dst_local_id) REFERENCES template_modules(template_id, local_id)
);
```

**Import flow:**
1. Load template modules, generate new project IDs (e.g., `saw-3`, `lpf-2`)
2. Build mapping: `{local_id: 0 -> "saw-3", local_id: 1 -> "lpf-2"}`
3. Remap patch references using the mapping
4. Insert into project's `modules` and `patches` tables

### Content-Addressable IDs

Using hashes instead of auto-increment IDs introduces interesting properties:

```sql
-- Instead of: id INTEGER PRIMARY KEY AUTOINCREMENT
-- Use: id TEXT PRIMARY KEY (SHA256 hash of content)

-- For presets: hash of (module_type + sorted params)
-- preset_id = sha256("SAW_OSC:amp=0.8,attack=0.01,freq=55")

-- For templates: hash of (modules + patches structure)
```

**Benefits:**
- **Determinism**: Same content = same ID, always
- **Deduplication**: Importing a preset you already have is a no-op
- **Integrity**: ID proves content hasn't been modified
- **Distributed sharing**: No central ID authority needed

**Trade-offs:**
- Any param change = new ID (is this a feature or a bug?)
- Longer IDs (64 hex chars vs integer)
- Can't have two presets with same params but different names (unless name is in hash)

**Hybrid approach:**
```sql
CREATE TABLE presets (
    id TEXT PRIMARY KEY,              -- UUID for identity
    content_hash TEXT NOT NULL,       -- SHA256 for deduplication
    name TEXT NOT NULL,
    -- ...
    UNIQUE (content_hash)             -- prevent duplicate content
);
```

This allows renaming without changing ID, while still detecting duplicates on import.

### Sharing Workflow

```
┌─────────────────────────────────────────────────────────────────┐
│  Export Options                                                 │
├─────────────────────────────────────────────────────────────────┤
│  [1] Entire session (.tuidaw)     - Everything, ready to play   │
│  [2] Rack template                - Modules + patches, no audio │
│  [3] Module preset                - Single module's settings    │
│  [4] Track pattern                - Sequencer steps only        │
└─────────────────────────────────────────────────────────────────┘
```

### Schema Considerations for Portability

1. **Avoid auto-increment for shared entities** - UUIDs or content hashes
2. **Separate "project" vs "library" tables** - Presets/templates are library items
3. **Include version info** - `app_version` field for compatibility checks
4. **Normalize references** - Use `module_type` not `module_id` where possible
5. **Store relative, not absolute** - Positions as indices, not pixel coordinates

## Benefits Over JSON

| Feature | JSON | SQLite |
|---------|------|--------|
| Atomic writes | No | Yes |
| Partial updates | No | Yes |
| Query data | Parse all | SQL |
| File size (large sessions) | Large | Compact |
| Concurrent access | No | Yes (WAL) |
| Corruption recovery | Manual | Built-in |
| Inspection tools | Text editor | DB browsers |
| Preset/template sharing | Manual extract | SQL queries |
| Deduplication | None | Content-hash unique |
