# Testing Guide

This guide covers testing strategies, patterns, and best practices for the Cilium IPIP Router project.

## Testing Strategy

### Unit Testing

- Test individual functions and modules
- Mock external dependencies
- Verify business logic in isolation

### Integration Testing

- Test component interactions
- Verify node-local IPIP tunnel operations
- Validate kernel route updates

### Manual Testing

- Test with real Kubernetes cluster
- Verify signal handling
- Confirm health check behavior
- Validate IPIP tunnel creation

## Testing Tools

### Dependencies

```toml
[dev-dependencies]
kube = { version = "0.95.0", features = ["runtime", "derive"] }
kube-core = "0.95.0"
mockall = "0.13.0"
```

### Test Command

```bash
# Run all tests
cargo test --target x86_64-unknown-linux-gnu

# Run with output capture
cargo test --target x86_64-unknown-linux-gnu -- --nocapture

# Run specific test
cargo test --target x86_64-unknown-linux-gnu -- test_name

# Run with coverage
cargo test --target x86_64-unknown-linux-gnu -- --nocapture
```

## Test Patterns

### Mocking Kubernetes Client

```rust
use kube::Api;
use mockall::predicate::*;

#[tokio::test]
async fn test_node_metadata() {
    let mut client = MockClient::new();
    
    let nodes: Api<Node> = Api::all(client.clone());
    
    // Mock the node retrieval
    let node = create_test_node();
    
    // Verify expected behavior
    assert_eq!(node.metadata.name, Some("test-node".to_string()));
}
```

### Testing Router Builder

```rust
#[tokio::test]
async fn test_router_builder() {
    let builder = RouterBuilder::new();
    
    assert!(builder.cmd_tx.is_some());
    assert!(builder.cmd_rx.is_some());
}
```

### Testing Router Handle

```rust
#[tokio::test]
async fn test_router_handle_stop() {
    let (tx, _) = tokio::sync::mpsc::unbounded_channel();
    let handle = RouterHandle::new(tx);
    
    handle.stop(false).await;
    
    // Verify command was sent
}
```

## Test Files

### Location

Tests should be co-located with the code they test:

```
src/controller/
├── root.rs          # Main implementation
└── root_tests.rs    # Unit tests
```

### Test Organization

```rust
#[cfg(test)]
mod root_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_example() {
        // Test implementation
    }
}
```

## Coverage Requirements

### Minimum Coverage

- Business logic: 80%+
- Error handling paths: 100%
- Edge cases: 100%

### Generating Coverage

```bash
# Install cargo-llvm-cov
cargo install cargo-llvm-cov

# Generate coverage report
cargo llvm-cov --target x86_64-unknown-linux-gnu --codecov --output-path coverage.json

# View HTML report
cargo llvm-cov --target x86_64-unknown-linux-gnu --open
```

## CI/CD Testing

### GitHub Workflows

**test.yaml:**
- Runs on pull requests
- Tests all supported targets
- Fails on warnings

**coverage.yml:**
- Generates coverage reports
- Uploads to coverage service
- Validates coverage thresholds

### Test Matrix

```yaml
jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target:
          - x86_64-unknown-linux-gnu
```

## Best Practices

### Writing Tests

1. **Arrange-Act-Assert Pattern**
   ```rust
   // Arrange
   let builder = RouterBuilder::new();
   
   // Act
   let router = Router::new(builder);
   
   // Assert
   assert!(router.handle().is_some());
   ```

2. **Test One Thing Per Test**
   - Each test should verify one behavior
   - Use descriptive test names

3. **Mock External Dependencies**
   - Avoid real Kubernetes API calls
   - Use mock clients for isolation

4. **Test Edge Cases**
   - Empty configurations
   - Single items
   - Error conditions
   - Signal interruptions

### Test Naming

```rust
// Good: Descriptive and specific
#[tokio::test]
async fn test_router_builder_creates_valid_instance() { }

#[tokio::test]
async fn test_router_handle_sends_stop_command() { }

#[tokio::test]
async fn test_tunnel_creation_creates_ipip_interface() { }
```

### Async Testing

```rust
// Use #[tokio::test] for async tests
#[tokio::test]
async fn test_async_function() {
    let result = async_function().await;
    assert!(result.is_ok());
}

// Set timeout for long-running tests
#[tokio::test]
#[timeout(std::time::Duration::from_secs(10))]
async fn test_long_running() { }
```

## Debugging Tests

### Print Debug Output

```rust
#[tokio::test]
async fn test_with_debug() {
    let result = run_test().await;
    eprintln!("Result: {:?}", result);
    assert!(result.is_ok());
}
```

### Test with Verbose Logging

```bash
RUST_LOG=trace cargo test --target x86_64-unknown-linux-gnu
```

### Test Single File

```bash
cargo test --target x86_64-unknown-linux-gnu -- controller::root_tests
```

## Test Data

### Creating Test Nodes

```rust
fn create_test_node() -> Node {
    Node {
        metadata: ObjectMeta {
            name: Some("test-node".to_string()),
            uid: Some("test-uid".to_string()),
            ..Default::default()
        },
        spec: NodeSpec {
            pod_cidr: Some("10.0.0.0/24".to_string()),
            ..Default::default()
        },
        ..Default::default()
    }
}
```

### Test Scenarios

1. **Node Retrieved**: Verify node metadata retrieval
2. **Router Started**: Verify HTTP server startup
3. **Router Stopped**: Verify graceful shutdown
4. **Tunnel Created**: Verify IPIP tunnel creation
5. **Route Updated**: Verify kernel route update
6. **Signal Reception**: Verify graceful shutdown
