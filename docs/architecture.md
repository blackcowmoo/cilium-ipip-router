# Architecture Overview

This document describes the high-level architecture of the Cilium IPIP Router system.

## Components

### Node-local IPIP Router

Each router instance operates independently on its assigned node to manage IPIP tunnel routes using kernel routing.

**Responsibilities:**
- Create and manage IPIP tunnels for local traffic routing
- Update kernel routing tables for Cilium CNI overlay
- Handle graceful shutdown sequences
- Manage node-local network configuration

**Implementation:**
- `src/main.rs`: Main application entry point
- `src/lib.rs`: Library module structure

## HTTP Server

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
Node Startup
      ↓
Initialize Router
      ↓
Create IPIP Tunnel
      ↓
Update Kernel Routes
```

## Concurrency Model

- **Async Runtime**: Tokio multi-threaded runtime
- **Event Handling**: Local event processing for route updates
- **Shutdown**: Coordinated shutdown using broadcast channels
- **Kernel Routing**: Direct manipulation of kernel routing tables
- **Cilium Support**: Updated for Cilium 1.18+ compatibility

## Error Handling

- Route update failures trigger retry logic
- Signal handling ensures graceful shutdown
- All errors logged before propagation
- Compatible with Kubernetes 1.34 and Cilium 1.18+
