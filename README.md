<div align="center">

# LoomWM

*A Linux window manager that learns how you like your windows arranged*

[![Rust](https://img.shields.io/badge/Rust-2024%20edition-DEA584?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Smithay](https://img.shields.io/badge/built%20with-Smithay%200.7-blue?style=flat-square)](https://smithay.github.io)
[![Tests](https://img.shields.io/badge/tested%20with-cargo%20test-green?style=flat-square&logo=rust)](AGENTS.md)

</div>

LoomWM is a desktop environment for Linux that keeps your windows perfectly organized — no overlapping, no dragging, no resizing by hand. Open as many apps as you like: LoomWM arranges each one into a tidy slot on screen, so you always see everything at a glance.

And it has a trick up its sleeve: LoomWM quietly learns which arrangement you prefer and can tidy your windows for you with a single keystroke.

<p align="center">
  <img src="assets/screenshot.png" alt="LoomWM tiling windows into a neat grid" width="100%">
</p>

One keystroke hands your windows to the AI assistant. Before: a scattered mess. After: everything snapped into the arrangement it has learned from you.

**Before:**

<p align="center">
  <img src="assets/before.png" alt="Windows scattered before the AI assistant" width="100%">
</p>

**After:**

<p align="center">
  <img src="assets/after.png" alt="Windows arranged by the AI assistant" width="100%">
</p>

## Features

- 🪟 **Automatic tiling** — every window gets its own spot on screen. No overlap, no fiddling.
- 🖥️ **Virtual desktops** — spread your work across multiple workspaces and switch instantly.
- 🤖 **AI layout assistant** — LoomWM watches which windows you use together and re-arranges them into your preferred order on demand. It runs on your machine, using the graphics card when available.
- ⌨️ **Fully customizable shortcuts** — bind every action to the keys you like, including mouse actions.
- 🎨 **Window rules** — decide per app whether it floats above the others, how thick its border is, or which workspace it opens on.
- ⚡ **Live settings** — change your configuration and it applies immediately, no restart needed.

## How it works

```mermaid
flowchart LR
    A[You open your apps] --> B[LoomWM arranges them<br>into a neat grid]
    B --> C[LoomWM records how you<br>like them ordered]
    C --> D[Press one shortcut]
    D --> B
    D --> E[Windows snap into your<br>learned arrangement]
```

The AI assistant keeps a running history of your layouts and retrains itself in the background, so it keeps up with how your habits change. Prefer to stay in control? Every arrangement can also be made by hand — move, swap, and resize windows with the keyboard or mouse, with clear visual feedback for each action.
