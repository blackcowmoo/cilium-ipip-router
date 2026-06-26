# Code Conventions

## Naming

- **Types**: PascalCase (e.g., `ControllerInner`, `IpCommand`)
- **Functions/Methods**: snake_case (e.g., `get_tunnel_name`, `tunnel_exists`)
- **Constants**: SCREAMING_SNAKE_CASE
- **Modules**: snake_case (e.g., `ipip_tests`, `controller`)

## Code Style

- **Line length**: Follow Rust community conventions (~100 chars)
- **Imports**: Grouped by standard library, external crates, then local modules
- **Async functions**: Use `async fn` syntax with `tokio::test` for async tests
- **Error handling**: Use `?` operator for propagation, return `io::Result` where appropriate

## Module Organization

- Tests are in the same file as the code they test, in a `#[cfg(test)] mod tests` module
- Test modules use `use super::*;` to import the parent module's exports
- IPIP-related functionality is in `src/ipip/` module
- Controller logic is in `src/controller/` module

## Testing Patterns

- Use `IpCommandExecutor` trait for mocking IPIP commands
- Async tests use `#[tokio::test]` attribute
- Channel tests verify state with `is_closed()`, `len()`, and `recv()` operations
- Match assertions use `matches!` macro for enum variants

## Documentation

- Public items should have doc comments
- Function documentation explains behavior, parameters, and return values
- Module-level documentation describes purpose and structure
