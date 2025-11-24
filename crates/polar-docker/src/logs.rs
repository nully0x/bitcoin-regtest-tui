//! Docker log streaming.

use bollard::container::LogsOptions;
use bollard::Docker;
use futures_util::StreamExt;
use tokio::sync::mpsc;

/// A handle to a log stream.
pub struct LogStream {
    /// Receiver for log lines.
    pub rx: mpsc::Receiver<String>,
}

impl LogStream {
    /// Start streaming logs from a container.
    pub fn start(docker: Docker, container_id: String) -> Self {
        let (tx, rx) = mpsc::channel(256);

        tokio::spawn(async move {
            let options = LogsOptions::<String> {
                follow: true,
                stdout: true,
                stderr: true,
                tail: "100".to_string(),
                ..Default::default()
            };

            let mut stream = docker.logs(&container_id, Some(options));

            while let Some(result) = stream.next().await {
                match result {
                    Ok(output) => {
                        let line = output.to_string();
                        if tx.send(line).await.is_err() {
                            // Receiver dropped, stop streaming
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!("Log stream error: {}", e);
                        break;
                    }
                }
            }
        });

        Self { rx }
    }
}
