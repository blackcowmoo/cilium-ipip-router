# Cilium IPIP Router

A node-local router that manages IPIP routing for Cilium CNI using kernel routing on each node.

## Overview

This Rust-based router creates and manages IPIP tunnels for Cilium network overlay on each node. It uses direct kernel routing to manage traffic without a central controller. Each router instance operates independently on its assigned node.

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
