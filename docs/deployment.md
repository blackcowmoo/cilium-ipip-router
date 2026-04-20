# Deployment Guide

This guide provides a high-level overview of deploying the Cilium IPIP Router.

## Overview

The router is deployed as a Kubernetes DaemonSet to ensure every node has local IPIP tunnel termination capability. This architecture aligns with Cilium's node-level operation.

## Key Concepts

- **DaemonSet Deployment**: One router pod per node for localized tunnel operations
- **Host Networking**: Uses `hostNetwork: true` for direct network access
- **Privileged Mode**: Requires elevated permissions for IPIP tunnel management
- **Kubernetes Integration**: Watches node changes via the API server

## Prerequisites

- Kubernetes cluster (v1.34+)
- Cilium CNI v1.18+
- Cluster admin access

## Deployment Considerations

### Why DaemonSet?

The DaemonSet pattern ensures:
- Router presence on every node for local IPIP handling
- Alignment with Cilium's node-based architecture
- Simplified scaling through node addition/removal

### Resource Requirements

- Memory: 64Mi-128Mi baseline
- CPU: 100m-200m baseline
- Adjustable based on cluster size

### Security

- Service account with node read permissions
- Privileged container for IPIP operations
- Read-only Kubernetes config mount

## Deployment Steps

1. Apply RBAC configurations
2. Create configMap for logging
3. Deploy DaemonSet
4. Verify pod status across nodes

## Monitoring

- Health endpoint at port 9090
- Prometheus metrics exposure
- Standard Kubernetes logging

## Maintenance

- Rolling updates supported
- No persistent state to backup
- State rebuilt from Kubernetes API on restart
