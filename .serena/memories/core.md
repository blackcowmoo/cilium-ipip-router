# Core Project Information

## Project Overview

cilium-ipip-router is a Kubernetes controller that manages IPIP tunnels for node-to-node communication. It watches Kubernetes Node resources and automatically creates/updates/deletes IPIP tunnels and routes.

## Entry Point

The application entry point is `src/bin/router.rs` which initializes the controller and starts watching nodes.

## Module Structure

- `src/lib.rs` - Library root, exports `controller` and `ipip` modules
- `src/controller/` - Controller logic for watching nodes and managing tunnels
  - `mod.rs` - Module declarations
  - `root.rs` - Main controller implementation (`ControllerInner` struct and `watch` function)
  - `builder.rs` - `ControllerBuilder` for constructing controller instances
  - `handle.rs` - `ControllerHandle` for controlling the controller
  - `ipip_tests.rs` - Tests for IPIP-related functionality
  - `builder.rs` - Tests for ControllerBuilder
  - `handle.rs` - Tests for ControllerHandle and ControllerCommand
- `src/ipip/` - IPIP tunnel management logic
  - `mod.rs` - Module re-exports
  - `executor.rs` - Core IPIP operations (tunnel management, route updates)
- `src/bin/` - Binary targets
  - `router.rs` - Application entry point

## Key Invariants

- All IPIP operations are abstracted behind `IpCommandExecutor` trait for testability
- Controller uses async/await pattern with tokio runtime
- Node watching is done via kube crate's Watch API
- Tunnel names are deterministic (MD5 hash of node name, truncated to 11 chars + "tun-" prefix)
- Maximum tunnel name length is 15 characters (Linux interface name limit)

## Testing Strategy

- Unit tests in respective modules (builder, handle, ipip_tests)
- Integration tests in `tests/` directory
- Mocking pattern uses `IpCommandExecutor` trait implementation for testing IPIP commands
- Controller tests verify channel communication and command handling
