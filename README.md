# Cilium IPIP Router

A node-local router that manages IPIP routing for Cilium CNI using kernel routing on each node.

## Overview

This Rust-based router watches Kubernetes Nodes and manages kernel routes for each remote PodCIDR. Nodes in the same configured L2 group use direct routes, while traffic to nodes in other groups is encapsulated with IPIP. Each router instance operates independently on its assigned node.

## Node group configuration

Set `NODE_GROUP_LABEL` to the Kubernetes Node label key that identifies a node
group. Nodes with the same label value are in the same group. If both nodes do
not have that label, they belong to the same default group; if only one node has
the label, they are in different groups.

Nodes in the same group use a direct route only when the remote InternalIP is
inside the local InternalIP's interface subnet. The router reads the local
address and prefix using `ip -o -4 addr show` and compares the calculated
network addresses. A missing prefix or failed address lookup forces IPIP even
when the group matches. Different groups always use IPIP. When
`NODE_GROUP_LABEL` is unset, every remote node uses IPIP for backward
compatibility.

For example:

```bash
kubectl label node node-a node-b router.example.com/l2-group=rack-a
kubectl label node node-c router.example.com/l2-group=rack-b
```

Configure each router container with:

```yaml
env:
  - name: NODE_NAME
    valueFrom:
      fieldRef:
        fieldPath: spec.nodeName
  - name: NODE_GROUP_LABEL
    value: router.example.com/l2-group
```

If `node-b`'s InternalIP is on the same subnet as `node-a`, the route from
`node-a` to `node-b`'s PodCIDR is installed through that InternalIP. The route to
`node-c`'s PodCIDR uses a node-specific IPIP tunnel. Node add and PodCIDR update
events reconcile these routes; node deletion removes the route and any
associated tunnel.

## Project Structure

```
/git/work/
├── src/
│   ├── lib.rs               # Library module declarations
│   ├── controller/
│   │   ├── mod.rs           # Controller module declarations
│   │   ├── builder.rs       # Controller builder implementation
│   │   ├── handle.rs        # Controller handle implementation
│   │   ├── root.rs          # Controller main implementation
│   │   └── ipip_tests.rs    # IPIP-related unit tests
│   ├── ipip/
│   │   ├── mod.rs           # IPIP module declarations
│   │   └── executor.rs      # IPIP command executor implementation
│   └── bin/
│       └── router.rs        # Application entry point
├── resources/
│   └── log4rs.yaml          # Logging configuration
├── Dockerfile               # Multi-stage Docker build
├── Cargo.toml               # Rust dependencies
├── Cargo.lock               # Dependency lock file
└── .github/
    └── workflows/           # CI/CD pipelines
        ├── deploy.yaml      # Docker image deployment
        ├── test.yaml        # Test execution
        ├── coverage.yml     # Coverage reporting
        └── docker.yaml      # Docker build validation
```
