# Developer Guide

This guide covers development workflows, code patterns, and best practices for contributing to the Cilium IPIP Router project.

## Getting Started

### Prerequisites

- Rust 1.82.0 or later
- kubeconfig with cluster access
- Docker (optional, for containerized builds)

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
├── main.rs           # Entry point, no modules
├── lib.rs            # Module declarations only
└── controller/       # Business logic
    ├── mod.rs        # Module exports
    ├── builder.rs    # Builder pattern
    ├── handle.rs     # Handle pattern
    └── root.rs       # Core logic
```

### Naming Conventions

| Pattern | Example |
|---------|---------|
| Modules | `controller`, `builder`, `handle` |
| Structs | `Controller`, `ControllerBuilder` |
| Enums | `ControllerCommand` |
| Functions | `run()`, `watch()`, `update_route()` |
| Constants | `MAX_RETRIES` |

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
async fn test_controller_builder() {
    let builder = ControllerBuilder::new();
    assert!(builder.cmd_tx.is_some());
}

// Mock external dependencies
#[cfg(test)]
use mockall::automock;

#[automock]
pub trait ControllerInterface {
    fn run(&self) -> Result<(), anyhow::Error>;
}
```

## Git Workflow

### Commit Messages

Follow conventional commits:
- `feat: Add new routing feature`
- `fix: Handle node deletion errors`
- `docs: Update README`
- `test: Add controller tests`
- `refactor: Simplify watch loop`

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

## Security Guidelines

- Never hardcode secrets (use environment variables)
- Validate all Kubernetes API responses
- Handle errors before they propagate
- Review dependencies for vulnerabilities
