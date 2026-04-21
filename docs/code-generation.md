# Code Generation Guidelines

This document outlines the guidelines and best practices for code generation in the Cilium IPIP Router project.

## Overview

Code generation is used for:

- API client code
- Serialization/deserialization
- Test fixtures

## Guidelines

### 1. Use Kubernetes Code Generators

For Kubernetes API clients, use `kube-derive`:

```rust
use kube::CustomResource;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Debug, Serialize, Deserialize)]
#[kube(group = "cilium.io", version = "v2", kind = "CiliumEndpoint", namespaced)]
pub struct CiliumEndpointSpec {
    pub title: String,
    pub content: String,
}
```

**Note**: For Cilium 1.18+ compatibility, ensure CRD definitions use `apiextensions.k8s.io/v1` instead of the deprecated `apiextensions.k8s.io/v1beta1`.

### 2. Code Generation Commands

```bash
# Update dependencies
cargo update

# Rebuild with generated code
cargo build --target x86_64-unknown-linux-gnu
```

### 3. Code Generation Patterns

#### Router Pattern

Each router instance manages IPIP tunnels using kernel routing:

```rust
pub struct Router {
    handle: RouterHandle,
    fut: BoxFuture<'static, io::Result<()>>,
}

impl Router {
    pub fn builder() -> RouterBuilder {
        RouterBuilder::default()
    }
    
    pub fn new(builder: RouterBuilder) -> Self {
        Router {
            handle: RouterHandle::new(builder.cmd_tx.clone()),
            fut: Box::pin(RouterInner::run(builder)),
        }
    }
    
    pub fn handle(&self) -> RouterHandle {
        self.handle.clone()
    }
}
```

#### RouterBuilder Pattern

```rust
pub struct RouterBuilder {
    pub cmd_tx: UnboundedSender<RouterCommand>,
    pub cmd_rx: UnboundedReceiver<RouterCommand>,
}

impl Default for RouterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RouterBuilder {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = unbounded_channel();
        RouterBuilder { cmd_tx, cmd_rx }
    }
}
```

#### RouterHandle Pattern

```rust
pub struct RouterHandle {
    cmd_tx: UnboundedSender<RouterCommand>,
}

impl RouterHandle {
    pub fn new(cmd_tx: UnboundedSender<RouterCommand>) -> Self {
        RouterHandle { cmd_tx }
    }
    
    pub async fn stop(&self, graceful: bool) {
        let _ = self.cmd_tx.send(RouterCommand::Stop { graceful });
    }
}
```

### 4. Code Generation Best Practices

#### Always

- ✅ Follow existing naming conventions
- ✅ Use proper error types
- ✅ Include documentation
- ✅ Add tests for generated code
- ✅ Format code with `cargo fmt`

#### Never

- ❌ Hardcode sensitive values
- ❌ Use `unwrap()` in generated code
- ❌ Skip error handling
- ❌ Ignore logging
- ❌ Commit generated files without review

### 5. Testing Generated Code

```rust
#[tokio::test]
async fn test_generated_router_builder() {
    let builder = RouterBuilder::new();
    assert!(builder.cmd_tx.is_some());
    assert!(builder.cmd_rx.is_some());
}
```

### 6. Code Generation Workflow

1. Define data structures
2. Add derive macros
3. Format with `cargo fmt`
4. Lint with `cargo clippy`
5. Test with `cargo test`

### 7. Versioning

- Lock dependency versions in `Cargo.lock`
- Update generators with `cargo update`
- Test before committing changes

### 8. Documentation

- Document all public types
- Add examples for complex patterns
- Include usage examples

## Example: Creating a New Component

```rust
// 1. Define the router struct
pub struct MyRouter {
    handle: RouterHandle,
    fut: BoxFuture<'static, io::Result<()>>,
}

// 2. Implement builder
pub struct MyRouterBuilder {
    pub cmd_tx: UnboundedSender<RouterCommand>,
    pub cmd_rx: UnboundedReceiver<RouterCommand>,
}

// 3. Implement handle
pub struct MyRouterHandle {
    cmd_tx: UnboundedSender<RouterCommand>,
}

// 4. Implement router logic
impl MyRouter {
    pub fn builder() -> MyRouterBuilder {
        MyRouterBuilder::default()
    }
}

// 5. Implement Future trait
impl Future for MyRouter {
    type Output = io::Result<()>;
    
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut Pin::into_inner(self).fut).poll(cx)
    }
}
```

## Common Patterns

### Router Run Pattern

```rust
async fn run(mut builder: RouterBuilder) -> io::Result<()> {
    // Initialize IPIP tunnel
    // Update kernel routes
    // Start HTTP server
    
    loop {
        tokio::select! {
            _ = tick.tick() => {
                // Handle commands
            }
        }
    }
}
```

### Shutdown Pattern

```rust
let mut sigterm = signal(SignalKind::terminate())?;
let mut sigint = signal(SignalKind::interrupt())?;

tokio::select! {
    _ = sigterm.recv() => {
        log::info!("received terminate signal");
        // Shutdown
    }
    _ = sigint.recv() => {
        log::info!("received interrupt signal");
        // Shutdown
    }
}
```

## Maintenance

### Updating Code

1. Check for new versions
2. Update dependencies
3. Regenerate code
4. Run tests
5. Update documentation

### Troubleshooting

**Build Failures:**
- Check Rust version: `rustc --version`
- Update dependencies: `cargo update`
- Clean build: `cargo clean`

**Test Failures:**
- Verify mock implementations
- Check async/await patterns
- Review error handling

## Related Documentation

- [Developer Guide](developer-guide.md) - Development workflows
- [Testing Guide](testing.md) - Testing best practices
- [Architecture](architecture.md) - System design
