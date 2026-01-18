# AGENTS.md - Claude Code Context

## Project Description

A time tracking application built with:
- **Language:** Rust
- **UI Framework:** GTK 4
- **Database:** SQLite
- **Build System:** Cargo

## Quality Standards

### Code Quality
- All code must compile without warnings (`cargo build`)
- All tests must pass (`cargo test`)
- No clippy warnings (`cargo clippy -- -D warnings`)
- Follow Rust idioms and best practices
- Use proper error handling with `Result` and `?` operator
- Document public APIs with doc comments

### Testing Requirements
- Unit tests for business logic
- Integration tests for database operations
- UI tests where practical

### Commit Standards
- Atomic commits with clear messages
- Run full feedback loop before committing

## Feedback Loop Commands

Run these commands to verify quality before considering a task complete:

```bash
cargo build
cargo test
cargo clippy -- -D warnings
```

All three must pass without errors or warnings.

## Task Prioritization

When multiple tasks are available, prioritize in this order:
1. **Blocking bugs** - Issues preventing basic functionality
2. **Core features** - Essential functionality from PRD
3. **Tests** - Coverage for implemented features
4. **Refactoring** - Code quality improvements
5. **Nice-to-have** - Non-essential enhancements

## Architecture Notes

- Keep GTK UI code separate from business logic
- Use repository pattern for database access
- Prefer composition over inheritance
- Keep functions small and focused

## File Organization

```
src/
├── main.rs          # Application entry point
├── lib.rs           # Library root
├── ui/              # GTK UI components
├── db/              # Database layer
├── models/          # Data structures
└── services/        # Business logic
```
