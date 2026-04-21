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

## Router API

### Router

Main router implementation that manages IPIP tunnels using kernel routing.

```rust
let router = Router::builder().run().await;
```

**Methods:**
- `builder()` - Get router builder
- `new(builder)` - Create router from builder
- `handle()` - Get router handle for commands

### RouterBuilder

Constructs router instances with default or custom configuration.

```rust
let builder = RouterBuilder::new();
let router = Router::new(builder);
```

**Methods:**
- `new()` - Create new builder instance
- `default()` - Default builder with standard configuration

### RouterHandle

Interface for controlling router lifecycle.

```rust
let handle = router.handle();
handle.stop(false).await;  // Non-graceful stop
handle.stop(true).await;   // Graceful stop
```

**Methods:**
- `stop(graceful: bool)` - Stop router execution

## Error Types

### Application Errors

Uses `anyhow::Error` for application-level errors.

```rust
pub type Result<T> = std::result::Result<T, anyhow::Error>;
```

### Future Errors

Uses `std::io::Error` for Future implementations.

```rust
impl Future for Router {
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
- `start router` - Router initialization
- `server started` - HTTP server ready
- `Applied: {node}` - Node configuration processed
- `Deleted: {node}` - Node configuration removed
- `shutdown router` - Graceful shutdown

**Error Level:**
- Route update failures
- Tunnel creation failures
- Signal handling failures

## Metrics

### Prometheus Metrics

Expose metrics via `prometheus-client` crate.

**Available Metrics:**
- Route update counter
- Tunnel creation duration histogram
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
| `KUBECONFIG` | Kubernetes config path (for node metadata) |
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
4. Stop router (graceful)
5. Exit application

## Return Values

### Success

- `Ok(())` - Operation completed successfully
- `HttpResponse::Ok()` - HTTP 200 response

### Error

- `Err(anyhow::Error)` - Application error
- `io::Error` - I/O or Future error
- `HttpResponse::InternalServerError()` - HTTP 500
