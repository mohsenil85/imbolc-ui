use std::fmt;

use serde::{Deserialize, Serialize};

use super::ModuleId;

/// Reference to a specific port on a module
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PortRef {
    pub module_id: ModuleId,
    pub port_name: String,
}

impl PortRef {
    pub fn new(module_id: ModuleId, port_name: impl Into<String>) -> Self {
        Self {
            module_id,
            port_name: port_name.into(),
        }
    }
}

impl fmt::Display for PortRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.module_id, self.port_name)
    }
}

/// A connection between two module ports
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Connection {
    /// Source port (output)
    pub src: PortRef,
    /// Destination port (input)
    pub dst: PortRef,
}

impl Connection {
    pub fn new(src: PortRef, dst: PortRef) -> Self {
        Self { src, dst }
    }
}

impl fmt::Display for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.src, self.dst)
    }
}

/// Errors that can occur when managing connections
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionError {
    /// Source module does not exist
    SourceModuleNotFound(ModuleId),
    /// Destination module does not exist
    DestModuleNotFound(ModuleId),
    /// Source port does not exist on the module
    SourcePortNotFound(ModuleId, String),
    /// Destination port does not exist on the module
    DestPortNotFound(ModuleId, String),
    /// Source port is not an output
    SourceNotOutput(ModuleId, String),
    /// Destination port is not an input
    DestNotInput(ModuleId, String),
    /// Connection already exists
    AlreadyConnected,
    /// Would create a cycle in the signal graph
    WouldCreateCycle,
}

impl fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionError::SourceModuleNotFound(id) => {
                write!(f, "source module {} not found", id)
            }
            ConnectionError::DestModuleNotFound(id) => {
                write!(f, "destination module {} not found", id)
            }
            ConnectionError::SourcePortNotFound(id, port) => {
                write!(f, "port '{}' not found on module {}", port, id)
            }
            ConnectionError::DestPortNotFound(id, port) => {
                write!(f, "port '{}' not found on module {}", port, id)
            }
            ConnectionError::SourceNotOutput(id, port) => {
                write!(f, "port '{}' on module {} is not an output", port, id)
            }
            ConnectionError::DestNotInput(id, port) => {
                write!(f, "port '{}' on module {} is not an input", port, id)
            }
            ConnectionError::AlreadyConnected => write!(f, "connection already exists"),
            ConnectionError::WouldCreateCycle => write!(f, "would create a cycle"),
        }
    }
}

impl std::error::Error for ConnectionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_ref_creation() {
        let port = PortRef::new(1, "out");
        assert_eq!(port.module_id, 1);
        assert_eq!(port.port_name, "out");
    }

    #[test]
    fn test_port_ref_display() {
        let port = PortRef::new(5, "cutoff_mod");
        assert_eq!(format!("{}", port), "5:cutoff_mod");
    }

    #[test]
    fn test_connection_creation() {
        let src = PortRef::new(1, "out");
        let dst = PortRef::new(2, "in");
        let conn = Connection::new(src.clone(), dst.clone());
        assert_eq!(conn.src, src);
        assert_eq!(conn.dst, dst);
    }

    #[test]
    fn test_connection_display() {
        let conn = Connection::new(PortRef::new(1, "out"), PortRef::new(2, "in"));
        assert_eq!(format!("{}", conn), "1:out -> 2:in");
    }

    #[test]
    fn test_connection_equality() {
        let conn1 = Connection::new(PortRef::new(1, "out"), PortRef::new(2, "in"));
        let conn2 = Connection::new(PortRef::new(1, "out"), PortRef::new(2, "in"));
        let conn3 = Connection::new(PortRef::new(1, "out"), PortRef::new(3, "in"));

        assert_eq!(conn1, conn2);
        assert_ne!(conn1, conn3);
    }

    #[test]
    fn test_connection_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        let conn1 = Connection::new(PortRef::new(1, "out"), PortRef::new(2, "in"));
        let conn2 = Connection::new(PortRef::new(1, "out"), PortRef::new(2, "in"));

        set.insert(conn1);
        assert!(!set.insert(conn2)); // Should return false (already present)
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_error_display() {
        let err = ConnectionError::SourceModuleNotFound(42);
        assert_eq!(format!("{}", err), "source module 42 not found");

        let err = ConnectionError::SourcePortNotFound(1, "out".to_string());
        assert_eq!(format!("{}", err), "port 'out' not found on module 1");
    }
}
