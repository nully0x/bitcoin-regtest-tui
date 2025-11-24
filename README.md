# Polar TUI

A terminal user interface for managing Lightning Network development environments.

## Features

- **Interactive TUI**: Full-featured terminal interface for managing Lightning networks
- **Multiple Networks**: Run multiple Lightning Network test environments simultaneously with automatic port allocation
- **External Access**: All nodes are accessible from your host machine with unique port assignments
- **Bitcoin Core & LND Support**: Pre-configured Bitcoin Core and LND nodes
- **Real-time Monitoring**: Live container log streaming and node status
- **Vim-style Navigation**: Familiar keyboard shortcuts for efficient workflow

## Requirements

- Rust 1.85+
- Docker

## Installation

```bash
 I have not determined install create yet(name can change)
```

## Usage

Launch the TUI (this is the primary interface):

```bash
polar
```

The TUI provides a complete interface for managing your Lightning networks. All operations can be performed through the interactive interface.

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Tab` | Switch between panels |
| `j/k` | Navigate down/up in lists |
| `Enter` | Select network or view details | 
| `a` | Add new node to selected network |
| `n` | Create new network |
| `s` | Start selected network |
| `x` | Stop selected network |
| `d` | Delete selected network |
| `l` | View container logs |
| `q` | Quit application |

## Network Configuration

Each network you create automatically gets:
- **Unique Port Assignments**: Starting from port 20000, each node gets its own set of ports
- **Bitcoin Core Ports**: RPC (18443→host), P2P (18444→host), ZMQ Block & TX
- **LND Ports**: REST API (8080→host), gRPC (10009→host), P2P (9735→host)
- **Isolated Docker Network**: Each network runs in its own Docker bridge network
- **Persistent Configuration**: Network state and port mappings are saved across restarts

## Project Structure

```
crates/
  polar-cli/     # CLI binary
  polar-tui/     # TUI rendering
  polar-core/    # Core types and config
  polar-docker/  # Docker management
  polar-nodes/   # Node implementations
```

## License

MIT
