use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{Connection as SqlConnection, Result as SqlResult};
use serde::{Deserialize, Serialize};

use super::connection::{Connection, ConnectionError, PortRef};
use super::mixer::MixerState;
use super::{Module, ModuleId, ModuleType, Param, PortDirection};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RackState {
    pub modules: HashMap<ModuleId, Module>,
    pub order: Vec<ModuleId>,
    pub connections: HashSet<Connection>,
    #[serde(skip)]
    pub selected: Option<usize>, // Index in order vec (UI state, not persisted)
    #[serde(skip)]
    pub mixer: MixerState, // Mixer state (TODO: add persistence)
    next_id: ModuleId,
}

impl RackState {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            order: Vec::new(),
            connections: HashSet::new(),
            selected: None,
            mixer: MixerState::new(),
            next_id: 0,
        }
    }

    pub fn add_module(&mut self, module_type: ModuleType) -> ModuleId {
        let id = self.next_id;
        self.next_id += 1;

        let module = Module::new(id, module_type);
        self.modules.insert(id, module);
        self.order.push(id);

        // Auto-assign OUTPUT modules to mixer channels
        if module_type == ModuleType::Output {
            if let Some(channel_id) = self.mixer.find_free_channel() {
                self.mixer.assign_module(channel_id, id);
            }
        }

        // Auto-select first module if none selected
        if self.selected.is_none() {
            self.selected = Some(0);
        }

        id
    }

    pub fn remove_module(&mut self, id: ModuleId) {
        if let Some(pos) = self.order.iter().position(|&mid| mid == id) {
            self.order.remove(pos);
            self.modules.remove(&id);

            // Cascade delete all connections involving this module
            self.connections
                .retain(|c| c.src.module_id != id && c.dst.module_id != id);

            // Unassign from mixer if this was an OUTPUT module
            self.mixer.unassign_module(id);

            // Adjust selection
            if let Some(selected_idx) = self.selected {
                if selected_idx >= self.order.len() {
                    // Selection was at or past removed item
                    self.selected = if self.order.is_empty() {
                        None
                    } else {
                        Some(self.order.len() - 1)
                    };
                }
            }
        }
    }

    pub fn selected_module(&self) -> Option<&Module> {
        self.selected
            .and_then(|idx| self.order.get(idx))
            .and_then(|id| self.modules.get(id))
    }

    pub fn selected_module_mut(&mut self) -> Option<&mut Module> {
        if let Some(idx) = self.selected {
            if let Some(&id) = self.order.get(idx) {
                return self.modules.get_mut(&id);
            }
        }
        None
    }

    pub fn move_up(&mut self) {
        if let Some(idx) = self.selected {
            if idx > 0 {
                self.order.swap(idx - 1, idx);
                self.selected = Some(idx - 1);
            }
        }
    }

    pub fn move_down(&mut self) {
        if let Some(idx) = self.selected {
            if idx < self.order.len().saturating_sub(1) {
                self.order.swap(idx, idx + 1);
                self.selected = Some(idx + 1);
            }
        }
    }

    pub fn select_next(&mut self) {
        if self.order.is_empty() {
            self.selected = None;
            return;
        }

        self.selected = match self.selected {
            None => Some(0),
            Some(idx) if idx < self.order.len() - 1 => Some(idx + 1),
            Some(idx) => Some(idx), // Stay at last item
        };
    }

    pub fn select_prev(&mut self) {
        if self.order.is_empty() {
            self.selected = None;
            return;
        }

        self.selected = match self.selected {
            None => Some(0),
            Some(0) => Some(0), // Stay at first item
            Some(idx) => Some(idx - 1),
        };
    }

    /// Add a connection between two module ports
    pub fn add_connection(&mut self, connection: Connection) -> Result<(), ConnectionError> {
        let src_module = self
            .modules
            .get(&connection.src.module_id)
            .ok_or(ConnectionError::SourceModuleNotFound(
                connection.src.module_id,
            ))?;

        let dst_module = self
            .modules
            .get(&connection.dst.module_id)
            .ok_or(ConnectionError::DestModuleNotFound(connection.dst.module_id))?;

        // Validate source port exists and is an output
        let src_ports = src_module.module_type.ports();
        let src_port = src_ports
            .iter()
            .find(|p| p.name == connection.src.port_name)
            .ok_or_else(|| {
                ConnectionError::SourcePortNotFound(
                    connection.src.module_id,
                    connection.src.port_name.clone(),
                )
            })?;

        if src_port.direction != PortDirection::Output {
            return Err(ConnectionError::SourceNotOutput(
                connection.src.module_id,
                connection.src.port_name.clone(),
            ));
        }

        // Validate destination port exists and is an input
        let dst_ports = dst_module.module_type.ports();
        let dst_port = dst_ports
            .iter()
            .find(|p| p.name == connection.dst.port_name)
            .ok_or_else(|| {
                ConnectionError::DestPortNotFound(
                    connection.dst.module_id,
                    connection.dst.port_name.clone(),
                )
            })?;

        if dst_port.direction != PortDirection::Input {
            return Err(ConnectionError::DestNotInput(
                connection.dst.module_id,
                connection.dst.port_name.clone(),
            ));
        }

        // Check if connection already exists
        if self.connections.contains(&connection) {
            return Err(ConnectionError::AlreadyConnected);
        }

        self.connections.insert(connection);
        Ok(())
    }

    /// Remove a connection
    pub fn remove_connection(&mut self, connection: &Connection) -> bool {
        self.connections.remove(connection)
    }

    /// Get all connections from a specific module
    pub fn connections_from(&self, module_id: ModuleId) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.src.module_id == module_id)
            .collect()
    }

    /// Get all connections to a specific module
    pub fn connections_to(&self, module_id: ModuleId) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.dst.module_id == module_id)
            .collect()
    }

    /// Save rack state to SQLite database (.tuidaw file)
    pub fn save(&self, path: &Path) -> SqlResult<()> {
        let conn = SqlConnection::open(path)?;

        // Create schema (following docs/sqlite-persistence.md)
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS session (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                name TEXT NOT NULL,
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL,
                next_module_id INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS modules (
                id INTEGER PRIMARY KEY,
                type TEXT NOT NULL,
                name TEXT NOT NULL,
                position INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS module_params (
                module_id INTEGER NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
                param_name TEXT NOT NULL,
                param_value REAL NOT NULL,
                param_min REAL NOT NULL,
                param_max REAL NOT NULL,
                param_type TEXT NOT NULL,
                PRIMARY KEY (module_id, param_name)
            );

            CREATE TABLE IF NOT EXISTS connections (
                src_module_id INTEGER NOT NULL,
                src_port_name TEXT NOT NULL,
                dst_module_id INTEGER NOT NULL,
                dst_port_name TEXT NOT NULL,
                PRIMARY KEY (src_module_id, src_port_name, dst_module_id, dst_port_name)
            );

            -- Clear existing data for full save
            DELETE FROM connections;
            DELETE FROM module_params;
            DELETE FROM modules;
            DELETE FROM session;
            ",
        )?;

        // Insert/update schema version
        conn.execute(
            "INSERT OR REPLACE INTO schema_version (version, applied_at) VALUES (1, datetime('now'))",
            [],
        )?;

        // Insert session metadata
        conn.execute(
            "INSERT INTO session (id, name, created_at, modified_at, next_module_id)
             VALUES (1, 'default', datetime('now'), datetime('now'), ?1)",
            [&self.next_id],
        )?;

        // Insert modules with position from order
        {
            let mut stmt = conn.prepare(
                "INSERT INTO modules (id, type, name, position) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for (position, &module_id) in self.order.iter().enumerate() {
                if let Some(module) = self.modules.get(&module_id) {
                    let type_str = format!("{:?}", module.module_type);
                    stmt.execute((&module.id, &type_str, &module.name, &(position as i32)))?;
                }
            }
        }

        // Insert params (normalized)
        {
            let mut stmt = conn.prepare(
                "INSERT INTO module_params (module_id, param_name, param_value, param_min, param_max, param_type)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for module in self.modules.values() {
                for param in &module.params {
                    let (value, param_type) = match &param.value {
                        crate::state::ParamValue::Float(v) => (*v as f64, "float"),
                        crate::state::ParamValue::Int(v) => (*v as f64, "int"),
                        crate::state::ParamValue::Bool(v) => (if *v { 1.0 } else { 0.0 }, "bool"),
                    };
                    stmt.execute((
                        &module.id,
                        &param.name,
                        &value,
                        &(param.min as f64),
                        &(param.max as f64),
                        &param_type,
                    ))?;
                }
            }
        }

        // Insert connections
        {
            let mut stmt = conn.prepare(
                "INSERT INTO connections (src_module_id, src_port_name, dst_module_id, dst_port_name)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for connection in &self.connections {
                stmt.execute((
                    &connection.src.module_id,
                    &connection.src.port_name,
                    &connection.dst.module_id,
                    &connection.dst.port_name,
                ))?;
            }
        }

        Ok(())
    }

    /// Load rack state from SQLite database (.tuidaw file)
    pub fn load(path: &Path) -> SqlResult<Self> {
        let conn = SqlConnection::open(path)?;

        // Load session metadata
        let next_id: ModuleId = conn.query_row(
            "SELECT next_module_id FROM session WHERE id = 1",
            [],
            |row| row.get(0),
        )?;

        // Load modules ordered by position
        let mut modules = HashMap::new();
        let mut order = Vec::new();

        {
            let mut stmt =
                conn.prepare("SELECT id, type, name FROM modules ORDER BY position")?;
            let module_iter = stmt.query_map([], |row| {
                let id: ModuleId = row.get(0)?;
                let type_str: String = row.get(1)?;
                let name: String = row.get(2)?;
                Ok((id, type_str, name))
            })?;

            for result in module_iter {
                let (id, type_str, name) = result?;
                let module_type = parse_module_type(&type_str);
                order.push(id);
                modules.insert(
                    id,
                    Module {
                        id,
                        module_type,
                        name,
                        params: Vec::new(), // loaded next
                    },
                );
            }
        }

        // Load params for each module
        {
            let mut stmt = conn.prepare(
                "SELECT param_name, param_value, param_min, param_max, param_type
                 FROM module_params WHERE module_id = ?1",
            )?;

            for module in modules.values_mut() {
                let param_iter = stmt.query_map([&module.id], |row| {
                    let name: String = row.get(0)?;
                    let value: f64 = row.get(1)?;
                    let min: f64 = row.get(2)?;
                    let max: f64 = row.get(3)?;
                    let param_type: String = row.get(4)?;
                    Ok((name, value, min, max, param_type))
                })?;

                for result in param_iter {
                    let (name, value, min, max, param_type) = result?;
                    let param_value = match param_type.as_str() {
                        "float" => crate::state::ParamValue::Float(value as f32),
                        "int" => crate::state::ParamValue::Int(value as i32),
                        "bool" => crate::state::ParamValue::Bool(value != 0.0),
                        _ => crate::state::ParamValue::Float(value as f32),
                    };
                    module.params.push(Param {
                        name,
                        value: param_value,
                        min: min as f32,
                        max: max as f32,
                    });
                }
            }
        }

        // Load connections (table may not exist in older files)
        let mut connections = HashSet::new();
        if let Ok(mut stmt) = conn.prepare(
            "SELECT src_module_id, src_port_name, dst_module_id, dst_port_name FROM connections",
        ) {
            let conn_iter = stmt.query_map([], |row| {
                let src_module_id: ModuleId = row.get(0)?;
                let src_port_name: String = row.get(1)?;
                let dst_module_id: ModuleId = row.get(2)?;
                let dst_port_name: String = row.get(3)?;
                Ok((src_module_id, src_port_name, dst_module_id, dst_port_name))
            })?;

            for result in conn_iter {
                let (src_module_id, src_port_name, dst_module_id, dst_port_name) = result?;
                connections.insert(Connection::new(
                    PortRef::new(src_module_id, src_port_name),
                    PortRef::new(dst_module_id, dst_port_name),
                ));
            }
        }

        Ok(Self {
            modules,
            order,
            connections,
            selected: None,
            mixer: MixerState::new(), // Fresh mixer state on load (TODO: persist)
            next_id,
        })
    }
}

impl Default for RackState {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse module type from string (used for SQLite loading)
fn parse_module_type(s: &str) -> ModuleType {
    match s {
        "Midi" => ModuleType::Midi,
        "SawOsc" => ModuleType::SawOsc,
        "SinOsc" => ModuleType::SinOsc,
        "SqrOsc" => ModuleType::SqrOsc,
        "TriOsc" => ModuleType::TriOsc,
        "Lpf" => ModuleType::Lpf,
        "Hpf" => ModuleType::Hpf,
        "Bpf" => ModuleType::Bpf,
        "AdsrEnv" => ModuleType::AdsrEnv,
        "Lfo" => ModuleType::Lfo,
        "Delay" => ModuleType::Delay,
        "Reverb" => ModuleType::Reverb,
        "Output" => ModuleType::Output,
        _ => ModuleType::Output, // fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rack_creation() {
        let rack = RackState::new();
        assert_eq!(rack.modules.len(), 0);
        assert_eq!(rack.order.len(), 0);
        assert_eq!(rack.selected, None);
    }

    #[test]
    fn test_add_module() {
        let mut rack = RackState::new();
        let id1 = rack.add_module(ModuleType::SawOsc);
        let id2 = rack.add_module(ModuleType::Lpf);

        assert_eq!(rack.modules.len(), 2);
        assert_eq!(rack.order.len(), 2);
        assert_eq!(rack.order[0], id1);
        assert_eq!(rack.order[1], id2);
        assert_eq!(rack.selected, Some(0)); // Auto-selected first
    }

    #[test]
    fn test_remove_module() {
        let mut rack = RackState::new();
        let id1 = rack.add_module(ModuleType::SawOsc);
        let id2 = rack.add_module(ModuleType::Lpf);
        let id3 = rack.add_module(ModuleType::Output);

        rack.remove_module(id2);

        assert_eq!(rack.modules.len(), 2);
        assert_eq!(rack.order.len(), 2);
        assert_eq!(rack.order[0], id1);
        assert_eq!(rack.order[1], id3);
    }

    #[test]
    fn test_remove_selected_module() {
        let mut rack = RackState::new();
        let id1 = rack.add_module(ModuleType::SawOsc);
        let id2 = rack.add_module(ModuleType::Lpf);
        let _id3 = rack.add_module(ModuleType::Output);

        rack.selected = Some(1); // Select middle module
        rack.remove_module(id2);

        assert_eq!(rack.selected, Some(1)); // Selection moves to next item
    }

    #[test]
    fn test_remove_last_module() {
        let mut rack = RackState::new();
        let id1 = rack.add_module(ModuleType::SawOsc);
        let id2 = rack.add_module(ModuleType::Lpf);

        rack.selected = Some(1); // Select last module
        rack.remove_module(id2);

        assert_eq!(rack.selected, Some(0)); // Selection adjusts to last available
    }

    #[test]
    fn test_remove_all_modules() {
        let mut rack = RackState::new();
        let id1 = rack.add_module(ModuleType::SawOsc);

        rack.remove_module(id1);

        assert_eq!(rack.selected, None);
        assert!(rack.order.is_empty());
    }

    #[test]
    fn test_selected_module() {
        let mut rack = RackState::new();
        rack.add_module(ModuleType::SawOsc);
        rack.add_module(ModuleType::Lpf);

        rack.selected = Some(0);
        let module = rack.selected_module().unwrap();
        assert_eq!(module.module_type, ModuleType::SawOsc);

        rack.selected = Some(1);
        let module = rack.selected_module().unwrap();
        assert_eq!(module.module_type, ModuleType::Lpf);
    }

    #[test]
    fn test_selected_module_mut() {
        let mut rack = RackState::new();
        rack.add_module(ModuleType::SawOsc);

        rack.selected = Some(0);
        if let Some(module) = rack.selected_module_mut() {
            module.name = "Custom Name".to_string();
        }

        let module = rack.selected_module().unwrap();
        assert_eq!(module.name, "Custom Name");
    }

    #[test]
    fn test_select_next() {
        let mut rack = RackState::new();
        rack.add_module(ModuleType::SawOsc);
        rack.add_module(ModuleType::Lpf);
        rack.add_module(ModuleType::Output);

        rack.selected = Some(0);
        rack.select_next();
        assert_eq!(rack.selected, Some(1));

        rack.select_next();
        assert_eq!(rack.selected, Some(2));

        rack.select_next();
        assert_eq!(rack.selected, Some(2)); // Stay at last
    }

    #[test]
    fn test_select_prev() {
        let mut rack = RackState::new();
        rack.add_module(ModuleType::SawOsc);
        rack.add_module(ModuleType::Lpf);
        rack.add_module(ModuleType::Output);

        rack.selected = Some(2);
        rack.select_prev();
        assert_eq!(rack.selected, Some(1));

        rack.select_prev();
        assert_eq!(rack.selected, Some(0));

        rack.select_prev();
        assert_eq!(rack.selected, Some(0)); // Stay at first
    }

    #[test]
    fn test_select_on_empty_rack() {
        let mut rack = RackState::new();

        rack.select_next();
        assert_eq!(rack.selected, None);

        rack.select_prev();
        assert_eq!(rack.selected, None);
    }

    #[test]
    fn test_move_up() {
        let mut rack = RackState::new();
        let id1 = rack.add_module(ModuleType::SawOsc);
        let id2 = rack.add_module(ModuleType::Lpf);
        let id3 = rack.add_module(ModuleType::Output);

        rack.selected = Some(1); // Select middle module
        rack.move_up();

        assert_eq!(rack.selected, Some(0));
        assert_eq!(rack.order[0], id2);
        assert_eq!(rack.order[1], id1);
        assert_eq!(rack.order[2], id3);
    }

    #[test]
    fn test_move_down() {
        let mut rack = RackState::new();
        let id1 = rack.add_module(ModuleType::SawOsc);
        let id2 = rack.add_module(ModuleType::Lpf);
        let id3 = rack.add_module(ModuleType::Output);

        rack.selected = Some(1); // Select middle module
        rack.move_down();

        assert_eq!(rack.selected, Some(2));
        assert_eq!(rack.order[0], id1);
        assert_eq!(rack.order[1], id3);
        assert_eq!(rack.order[2], id2);
    }

    #[test]
    fn test_move_up_at_top() {
        let mut rack = RackState::new();
        let id1 = rack.add_module(ModuleType::SawOsc);
        let id2 = rack.add_module(ModuleType::Lpf);

        rack.selected = Some(0);
        rack.move_up();

        assert_eq!(rack.selected, Some(0)); // Stay at top
        assert_eq!(rack.order[0], id1); // Order unchanged
        assert_eq!(rack.order[1], id2);
    }

    #[test]
    fn test_move_down_at_bottom() {
        let mut rack = RackState::new();
        let id1 = rack.add_module(ModuleType::SawOsc);
        let id2 = rack.add_module(ModuleType::Lpf);

        rack.selected = Some(1);
        rack.move_down();

        assert_eq!(rack.selected, Some(1)); // Stay at bottom
        assert_eq!(rack.order[0], id1); // Order unchanged
        assert_eq!(rack.order[1], id2);
    }

    #[test]
    fn test_save_and_load() {
        use std::fs;
        use tempfile::tempdir;

        // Create a rack with some modules
        let mut rack = RackState::new();
        let id1 = rack.add_module(ModuleType::SawOsc);
        let id2 = rack.add_module(ModuleType::Lpf);
        let id3 = rack.add_module(ModuleType::AdsrEnv);

        // Modify a param
        if let Some(module) = rack.modules.get_mut(&id1) {
            if let Some(param) = module.params.iter_mut().find(|p| p.name == "freq") {
                param.value = crate::state::ParamValue::Float(880.0);
            }
        }

        // Save to temp file
        let dir = tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("test.tuidaw");
        rack.save(&path).expect("Failed to save");

        // Load and verify
        let loaded = RackState::load(&path).expect("Failed to load");

        // Verify modules
        assert_eq!(loaded.modules.len(), 3);
        assert_eq!(loaded.order.len(), 3);
        assert_eq!(loaded.order[0], id1);
        assert_eq!(loaded.order[1], id2);
        assert_eq!(loaded.order[2], id3);

        // Verify module types
        assert_eq!(loaded.modules.get(&id1).unwrap().module_type, ModuleType::SawOsc);
        assert_eq!(loaded.modules.get(&id2).unwrap().module_type, ModuleType::Lpf);
        assert_eq!(loaded.modules.get(&id3).unwrap().module_type, ModuleType::AdsrEnv);

        // Verify modified param was saved
        let saw = loaded.modules.get(&id1).unwrap();
        let freq_param = saw.params.iter().find(|p| p.name == "freq").expect("freq param");
        if let crate::state::ParamValue::Float(f) = freq_param.value {
            assert!((f - 880.0).abs() < 0.01, "Expected freq=880.0, got {}", f);
        } else {
            panic!("Expected Float param");
        }

        // Verify next_id was preserved
        assert_eq!(loaded.next_id, 3);

        // Clean up
        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_add_valid_connection() {
        let mut rack = RackState::new();
        let osc_id = rack.add_module(ModuleType::SawOsc);
        let filter_id = rack.add_module(ModuleType::Lpf);

        let connection = Connection::new(
            PortRef::new(osc_id, "out"),
            PortRef::new(filter_id, "in"),
        );

        assert!(rack.add_connection(connection.clone()).is_ok());
        assert_eq!(rack.connections.len(), 1);
        assert!(rack.connections.contains(&connection));
    }

    #[test]
    fn test_add_connection_invalid_source_module() {
        let mut rack = RackState::new();
        let filter_id = rack.add_module(ModuleType::Lpf);

        let connection = Connection::new(
            PortRef::new(999, "out"), // Non-existent module
            PortRef::new(filter_id, "in"),
        );

        let result = rack.add_connection(connection);
        assert!(matches!(result, Err(ConnectionError::SourceModuleNotFound(999))));
    }

    #[test]
    fn test_add_connection_invalid_port() {
        let mut rack = RackState::new();
        let osc_id = rack.add_module(ModuleType::SawOsc);
        let filter_id = rack.add_module(ModuleType::Lpf);

        let connection = Connection::new(
            PortRef::new(osc_id, "nonexistent"),
            PortRef::new(filter_id, "in"),
        );

        let result = rack.add_connection(connection);
        assert!(matches!(result, Err(ConnectionError::SourcePortNotFound(_, _))));
    }

    #[test]
    fn test_add_connection_source_not_output() {
        let mut rack = RackState::new();
        let filter1_id = rack.add_module(ModuleType::Lpf);
        let filter2_id = rack.add_module(ModuleType::Lpf);

        // Try to use input port as source
        let connection = Connection::new(
            PortRef::new(filter1_id, "in"), // "in" is an input, not output
            PortRef::new(filter2_id, "in"),
        );

        let result = rack.add_connection(connection);
        assert!(matches!(result, Err(ConnectionError::SourceNotOutput(_, _))));
    }

    #[test]
    fn test_add_connection_dest_not_input() {
        let mut rack = RackState::new();
        let osc1_id = rack.add_module(ModuleType::SawOsc);
        let osc2_id = rack.add_module(ModuleType::SawOsc);

        // Try to connect to output port
        let connection = Connection::new(
            PortRef::new(osc1_id, "out"),
            PortRef::new(osc2_id, "out"), // "out" is an output, not input
        );

        let result = rack.add_connection(connection);
        assert!(matches!(result, Err(ConnectionError::DestNotInput(_, _))));
    }

    #[test]
    fn test_add_connection_already_exists() {
        let mut rack = RackState::new();
        let osc_id = rack.add_module(ModuleType::SawOsc);
        let filter_id = rack.add_module(ModuleType::Lpf);

        let connection = Connection::new(
            PortRef::new(osc_id, "out"),
            PortRef::new(filter_id, "in"),
        );

        assert!(rack.add_connection(connection.clone()).is_ok());
        let result = rack.add_connection(connection);
        assert!(matches!(result, Err(ConnectionError::AlreadyConnected)));
    }

    #[test]
    fn test_remove_connection() {
        let mut rack = RackState::new();
        let osc_id = rack.add_module(ModuleType::SawOsc);
        let filter_id = rack.add_module(ModuleType::Lpf);

        let connection = Connection::new(
            PortRef::new(osc_id, "out"),
            PortRef::new(filter_id, "in"),
        );

        rack.add_connection(connection.clone()).unwrap();
        assert!(rack.remove_connection(&connection));
        assert!(rack.connections.is_empty());
    }

    #[test]
    fn test_connections_from() {
        let mut rack = RackState::new();
        let osc_id = rack.add_module(ModuleType::SawOsc);
        let filter_id = rack.add_module(ModuleType::Lpf);
        let output_id = rack.add_module(ModuleType::Output);

        let conn1 = Connection::new(
            PortRef::new(osc_id, "out"),
            PortRef::new(filter_id, "in"),
        );
        let conn2 = Connection::new(
            PortRef::new(filter_id, "out"),
            PortRef::new(output_id, "in"),
        );

        rack.add_connection(conn1).unwrap();
        rack.add_connection(conn2).unwrap();

        let from_osc = rack.connections_from(osc_id);
        assert_eq!(from_osc.len(), 1);
        assert_eq!(from_osc[0].dst.module_id, filter_id);

        let from_filter = rack.connections_from(filter_id);
        assert_eq!(from_filter.len(), 1);
        assert_eq!(from_filter[0].dst.module_id, output_id);
    }

    #[test]
    fn test_connections_to() {
        let mut rack = RackState::new();
        let osc_id = rack.add_module(ModuleType::SawOsc);
        let lfo_id = rack.add_module(ModuleType::Lfo);
        let filter_id = rack.add_module(ModuleType::Lpf);

        let conn1 = Connection::new(
            PortRef::new(osc_id, "out"),
            PortRef::new(filter_id, "in"),
        );
        let conn2 = Connection::new(
            PortRef::new(lfo_id, "out"),
            PortRef::new(filter_id, "cutoff_mod"),
        );

        rack.add_connection(conn1).unwrap();
        rack.add_connection(conn2).unwrap();

        let to_filter = rack.connections_to(filter_id);
        assert_eq!(to_filter.len(), 2);
    }

    #[test]
    fn test_remove_module_cascades_connections() {
        let mut rack = RackState::new();
        let osc_id = rack.add_module(ModuleType::SawOsc);
        let filter_id = rack.add_module(ModuleType::Lpf);
        let output_id = rack.add_module(ModuleType::Output);

        let conn1 = Connection::new(
            PortRef::new(osc_id, "out"),
            PortRef::new(filter_id, "in"),
        );
        let conn2 = Connection::new(
            PortRef::new(filter_id, "out"),
            PortRef::new(output_id, "in"),
        );

        rack.add_connection(conn1).unwrap();
        rack.add_connection(conn2).unwrap();
        assert_eq!(rack.connections.len(), 2);

        // Remove filter - should remove both connections
        rack.remove_module(filter_id);
        assert_eq!(rack.connections.len(), 0);
    }

    #[test]
    fn test_save_load_with_connections() {
        use std::fs;
        use tempfile::tempdir;

        let mut rack = RackState::new();
        let osc_id = rack.add_module(ModuleType::SawOsc);
        let filter_id = rack.add_module(ModuleType::Lpf);
        let output_id = rack.add_module(ModuleType::Output);

        let conn1 = Connection::new(
            PortRef::new(osc_id, "out"),
            PortRef::new(filter_id, "in"),
        );
        let conn2 = Connection::new(
            PortRef::new(filter_id, "out"),
            PortRef::new(output_id, "in"),
        );

        rack.add_connection(conn1.clone()).unwrap();
        rack.add_connection(conn2.clone()).unwrap();

        // Save to temp file
        let dir = tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("test_connections.tuidaw");
        rack.save(&path).expect("Failed to save");

        // Load and verify
        let loaded = RackState::load(&path).expect("Failed to load");

        assert_eq!(loaded.connections.len(), 2);
        assert!(loaded.connections.contains(&conn1));
        assert!(loaded.connections.contains(&conn2));

        // Clean up
        fs::remove_file(&path).ok();
    }
}
