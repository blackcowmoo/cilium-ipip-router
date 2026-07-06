# Suggested Commands

## Development Commands

```bash
# Build the project
cargo build

# Build in release mode
cargo build --release

# Run tests
cargo test

# Run specific test
cargo test <test_name>

# Run tests with verbose output
cargo test -- --nocapture

# Check code without building
cargo check

# Format code
cargo fmt
```

## Binary Execution

```bash
# Run the router binary
cargo run --bin router

# Run in release mode
cargo run --bin router --release
```

## Quality Checks

```bash
# Linting (requires clippy)
cargo clippy

# Generate documentation
cargo doc --open
```

## Git Commands (Linux-specific)

```bash
# View recent commits
git log --oneline -10

# Check status
git status

# Show diff
git diff

# View branch
git branch --show-current
```

## Docker

```bash
# Build Docker image
docker build -t cilium-ipip-router .

# Run container
docker run --rm cilium-ipip-router
```
