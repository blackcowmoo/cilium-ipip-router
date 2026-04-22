# Kubernetes Manifests

This directory contains Kubernetes manifests for deploying the Cilium IPIP Router.

## Prerequisites

- Kubernetes cluster (v1.34+)
- Cilium CNI v1.18+

## Deployment

Apply all manifests in order:

```bash
kubectl apply -f namespace.yaml
kubectl apply -f router.yaml
```

Or apply all at once:

```bash
kubectl apply -f .
```

## Security Context

The router requires network privileges (`NET_ADMIN` capability) to:

- Create IPIP tunnel routes using the kernel's `ip` command
- Manage kernel routing tables
- Perform network configuration operations

The container is configured with:
- `hostNetwork: true` for direct network access
- `capabilities.add: NET_ADMIN` for network administration privileges
- `privileged: false` for minimal privilege elevation

## Architecture

The router runs as a DaemonSet, ensuring one instance per node. Each router instance:
- Manages only its node's IPIP routes
- Uses kernel routing for tunnel operations
- Operates independently without central controller
