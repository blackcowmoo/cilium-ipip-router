# Architecture Overview

This document describes the high-level architecture of the Cilium IPIP Router system.

## Components

### Node-local IPIP Router

Each router instance operates independently on its assigned node to manage IPIP tunnel routes using kernel routing.

**Responsibilities:**
- Watch Node add, update, and delete events, including delayed PodCIDR assignment
- Create direct routes between nodes in the same group and InternalIP subnet
- Create and manage IPIP tunnels between different node groups
- Reconcile and remove kernel routes for Cilium CNI networking
- Handle graceful shutdown sequences
- Manage node-local network configuration

**Implementation:**
- `src/bin/router.rs`: Main application entry point
- `src/lib.rs`: Library module structure

## HTTP Server

Lightweight web server for health checks and observability.

**Endpoints:**
- `GET /health`: Returns "healthy" status

**Implementation:**
- `src/bin/router.rs`: Server configuration and startup

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
Watch Kubernetes Nodes
      ↓
Wait for Node IP and PodCIDR
      ↓
Compare the configured node-group label
      ↓
Same group and on-link InternalIP: direct route / Otherwise: IPIP route
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
