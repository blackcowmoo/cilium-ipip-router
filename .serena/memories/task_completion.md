# Task Completion Checklist

## Code Changes

After implementing any feature or fix:

1. **Run linter** (if available):
   ```bash
   cargo clippy
   ```

2. **Format code**:
   ```bash
   cargo fmt
   ```

3. **Run tests**:
   ```bash
   cargo test
   ```

4. **Check for compilation errors**:
   ```bash
   cargo check
   ```

## Testing Requirements

- All new functionality must have unit tests
- Async functions need `#[tokio::test]` annotated tests
- Use `IpCommandExecutor` trait for mocking external commands
- Verify channel state changes in controller tests

## Documentation Requirements

- Update `mem:core` if module structure changes
- Update `mem:conventions` if code style changes
- Add doc comments to public APIs

## Git Workflow

- Commit changes with descriptive messages
- Run `git status` to verify changes
- Run `git diff` to review changes before committing
