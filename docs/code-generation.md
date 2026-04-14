# Code Generation Guidelines

This document outlines the guidelines and best practices for code generation in the Cilium IPIP Router project.

## Overview

Code generation is used for:

- Kubernetes CRD definitions
- API client code
- Serialization/deserialization
- Test fixtures

## Guidelines

### 1. Use Kubernetes Code Generators

For CRD definitions, use `kube-derive`:

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

### 2. Code Generation Commands

```bash
# Generate Kubernetes API clients
cargo run --bin generate

# Update dependencies
cargo update

# Rebuild with generated code
cargo build --target x86_64-unknown-linux-gnu
```

### 3. Generated Code Patterns

#### Builder Pattern

Always use builder pattern for complex objects:

```rust
pub struct ControllerBuilder {
    pub cmd_tx: UnboundedSender<ControllerCommand>,
    pub cmd_rx: UnboundedReceiver<ControllerCommand>,
}

impl Default for ControllerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ControllerBuilder {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = unbounded_channel();
        ControllerBuilder { cmd_tx, cmd_rx }
    }
}
```

#### Handle Pattern

Use handle pattern for lifecycle management:

```rust
pub struct ControllerHandle {
    cmd_tx: UnboundedSender<ControllerCommand>,
}

impl ControllerHandle {
    pub fn new(cmd_tx: UnboundedSender<ControllerCommand>) -> Self {
        ControllerHandle { cmd_tx }
    }
    
    pub async fn stop(&self, graceful: bool) {
        let _ = self.cmd_tx.send(ControllerCommand::Stop { graceful });
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
async fn test_generated_builder() {
    let builder = ControllerBuilder::new();
    assert!(builder.cmd_tx.is_some());
    assert!(builder.cmd_rx.is_some());
}
```

### 6. Code Generation Workflow

1. Define data structures
2. Add derive macros
3. Generate code
4. Format with `cargo fmt`
5. Lint with `cargo clippy`
6. Test with `cargo test`

### 7. Versioning

- Lock dependency versions in `Cargo.lock`
- Update generators with `cargo update`
- Test before committing changes

### 8. Documentation

- Document all public types
- Add examples for complex patterns
- Include usage examples

## Example: Generating a New Controller

```rust
// 1. Define the controller struct
pub struct MyController {
    handle: ControllerHandle,
    fut: BoxFuture<'static, io::Result<()>>,
}

// 2. Implement builder
pub struct MyControllerBuilder {
    pub cmd_tx: UnboundedSender<MyControllerCommand>,
    pub cmd_rx: UnboundedReceiver<MyControllerCommand>,
}

// 3. Implement handle
pub struct MyControllerHandle {
    cmd_tx: UnboundedSender<MyControllerCommand>,
}

// 4. Implement controller logic
impl MyController {
    pub fn builder() -> MyControllerBuilder {
        MyControllerBuilder::default()
    }
}

// 5. Implement Future trait
impl Future for MyController {
    type Output = io::Result<()>;
    
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut Pin::into_inner(self).fut).poll(cx)
    }
}
```

## Common Patterns

### Watch Pattern

```rust
async fn watch(mut builder: ControllerBuilder) -> io::Result<()> {
    let client = Client::try_default().await?;
    let nodes: Api<Node> = Api::all(client);
    
    let mut stream = nodes.watch(&lp, "0").await?.boxed();
    
    loop {
        tokio::select! {
            Ok(Some(event)) = stream.try_next() => {
                // Handle event
            }
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

### Updating Generators

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
