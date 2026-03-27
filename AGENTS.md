# LoomWM Agent Guidelines

LoomWM is a Wayland compositor built with the Smithay library, featuring dynamic/manual tiling and AI assistance capabilities.

## Build/Test Commands

```bash
# Build the project (use CARGO_INCREMENTAL=0 if compiler crashes)
CARGO_INCREMENTAL=0 cargo build

# Build for release
cargo build --release

# Run all tests
cargo test

# Run a specific test by name pattern
cargo test test_resize_tile

# Run tests with output visible
cargo test -- --nocapture

# Run integration tests
cargo test --test integration_tests

# Lint with clippy
cargo clippy

# Lint with clippy and treat warnings as errors
cargo clippy -- -D warnings

# Format code
cargo fmt

# Check formatting without modifying files
cargo fmt -- --check

# Run cargo-deny for license/advisory checking
cargo-deny check

# Check for unused dependencies
cargo-machete

# Edit dependencies
cargo add <crate>
cargo rm <crate>
```

## Code Style Guidelines

### Language & Tooling
- **Edition**: Rust 2024 edition
- **Formatter**: Use `rustfmt` with default settings
- **Linter**: All code must pass `cargo clippy` without warnings
- **MSRV**: Defined in Cargo.toml (check `rust-toolchain.toml` if present)

### Project Structure
```
loomwm/
├── src/
│   ├── backend/        # Backend implementation (display, window management)
│   ├── handlers/       # Wayland protocol handlers
│   ├── input/          # Input handling (keyboard, mouse)
│   ├── monitor/        # Monitor management
│   ├── window/         # Window types and management
│   └── utils/          # Utility modules
├── assistant/          # AI assistant module (workspace member)
├── tests/              # Integration tests
└── example/            # Example configurations
```
- **Type**: Wayland compositor using Smithay library
- **Architecture**: Modular design with backend, handlers, input, monitor, window, and utils modules
- **Key Dependencies**: Smithay (Wayland), slotmap (arena allocation), serde (config), anyhow (errors), burn (ML/tensor operations)

### Import Organization
Group imports in this order with blank lines between groups:
1. Standard library (`std::`)
2. External crates (e.g., `smithay::`, `serde::`)
3. Local modules (`crate::`)

Example:
```rust
use std::sync::Arc;

use smithay::input::keyboard::KeyboardInputEvent;
use serde::{Deserialize, Serialize};

use crate::backend::display::DisplayManager;
use crate::window::WindowState;
```

### Naming Conventions
- **Types/Traits/Enums**: PascalCase (e.g., `WindowManagerState`, `TileTree`)
- **Functions/Variables**: snake_case (e.g., `add_window`, `get_focus`)
- **Constants**: UPPER_SNAKE_CASE for true constants
- **Modules**: snake_case (e.g., `floating_resize_grab.rs`)
- **Generic Parameters**: Single uppercase letters (e.g., `T`, `R`)

### Error Handling
- **Primary**: Use `anyhow::Result` for error propagation
- **Avoid**: `unwrap()` and `expect()` except in tests or truly unreachable cases
- **Pattern**: Use `?` operator for error propagation
- **Option**: Use `if let Some(x) = ...` or `let-else` syntax for option unpacking

### Types & Traits
- Use strong typing; avoid raw primitives where semantic types add clarity
- Derive common traits: `Debug`, `Clone`, `Default`, `PartialEq`, `Eq` where appropriate
- Use `#[serde(...)]` attributes for configuration deserialization
- Implement custom traits (e.g., `TileTreeWindow`) for domain-specific behavior
- Use `bitflags` for flag types (see existing usage)

### Safety & Unsafe Code
- Minimize unsafe code; document why it is necessary
- Common uses: Wayland FFI, Smithay internal APIs

### Testing
- **Framework**: Uses `test-case` crate for parameterized tests
- **Location**: Tests are inline in source files under `#[cfg(test)]` modules
- **Integration Tests**: Located in `tests/integration_tests.rs`
- **Macros**: Custom macros like `tile_tree!` and `assert_tree_eq!` for test construction
- **Running**: Use `cargo test <pattern>` to run specific tests
- **Test Case Policy**: Use `#[test_case()]` macro as much as possible. Only use `#[test]` macro when there is exactly 1 test case and it is unlikely additional test cases will be added.

### Code Generation Policy
- **Comments**: Generated code must have 0 comments unless explicitly specified
- **Documentation**: Add doc comments (`///`) for public APIs and important types

### Working with Smithay
- Smithay APIs often require specific lifetimes and unsafe blocks for FFI
- Follow existing patterns in the codebase for Wayland protocol handling
- Use Smithay's built-in types for coordinates (`Point`, `Rect`, `Size`)

### AI/Assistant Module
- The `assistant/` workspace member handles AI functionality
- Uses `burn` for tensor operations
- Follows the same code style as the main crate