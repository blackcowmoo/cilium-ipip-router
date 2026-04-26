# Cilium IPIP Router Documentation

Welcome to the Cilium IPIP Router documentation. This project implements a Kubernetes controller for managing IPIP routing for Cilium CNI.

**Version**: Kubernetes 1.34 / Cilium 1.18+

## Documentation Structure

### Core Documentation

- **[Architecture Overview](architecture.md)** - High-level system architecture and component interactions
- **[API Reference](api-reference.md)** - Public API endpoints and interfaces

## Quick Start

### For Developers

1. Set up your development environment
2. Run tests: `cargo test --target x86_64-unknown-linux-gnu`
3. Submit pull requests following the guidelines

### For Operators

1. Set up your deployment environment
2. Configure RBAC and permissions
3. Deploy and monitor

## Project Structure

```
src/
├── main.rs              # Entry point
├── lib.rs               # Module declarations
└── controller/          # Business logic
    ├── mod.rs
    ├── root.rs
    ├── builder.rs
    └── handle.rs

resources/
└── log4rs.yaml          # Logging configuration

docs/                    # This documentation
```

## Key Concepts

- **Controller Pattern**: Watches Kubernetes Nodes for changes
- **IPIP Routing**: Manages IP-in-IP tunnels for Cilium CNI
- **Async Runtime**: Tokio-based async processing
- **Graceful Shutdown**: Signal handling for clean termination
- **Kubernetes Version**: Compatible with Kubernetes 1.34
- **Cilium Version**: Compatible with Cilium 1.18+

## Contributing

1. Follow the coding standards
2. Add tests for new functionality
3. Update documentation as needed

## Support

- Check existing documentation
- Review test examples
- Examine sample configurations

## License

MIT
