# API Reference

This document describes the public API endpoints and interfaces.

## HTTP Endpoints

### Health Check

```
GET /health
```

**Response:**
- Status: `200 OK`
- Body: `"healthy"`

**Example:**
```bash
curl http://localhost:9090/health
# Output: "healthy"
```

## Controller API

### ControllerBuilder

Constructs controller instances with default or custom configuration.

```rust
let builder = ControllerBuilder::new();
let controller = Controller::new(builder);
```

**Methods:**
- `new()` - Create new builder instance
- `default()` - Default builder with standard configuration

### ControllerHandle

Interface for controlling controller lifecycle.

```rust
let handle = controller.handle();
handle.stop(false).await;  // Non-graceful stop
handle.stop(true).await;   // Graceful stop
```

**Methods:**
- `stop(graceful: bool)` - Stop controller execution

### Controller

Main controller implementation that watches Kubernetes Nodes.

```rust
let controller = Controller::builder().run().await;
```

**Methods:**
- `builder()` - Get controller builder
- `new(builder)` - Create controller from builder
- `handle()` - Get controller handle for commands

**Lifecycle Events:**
- `Added` - Node created, routes updated
- `Modified` - Node changed, routes updated
- `Deleted` - Node removed, routes cleaned up

## Error Types

### Application Errors

Uses `anyhow::Error` for application-level errors.

```rust
pub type Result<T> = std::result::Result<T, anyhow::Error>;
```

### Future Errors

Uses `std::io::Error` for Future implementations.

```rust
impl Future for Controller {
    type Output = io::Result<()>;
}
```

## Logging

### Log Levels

Configure via `resources/log4rs.yaml`:

| Level | Use Case |
|-------|----------|
| `trace` | Detailed debugging |
| `debug` | Development troubleshooting |
| `info` | Production operational |
| `warn` | Warning conditions |
| `error` | Error conditions |

### Log Messages

**Info Level:**
- `start controller` - Controller initialization
- `server started` - HTTP server ready
- `Applied: {node}` - Node added/modified
- `Deleted: {node}` - Node deleted
- `shutdown controller` - Graceful shutdown

**Error Level:**
- API connection failures
- Watch stream errors
- Signal handling failures

## Metrics

### Prometheus Metrics

Expose metrics via `prometheus-client` crate.

**Available Metrics:**
- Node watch events counter
- Route update duration histogram
- HTTP request latency histogram

**Metrics Endpoint:**
- Path: `/metrics` (if configured)
- Format: Prometheus text exposition

## Configuration API

### Logging Configuration

```yaml
refresh_rate: 30 seconds

appenders:
  stdout:
    kind: console

root:
  level: info
  appenders:
    - stdout
```

### Environment Configuration

| Variable | Purpose |
|----------|---------|
| `RUST_LOG` | Override log level |
| `KUBECONFIG` | Kubernetes config path |
| `LISTEN_ADDR` | HTTP server address |

## Signal Handling

### Supported Signals

| Signal | Action |
|--------|--------|
| `SIGTERM` | Graceful shutdown |
| `SIGINT` | Graceful shutdown |

### Shutdown Sequence

1. Receive signal
2. Log shutdown initiation
3. Stop HTTP server (30s timeout)
4. Stop controller (graceful)
5. Exit application

## Return Values

### Success

- `Ok(())` - Operation completed successfully
- `HttpResponse::Ok()` - HTTP 200 response

### Error

- `Err(anyhow::Error)` - Application error
- `io::Error` - I/O or Future error
- `HttpResponse::InternalServerError()` - HTTP 500
