# Developer Guide

This guide covers development workflows, code patterns, and best practices for contributing to the Cilium IPIP Router project.

## Getting Started

### Prerequisites

- Rust 1.82.0 or later
- kubeconfig with cluster access
- Docker (optional, for containerized builds)
- Kubernetes cluster (v1.34+)
- Cilium CNI v1.18+

### Setting Up Development Environment

```bash
# Clone repository
git clone <repository-url>
cd cilium-ipip-router

# Install Rust toolchain
rustup override set 1.82.0

# Install dependencies
cargo build
```

## Code Structure

### Module Organization

```
src/
├── main.rs           # Entry point
├── lib.rs            # Module declarations only
└── controller/       # Domain logic (legacy)
    ├── mod.rs        # Module exports
    ├── builder.rs    # Builder pattern (legacy)
    ├── handle.rs     # Handle pattern (legacy)
    └── root.rs       # Core logic (legacy)
```

### Naming Conventions

| Pattern | Example |
|---------|---------|
| Modules | `main`, `lib` |
| Structs | `Router`, `RouterBuilder` |
| Enums | `RouterCommand` |
| Functions | `run()`, `setup_routes()`, `create_tunnel()` |
| Constants | `MAX_RETRIES` |

Note: Legacy controller naming conventions are shown for reference but should not be used for new code.

## Development Workflows

### Building

```bash
# Development build
cargo build

# Release build
cargo build --release

# Target-specific build
cargo build --release --target x86_64-unknown-linux-gnu
```

### Testing

```bash
# Run all tests
cargo test --target x86_64-unknown-linux-gnu

# Run specific test
cargo test --target x86_64-unknown-linux-gnu -- test_name

# Run with coverage
cargo test --target x86_64-unknown-linux-gnu -- --nocapture
```

### Code Quality

```bash
# Format code
cargo fmt --all

# Check formatting without changes
cargo fmt --all -- --check

# Run linter
cargo clippy --target x86_64-unknown-linux-gnu -- -D warnings
```

## Coding Standards

### Error Handling

```rust
// Good: Use anyhow::Result for application errors
pub fn run() -> anyhow::Result<()> {
    let config = load_config()?;
    Ok(())
}

// Good: Use io::Result for Future implementations
impl Future for Controller {
    type Output = io::Result<()>;
    // ...
}
```

### Async Patterns

```rust
// Use BoxFuture for long-running operations
type Fut = BoxFuture<'static, io::Result<()>>;

// Use tokio::select! for concurrent events
tokio::select! {
    result = watch_stream => { /* handle */ }
    _ = shutdown_signal => { /* cleanup */ }
}
```

### Testing

```rust
// Use #[tokio::test] for async tests
#[tokio::test]
async fn test_router_setup() {
    // Test router initialization
    assert!(true); // Replace with actual tests
}

// Mock external dependencies
#[cfg(test)]
use mockall::automock;

#[automock]
pub trait RouterInterface {
    fn run(&self) -> Result<(), anyhow::Error>;
}
```

## Git Workflow

### Commit Messages

Follow conventional commits:
- `feat: Add new routing feature`
- `fix: Handle node deletion errors`
- `docs: Update README`
- `test: Add router tests`
- `refactor: Simplify route setup`

### Branch Strategy

- `main`: Stable, production-ready code
- `feature/*`: New features
- `fix/*`: Bug fixes
- `docs/*`: Documentation updates

### Pre-commit Checklist

- [ ] Code formatted: `cargo fmt --all`
- [ ] Lint passes: `cargo clippy -- -D warnings`
- [ ] Tests pass: `cargo test`
- [ ] Documentation updated
- [ ] No hardcoded secrets

## Debugging

### Logging

```bash
# View logs
cargo run --target x86_64-unknown-linux-gnu

# Logs are written to stdout with timestamps
```

### Debug Build

```bash
# Enable debug symbols
cargo build --target x86_64-unknown-linux-gnu --debug

# Run with GDB
gdb target/x86_64-unknown-linux-gnu/debug/router
```

## Performance Considerations

- Use `BoxFuture` for long-running futures to avoid stack overflow
- Limit watch stream buffer sizes
- Graceful shutdown with timeout (30 seconds)
- Avoid blocking operations in async context
- Kernel routing operations should be non-blocking

## Security Guidelines

- Never hardcode secrets (use environment variables)
- Validate all node configuration
- Handle errors before they propagate
- Review dependencies for vulnerabilities
- Ensure privileged operations are minimal and secure
