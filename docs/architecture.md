# Architecture Overview

This document describes the high-level architecture of the Cilium IPIP Router system.

## Components

### Controller

The core component that watches Kubernetes Node resources and manages IPIP routing.

**Responsibilities:**
- Watch Node add/modify/delete events via Kubernetes API
- Process node changes and update IPIP routes
- Handle graceful shutdown sequences

**Implementation:**
- `src/controller/root.rs`: Main controller logic
- `src/controller/builder.rs`: Builder pattern for construction
- `src/controller/handle.rs`: Command interface for lifecycle management

### HTTP Server

Lightweight web server for health checks and observability.

**Endpoints:**
- `GET /health`: Returns "healthy" status

**Implementation:**
- `src/main.rs`: Server configuration and startup

### Logging

Centralized logging configuration.

**Features:**
- Console output
- Configurable log levels
- Hot reload of configuration

**Configuration:**
- File: `resources/log4rs.yaml`
- Default level: INFO

## Data Flow

```
Node Change (K8s API)
         ↓
  Watch Stream
         ↓
  Controller Logic
         ↓
  IPIP Route Update
```

## Concurrency Model

- **Async Runtime**: Tokio multi-threaded runtime
- **Event Handling**: Watch stream with tick-based command polling
- **Shutdown**: Coordinated shutdown using broadcast channels

## Error Handling

- Watch stream failures trigger retry logic
- Signal handling ensures graceful shutdown
- All errors logged before propagation
