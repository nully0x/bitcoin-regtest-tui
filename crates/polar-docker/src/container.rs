//! Docker container management.

use crate::PortMap;
use bollard::Docker;
use bollard::container::{
    Config, CreateContainerOptions, RemoveContainerOptions, StartContainerOptions,
    StopContainerOptions,
};
use polar_core::{Error, Result};

/// Manages Docker containers for nodes.
pub struct ContainerManager {
    docker: Docker,
}

impl ContainerManager {
    /// Create a new container manager.
    pub fn new() -> Result<Self> {
        let docker =
            Docker::connect_with_local_defaults().map_err(|e| Error::Docker(e.to_string()))?;
        Ok(Self { docker })
    }

    /// Create a new container manager with a custom socket path.
    pub fn with_socket(socket_path: &str) -> Result<Self> {
        let docker = Docker::connect_with_socket(socket_path, 120, bollard::API_DEFAULT_VERSION)
            .map_err(|e| Error::Docker(e.to_string()))?;
        Ok(Self { docker })
    }

    /// Get a reference to the Docker client.
    pub fn docker(&self) -> &Docker {
        &self.docker
    }

    /// Create a container.
    pub async fn create_container(
        &self,
        name: &str,
        image: &str,
        cmd: Option<Vec<String>>,
    ) -> Result<String> {
        self.create_container_with_config(name, image, cmd, None, None)
            .await
    }

    /// Create a container with advanced configuration.
    pub async fn create_container_with_config(
        &self,
        name: &str,
        image: &str,
        cmd: Option<Vec<String>>,
        port_map: Option<PortMap>,
        network: Option<&str>,
    ) -> Result<String> {
        use bollard::service::{EndpointSettings, HostConfig};
        use std::collections::HashMap;

        let options = CreateContainerOptions {
            name,
            ..Default::default()
        };

        // Convert PortMap to bollard's port binding format
        let port_bindings = port_map.as_ref().map(|pm| pm.to_bollard_bindings());

        // Build exposed ports map if we have port bindings
        let exposed_ports = port_bindings.as_ref().map(|bindings| {
            bindings
                .keys()
                .map(|port| (port.clone(), HashMap::new()))
                .collect::<HashMap<_, _>>()
        });

        let mut networks_config = HashMap::new();
        if let Some(net) = network {
            networks_config.insert(
                net.to_string(),
                EndpointSettings {
                    ..Default::default()
                },
            );
        }

        let config = Config {
            image: Some(image.to_string()),
            cmd: cmd.map(|c| c.into_iter().collect()),
            exposed_ports,
            host_config: Some(HostConfig {
                port_bindings,
                ..Default::default()
            }),
            ..Default::default()
        };

        let response = self
            .docker
            .create_container(Some(options), config)
            .await
            .map_err(|e| Error::Docker(e.to_string()))?;

        // Connect to network if specified
        if let Some(net) = network {
            use bollard::network::ConnectNetworkOptions;
            let connect_options = ConnectNetworkOptions {
                container: response.id.clone(),
                endpoint_config: Default::default(),
            };

            if let Err(e) = self.docker.connect_network(net, connect_options).await {
                // Network might not exist or container might already be connected
                // Log but don't fail
                eprintln!("Warning: Failed to connect to network {}: {}", net, e);
            }
        }

        Ok(response.id)
    }

    /// Create a Docker network.
    pub async fn create_network(&self, name: &str) -> Result<String> {
        use bollard::network::CreateNetworkOptions;

        let options = CreateNetworkOptions {
            name: name.to_string(),
            check_duplicate: true,
            driver: "bridge".to_string(),
            ..Default::default()
        };

        let response = self
            .docker
            .create_network(options)
            .await
            .map_err(|e| Error::Docker(e.to_string()))?;

        Ok(response.id)
    }

    /// Remove a Docker network.
    pub async fn remove_network(&self, name: &str) -> Result<()> {
        self.docker
            .remove_network(name)
            .await
            .map_err(|e| Error::Docker(e.to_string()))?;
        Ok(())
    }

    /// Start a container.
    pub async fn start_container(&self, container_id: &str) -> Result<()> {
        self.docker
            .start_container(container_id, None::<StartContainerOptions<String>>)
            .await
            .map_err(|e| Error::Docker(e.to_string()))?;
        Ok(())
    }

    /// Stop a container.
    pub async fn stop_container(&self, container_id: &str) -> Result<()> {
        let options = StopContainerOptions { t: 10 };
        self.docker
            .stop_container(container_id, Some(options))
            .await
            .map_err(|e| Error::Docker(e.to_string()))?;
        Ok(())
    }

    /// Remove a container.
    pub async fn remove_container(&self, container_id: &str) -> Result<()> {
        let options = RemoveContainerOptions {
            force: true,
            ..Default::default()
        };
        self.docker
            .remove_container(container_id, Some(options))
            .await
            .map_err(|e| Error::Docker(e.to_string()))?;
        Ok(())
    }

    /// Check if Docker is available.
    pub async fn ping(&self) -> Result<()> {
        self.docker
            .ping()
            .await
            .map_err(|e| Error::Docker(e.to_string()))?;
        Ok(())
    }

    /// Pull a Docker image.
    pub async fn pull_image(&self, image: &str) -> Result<()> {
        use bollard::image::CreateImageOptions;
        use futures_util::StreamExt;

        let options = Some(CreateImageOptions {
            from_image: image,
            ..Default::default()
        });

        let mut stream = self.docker.create_image(options, None, None);

        while let Some(result) = stream.next().await {
            match result {
                Ok(_info) => {
                    // Progress update - could log this
                }
                Err(e) => {
                    return Err(Error::Docker(format!(
                        "Failed to pull image {}: {}",
                        image, e
                    )));
                }
            }
        }

        Ok(())
    }

    /// Check if an image exists locally.
    pub async fn image_exists(&self, image: &str) -> Result<bool> {
        use bollard::image::ListImagesOptions;
        use std::collections::HashMap;

        let mut filters = HashMap::new();
        filters.insert("reference".to_string(), vec![image.to_string()]);

        let options = Some(ListImagesOptions {
            filters,
            ..Default::default()
        });

        let images = self
            .docker
            .list_images(options)
            .await
            .map_err(|e| Error::Docker(e.to_string()))?;

        Ok(!images.is_empty())
    }

    /// Pull image if it doesn't exist locally.
    pub async fn ensure_image(&self, image: &str) -> Result<()> {
        if !self.image_exists(image).await? {
            self.pull_image(image).await?;
        }
        Ok(())
    }

    /// Execute a command in a running container and return the output.
    pub async fn exec_command(&self, container_id: &str, cmd: Vec<&str>) -> Result<String> {
        use bollard::exec::{CreateExecOptions, StartExecResults};
        use futures_util::StreamExt;

        // Create exec instance
        let exec = self
            .docker
            .create_exec(
                container_id,
                CreateExecOptions {
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    cmd: Some(cmd),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| Error::Docker(format!("Failed to create exec: {}", e)))?;

        // Start and collect output
        let mut output = Vec::new();
        if let StartExecResults::Attached {
            output: mut stream, ..
        } = self
            .docker
            .start_exec(&exec.id, None)
            .await
            .map_err(|e| Error::Docker(format!("Failed to start exec: {}", e)))?
        {
            while let Some(Ok(msg)) = stream.next().await {
                use bollard::container::LogOutput;
                match msg {
                    LogOutput::StdOut { message } | LogOutput::StdErr { message } => {
                        output.extend_from_slice(&message);
                    }
                    _ => {}
                }
            }
        }

        String::from_utf8(output)
            .map_err(|e| Error::Docker(format!("Failed to parse command output: {}", e)))
    }

    /// Get container inspection details.
    pub async fn inspect_container(
        &self,
        container_id: &str,
    ) -> Result<bollard::models::ContainerInspectResponse> {
        self.docker
            .inspect_container(container_id, None)
            .await
            .map_err(|e| Error::Docker(format!("Failed to inspect container: {}", e)))
    }
}
