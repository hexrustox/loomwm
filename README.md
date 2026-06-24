# LoomWM

A tiling Wayland compositor written in Rust, with a built-in AI assistant
that learns your window-layout habits and suggests the next layout for you.

A window manager is the program that draws your apps on screen and decides
where they go. LoomWM replaces the desktop environment: it speaks the modern
Wayland protocol, arranges windows in configurable tile layouts (a
master/stack layout by default), and is driven by a small TOML config with
vim-style keybindings. The unusual part is the assistant. As you work,
LoomWM records which apps you tend to have open together and the order
they appear in, and a small transformer model — trained locally on-device
using the Burn ML framework — ranks your current windows into the layout
you are most likely to want next. Nothing leaves the machine; the model
just adapts to your patterns over time.

The project is a Rust workspace with two crates: the compositor itself
and the `assistant` ML module, sharing the same toolchain and tests.
