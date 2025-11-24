# LoomWM Agent Guidelines

## Build/Test Commands
- `cargo build` - Build the project
- `cargo test` - Run all tests
- `cargo test <test_name>` - Run single test
- `cargo clippy` - Lint with clippy
- `cargo fmt` - Format code
- `rust-analyzer` - LSP server (via `just lsp`)

## Code Style Guidelines
- **Language**: Rust 2024 edition
- **Formatting**: Use `rustfmt` with default settings
- **Linting**: Pass all `clippy` checks
- **Dependencies**: Smithay Wayland compositor library
- **Project Structure**: Wayland compositor with dynamic/manual tiling + AI assistance
- **Error Handling**: Use Rust's Result/Option types, avoid unwrap()
- **Naming**: snake_case for functions/variables, PascalCase for types
- **Imports**: Group std imports, then external crates, then local modules
- **Documentation**: Public APIs need rustdoc comments
