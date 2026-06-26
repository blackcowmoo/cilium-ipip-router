# Technology Stack

## Language & Build

- **Language**: Rust (edition 2021)
- **Build Tool**: Cargo
- **Minimum Version**: 1.95.0 (based on Dockerfile)

## Core Dependencies

- **Kubernetes Client**: `kube` 0.96.0 with `runtime` and `derive` features
- **Kubernetes Types**: `k8s-openapi` 0.23.0 with `v1_28` feature
- **Async Runtime**: `tokio` 1.40.0 with `macros` and `rt-multi-thread` features
- **Futures**: `futures` 0.3.31, `futures-core` 0.3.31
- **Logging**: `log` 0.4.22, `log4rs` 1.3.0
- **HTTP Server**: `actix-web` 4.9.0
- **Metrics**: `prometheus-client` 0.22.3
- **Hashing**: `md5` 0.7
- **Error Handling**: `anyhow` 1.0.89

## Dev Dependencies

- `kube` 0.96.0 (same as prod)
- `kube-core` 0.96.0
- `mockall` 0.13.0

## Build Environment

- Base image: Rust 1.95.0
- Container runtime: Docker
- Test framework: Built-in Rust test harness with tokio support
