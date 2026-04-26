# Cilium IPIP Router

A node-local router that manages IPIP routing for Cilium CNI using kernel routing on each node.

## Overview

This Rust-based router creates and manages IPIP tunnels for Cilium network overlay on each node. It uses direct kernel routing to manage traffic without a central controller. Each router instance operates independently on its assigned node.

## Project Structure

```
/git/work/
├── src/
│   ├── main.rs              # Application entry point
│   ├── lib.rs               # Library module declarations
│   └── router/
│       ├── mod.rs           # Router module exports
│       └── handle.rs        # Router handle implementation
├── resources/
│   └── log4rs.yaml          # Logging configuration
├── Dockerfile               # Multi-stage Docker build
├── Cargo.toml               # Rust dependencies
├── Cargo.lock               # Dependency lock file
└── .github/
    └── workflows/           # CI/CD pipelines
        ├── deploy.yaml      # Docker image deployment
        ├── test.yaml        # Test execution
        ├── coverage.yml     # Coverage reporting
        └── docker.yaml      # Docker build validation
```

## Project Specifications

### Core Functionality

- **Node-local IPIP Routing**: Creates and manages IPIP tunnels using kernel routing on each node
- **Cilium Support**: Updated for Cilium 1.18+ compatibility
- **Kernel Routing**: Direct manipulation of kernel routing tables for tunnel traffic
- **Kubernetes Version**: Compatible with Kubernetes 1.34
- **Node-local Monitoring**: Each instance manages only its assigned node's routes
- **Graceful Shutdown**: Handles SIGTERM and SIGINT signals for clean termination

### Technical Stack

| Component | Technology |
|-----------|------------|
| Runtime | Tokio (async runtime) |
| Web Framework | Actix Web 4.9.0 |
| Kubernetes Client | kube 0.96.0 |
| Logging | log + log4rs 1.3.0 |
| Metrics | prometheus-client 0.22.3 |
| Error Handling | anyhow 1.0.89 |

### Key Dependencies

- `actix-web`: HTTP server framework
- `kube`: Kubernetes client library
- `tokio`: Async runtime with signal handling
- `log4rs`: Configurable logging system
- `mockall`: Unit test mocking framework

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Kubernetes Cluster                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                  │
│  │  Node 1  │  │  Node 2  │  │  Node N  │                  │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘                  │
│       │             │             │                         │
│  ┌────▼─────┐  ┌────▼─────┐  ┌────▼─────┐                 │
│  │  Router  │  │  Router  │  │  Router  │                 │
│  │  - Local │  │  - Local │  │  - Local │                 │
│  │  IPIP    │  │  IPIP    │  │  IPIP    │                 │
│  │  Tunnel  │  │  Tunnel  │  │  Tunnel  │                 │
│  │  Kernel  │  │  Kernel  │  │  Kernel  │                 │
│  │  Routes  │  │  Routes  │  │  Routes  │                 │
│  └──────────┘  └──────────┘  └──────────┘                 │
│                                                             │
│  Each router manages only its node's IPIP routes via      │
│  direct kernel routing - no central controller.            │
└─────────────────────────────────────────────────────────────┘
```

## How to Use

### Prerequisites

- Rust 1.82.0 or later
- Kubernetes cluster access (kubeconfig)
- Docker (for containerized deployment)

### Building from Source

```bash
# Clone the repository
git clone <repository-url>
cd cilium-ipip-router

# Build in release mode
cargo build --release

# Or build for specific target
cargo build --release --target x86_64-unknown-linux-gnu
```

### Running Locally

```bash
# Ensure kubeconfig is configured
export KUBECONFIG=/path/to/kubeconfig

# Run the application
cargo run --target x86_64-unknown-linux-gnu
```

The application will:
1. Initialize logging via `resources/log4rs.yaml`
2. Create IPIP tunnel for local node
3. Update kernel routing tables
4. Start HTTP server on `0.0.0.0:9090`
5. Expose `/health` endpoint for health checks

### Docker Deployment

```bash
# Build Docker image
docker build -t cilium-ipip-router:latest .

# Run container
docker run -it --rm \
  --name cilium-router \
  cilium-ipip-router:latest
```

### Kubernetes Deployment

The router is deployed as a DaemonSet to ensure each node has its own IPIP routing instance. Each instance manages routes for its assigned node using kernel routing.

## Configuration

### Logging

Configure logging in `resources/log4rs.yaml`:
- Default level: `info`
- Output: stdout
- Scan interval: 30 seconds for config changes

### Environment Variables

None required. The application uses:
- Default kubeconfig discovery (in-cluster or ~/.kube/config)
- Hardcoded server binding: `0.0.0.0:9090`

## API Endpoints

### Health Check

```
GET /health
```

Response:
```json
"healthy"
```

## Code Structure

### Module Structure

All code follows this organization:

```
src/
├── main.rs           # Application entry point
├── lib.rs            # Module re-exports only
└── router/
    ├── mod.rs        # Module declarations and re-exports
    └── handle.rs     # Router handle implementation
```

### Naming Conventions

- **Modules**: snake_case (`router`, `handle`)
- **Structs**: PascalCase (`RouterHandle`)
- **Functions**: snake_case (`run`, `watch`, `update_route`)
- **Constants**: SCREAMING_SNAKE_CASE

### Error Handling

- Use `anyhow::Result` for application errors
- Use `std::io::Result` for Future implementations
- Log errors with `log::error!` before returning
- Avoid `unwrap()` in production code

### Async Patterns

- Use `#[tokio::main]` for async entry points
- Use `BoxFuture<'static, io::Result<()>>` for long-running futures
- Use `tokio::select!` for concurrent event handling

### Testing Guidelines

1. **Test Location**: Place tests in same file as code
2. **Async Tests**: Use `#[tokio::test]` attribute
3. **Mocking**: Use `mockall` for external dependencies
4. **Isolation**: Test without real Kubernetes API when possible

## Development Guidelines

### Code Style

- Run formatter before commit: `cargo fmt --all`
- Run linter: `cargo clippy --target x86_64-unknown-linux-gnu -- -D warnings`
- Check for warnings: Enable `-D warnings` in CI

### Commit Policy

- Author must be configured (not `opencode`)
- Use descriptive commit messages
- Follow conventional commits format recommended

### CI/CD Pipeline

- **Test Workflow**: Runs on pull requests and pushes to main
- **Deploy Workflow**: Pushes Docker images on tags and main branch
- **Coverage**: Generates coverage reports for each build
- **Docker**: Validates Docker build process

## Monitoring

The application exposes:
- Prometheus metrics endpoint (via `prometheus-client`)
- Health check endpoint at `/health`
- Structured logging via `log4rs`

## License

MIT

## Documentation

For more detailed information, see the [documentation](docs/) directory:

- [Architecture Overview](docs/architecture.md)


## Contributing

1. Follow existing code patterns and structure
2. Run `cargo fmt` and `cargo clippy` before committing
3. Add tests for new functionality
4. Update documentation as needed
