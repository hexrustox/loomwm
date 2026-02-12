# LoomWM Agent Guidelines

LoomWM is a Wayland compositor built with the Smithay library, featuring dynamic/manual tiling and AI assistance capabilities.

## Build/Test Commands

```bash
# Build the project
cargo build

# Build for release
cargo build --release

# Run all tests
cargo test

# Run a specific test by name pattern
cargo test test_resize_tile

# Run tests with output visible
cargo test -- --nocapture

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
- **MSRV**: Defined in Cargo.toml

### Project Structure
- **Type**: Wayland compositor using Smithay library
- **Architecture**: Modular design with backend, handlers, input, monitor, window, and utils modules
- **Key Dependencies**: Smithay (Wayland), slotmap (arena allocation), serde (config), anyhow (errors)

### Import Organization
Group imports in this order with blank lines between groups:
1. Standard library (`std::`)
2. External crates (e.g., `smithay::`, `serde::`)
3. Local modules (`crate::`)

Example:
```rust
use std::{collections::HashMap, rc::Rc};

use smithay::{
    backend::renderer::ImportAll,
    desktop::Space,
};
use serde::Deserialize;

use crate::{
    input::KeyModifiers,
    monitor::Monitors,
};
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

Example:
```rust
fn read_config(path: &str) -> Result<Config, anyhow::Error> {
    let content = std::fs::read_to_string(path)?;
    let config = toml::from_str(&content)?;
    Ok(config)
}
```

### Types & Traits
- Use strong typing; avoid raw primitives where semantic types add clarity
- Derive common traits: `Debug`, `Clone`, `Default`, `PartialEq`, `Eq` where appropriate
- Use `#[serde(...)]` attributes for configuration deserialization
- Implement custom traits (e.g., `TileTreeWindow`) for domain-specific behavior

### Safety & Unsafe Code
- Minimize unsafe code; document why it is necessary
- Common uses: Wayland FFI, Smithay internal APIs
- Example pattern:
```rust
unsafe {
    display
        .get_mut()
        .dispatch_clients(&mut state.compositor)
        .unwrap();
}
```

### Testing
- **Framework**: Uses `test-case` crate for parameterized tests
- **Location**: Tests are inline in source files under `#[cfg(test)]` modules
- **Macros**: Custom macros like `tile_tree!` and `assert_tree_eq!` for test construction
- **Running**: Use `cargo test <pattern>` to run specific tests

Example test pattern:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(
        tile_tree!(layout() [ window(id: 0), window(id: 1) ]),
        1,
        ResizeEdge::LEFT,
        10,
        tile_tree!(layout(...) [ ... ]);
        "test description"
    )]
    fn test_resize_tile(...) { ... }
}
```

### Documentation
- Public APIs require rustdoc comments (`///`)
- Document panics, errors, and safety invariants
- Use `// TODO` for planned improvements

### Configuration
- Config files use TOML format
- Located at `/data/example/config.toml` in development
- Supports hot-reloading via file watching

### Common Patterns

**State Management**:
- Central state in `WindowManagerState` struct
- Use interior mutability (e.g., `Mutex`, `RefCell`) for shared state
- Implement methods on state for operations

**Window Handling**:
- Distinguish between unmapped and mapped windows
- Use `MappedWindow` with interior mutability for tracked windows
- Handle window rules for dynamic behavior

**Input Handling**:
- Process events in `process_input_event`
- Use grab system for interactive operations (move, resize, swap)

### Nix Development Environment
- Uses Nix flake for reproducible development environment
- Includes rust-analyzer, clippy, rustfmt, cargo-deny, cargo-machete
- Run `nix develop` to enter the dev shell

### Performance Considerations
- Use arena allocation (slotmap) for tile trees
- Minimize allocations in hot paths
- Use iterators and lazy evaluation where appropriate
