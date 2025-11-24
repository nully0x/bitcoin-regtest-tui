//! Port mapping abstraction for Docker containers.

use bollard::service::PortBinding;
use std::collections::HashMap;

/// Simple port mapping type: container_port -> host_port.
///
/// This provides a clean abstraction over Docker's port binding implementation,
/// hiding the complexity of bollard types from other crates.
#[derive(Debug, Clone, Default)]
pub struct PortMap {
    mappings: HashMap<u16, u16>,
}

impl PortMap {
    /// Create a new empty port map.
    pub fn new() -> Self {
        Self {
            mappings: HashMap::new(),
        }
    }

    /// Add a port mapping from container port to host port.
    pub fn add(&mut self, container_port: u16, host_port: u16) -> &mut Self {
        self.mappings.insert(container_port, host_port);
        self
    }

    /// Convert to bollard's PortBinding format.
    ///
    /// This is an internal implementation detail that converts our simple
    /// port map to Docker's expected format.
    pub(crate) fn to_bollard_bindings(&self) -> HashMap<String, Option<Vec<PortBinding>>> {
        let mut bindings = HashMap::new();

        for (container_port, host_port) in &self.mappings {
            bindings.insert(
                format!("{}/tcp", container_port),
                Some(vec![PortBinding {
                    host_ip: Some("0.0.0.0".to_string()),
                    host_port: Some(host_port.to_string()),
                }]),
            );
        }

        bindings
    }

    /// Check if the port map is empty.
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    /// Get the number of port mappings.
    pub fn len(&self) -> usize {
        self.mappings.len()
    }
}

impl From<Vec<(u16, u16)>> for PortMap {
    fn from(mappings: Vec<(u16, u16)>) -> Self {
        let mut port_map = PortMap::new();
        for (container_port, host_port) in mappings {
            port_map.add(container_port, host_port);
        }
        port_map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_map_creation() {
        let mut port_map = PortMap::new();
        port_map.add(8080, 20000);
        port_map.add(9735, 20001);

        assert_eq!(port_map.len(), 2);
        assert!(!port_map.is_empty());
    }

    #[test]
    fn test_port_map_from_vec() {
        let port_map = PortMap::from(vec![(8080, 20000), (9735, 20001)]);
        assert_eq!(port_map.len(), 2);
    }

    #[test]
    fn test_bollard_conversion() {
        let mut port_map = PortMap::new();
        port_map.add(8080, 20000);

        let bindings = port_map.to_bollard_bindings();
        assert!(bindings.contains_key("8080/tcp"));

        let binding = bindings.get("8080/tcp").unwrap().as_ref().unwrap();
        assert_eq!(binding[0].host_port.as_deref(), Some("20000"));
        assert_eq!(binding[0].host_ip.as_deref(), Some("0.0.0.0"));
    }
}
