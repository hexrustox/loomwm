# LoomWM

A Wayland compositor with dynamic/manual tiling and an AI-powered layout assistant
that learns how you arrange windows.

## About

LoomWM is a personal project exploring two domains: building a non-trivial Wayland
compositor from the library up, and integrating on-device machine learning into a
desktop environment. The tiling engine handles both manual and dynamic layouts
(master-stack, grid, nested splits). The assistant records your window arrangements
over time and trains a transformer model to suggest optimal layouts on demand.

It's built for my own daily use. I'm sharing it as a reference for anyone curious
about Smithay compositors, Burn-based ML in Rust, or the intersection of the two.

## Features

### Window Management
- **Dynamic/manual tiling** with a hierarchical tile tree. Supports master-stack,
  horizontal/vertical splits, repeating nodes, and custom nested layouts.
- **Floating windows** coexist alongside tiled windows on the same workspace.
- **Multi-workspace** support with switch, move-to, and move-without-focus operations.
- **Keyboard-driven** focus, swap, and resize (by pixel or ratio per edge).
- **Mouse-driven** move and resize via pointer grabs (`alt+left`, `alt+right`).
- **Configurable window rules** that match by app ID and title (regex), window state,
  or workspace. Rules control border width/color, opacity, and decoration mode.
- **Live config reload** via inotify — tweak keybindings, layouts, or window rules
  and they apply immediately without restarting the compositor.
- **XDG decoration** protocol with per-window server-side or client-side decoration.

### AI Layout Assistant
- Records the app IDs of tiled windows per workspace over time.
- Trains a **two-level transformer model** (string-level embeddings for app IDs,
  list-level encoder for ordering) to predict your preferred window arrangement.
- Runs inference to rank and reorder windows on trigger (`alt+a`).
- Supports both **GPU** (Wgpu backend) and **CPU** (NdArray backend) training/inference.
- Automatically saves history and periodically retrains the model on disk.
- Layout data and model stored under `$XDG_STATE_HOME/loomwm/model/`.

## Tech Stack

| Layer             | Technology                              |
|-------------------|-----------------------------------------|
| Language          | Rust (Edition 2024)                     |
| Wayland           | [Smithay](https://github.com/Smithay/smithay) 0.7 |
| Event Loop        | Calloop (via Smithay)                   |
| Rendering         | OpenGL (GlesRenderer)                   |
| Machine Learning  | [Burn](https://burn.dev) 0.20 — Transformer + Adam optimizer |
| GPU Compute       | Wgpu 26                                 |
| Input             | evdev (libinput)                        |
| Config Format     | TOML, hot-reloaded via inotify          |
| Arena Allocation  | slotmap                                 |

## Getting Started

### Prerequisites

- Rust toolchain (stable, Edition 2024)
- A Wayland session
- libudev, libinput, libEGL, libGLESv2

### Build & Run

```bash
git clone <repo-url>
cd loomwm

# Create config directory and copy example config
mkdir -p ~/.config/loomwm
cp example/config.toml ~/.config/loomwm/

# Build and run
cargo run
```

The compositor sets `WAYLAND_DISPLAY` automatically, so Wayland clients launched
after will connect to it.

Enable the AI assistant by setting `assistant.enable = true` in your config.
It will start recording layouts and can be triggered with `alt+a`.

## Configuration

Configuration lives at `$XDG_CONFIG_HOME/loomwm/config.toml`. A full example is at
`example/config.toml`. Key sections:

```toml
[general]
allow-move-request = true
allow-fullscreen-request = true

[layouts]
master = { nodes = [{ ratio = 1.5 }, { layout = "slaves" }] }
slaves = { split = "horizontal", nodes = [{ repeat = 3 }] }
default = "master"

[key.bindings]
"alt+q" = { action = "close_window" }
"alt+return" = { action = "execute", command = ["alacritty"] }
"alt+a" = { action = "assistant" }

[[window-rules]]
matches = [{ is-focused = true }]
border = { width = 2, color = "6a9fb5" }

[assistant]
enable = false
save-layout-after = 10
```

Changes are picked up automatically via inotify. No need to restart.

## Project Structure

```
src/
├── main.rs             Entry point: event loop, display, backend init
├── lib.rs              CompositorData, module declarations
├── state.rs            WindowManagerState — central compositor state
├── backend/            Winit and headless backends
├── config/             TOML config parsing and types
├── handlers/           Wayland protocol handlers (compositor, xdg_shell, decoration)
├── input/              Keyboard bindings, pointer grabs (move, resize, swap)
├── monitor/            Monitor management, workspace/tile tree, assistant integration
├── window/             Window state, rendering, window rules engine
├── watcher.rs          inotify config hot-reload
└── path.rs             XDG path resolution

assistant/
└── src/
    ├── data.rs         Dataset, vocabulary, batching
    ├── model.rs        Two-level transformer ranker
    ├── train.rs        Training loop with Adam + early stopping
    └── infer.rs        Inference / ranking pipeline
```

## Key Design Decisions & Learnings

- **Smithay over wlroots:** Smithay is a pure-Rust Wayland library that lets you
  compose a compositor from components rather than wrapping a C library. This gave
  full control over the architecture but required implementing many protocol handlers
  from scratch.

- **Hierarchical tile tree (slotmap):** The tile tree uses a slotmap-backed arena
  for O(1) node access and manipulation. The tree representation supports arbitrary
  nesting (splits within splits) while keeping resize and swap operations efficient.
  The implementation in `tile.rs` is over 3,000 lines and handles edge cases like
  ratio clamping, cursor repositioning after resize, and maintaining the invariant
  that all regions fill the workspace without gaps.

- **On-device ML with Burn:** Burn is a Rust-native ML framework that abstracts over
  backends (Wgpu, NdArray, etc.). Integrating a transformer model into a compositor
  required careful design: the assistant runs training in a blocking context outside
  the render loop, uses a small vocabulary built from observed app IDs, and applies
  the output ranking as a sequence of tile swaps to minimize disruption.

- **Live config reload:** Watching the config file with inotify and re-parsing TOML
  is straightforward, but the subtle part is diffing the old and new config to
  apply changes without losing runtime state (window positions, workspace layouts,
  etc.).

- **XDG decoration negotiation:** Handling both server-side and client-side
  decoration modes (and switching between them per window) required careful
  tracking of the decoration state machine per surface.

## Status

This is a personal project in early development (v0.1.0). It works for my daily
workflow on a single-monitor setup, but expect rough edges. I'm sharing it for
reference, not as a supported product. No formal support, no promised compatibility,
no contribution guidelines — but feel free to fork and explore.
