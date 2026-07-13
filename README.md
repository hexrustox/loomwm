# LoomWM

A tiling Wayland compositor written in Rust, with a built-in AI assistant that learns your window-layout habits.

## About

LoomWM replaces your desktop environment: it speaks the modern Wayland protocol, arranges windows in configurable tile layouts, and is driven by a small TOML config with vim-style keybindings. As you work, a small transformer model — trained locally on-device using the Burn ML framework — records which apps you tend to have open together and ranks your current windows into the layout you are most likely to want next. Nothing leaves the machine; the model just adapts to your patterns over time.

## Features

- Configurable tiling layouts (master/stack by default)
- Vim-style keybindings for focus, swap, and resize
- Window rules (borders, decorations, opacity)
- Multiple workspaces with move/focus/send actions
- Floating and maximized window modes
- Pointer move and resize (alt+drag)
- AI layout prediction via a local transformer model
- Hot-reload configuration via file watcher

## Requirements

- Rust toolchain (edition 2024)
- System libraries: `libxkbcommon`, `wayland`, `libglvnd`, `mesa`

## Building & Running

```bash
cargo build
cargo run
```

## Configuration

Config is read from `~/.config/loomwm/config.toml`.

See [example/config.toml](example/config.toml) for a complete reference.

## AI Assistant

The assistant is a small transformer model that learns your layout preferences over time. It is trained locally using the Burn ML framework and runs inference on-device — no data is sent anywhere.

Enable it in your config:

```toml
[assistant]
enable = true
save-layout-after = 10
```

## Acknowledgments

- [Smithay](https://github.com/Smithay/smithay) — Wayland compositor building blocks
- [Burn](https://github.com/tracel-ai/burn) — ML framework for training and inference
