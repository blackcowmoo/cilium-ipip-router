# Testing Guidelines

## Commands
- Run tests: `cargo test --target x86_64-unknown-linux-gnu`
- Format code: `cargo fmt --all -- --check`
- Lint code: `cargo clippy --target x86_64-unknown-linux-gnu -- -D warnings`

## Testing Standards
- Use `#[tokio::test]` for async tests
- Use mockall for mocking external dependencies
- Tests should be in the same file as the code they test (root_tests.rs)
- Mock Kubernetes API clients to enable unit testing without cluster access

## Test Patterns
- Mock external services using mockall
- Test controller builder functionality
- Test controller lifecycle (start, stop)
- Test with mock clients to avoid actual Kubernetes API calls
