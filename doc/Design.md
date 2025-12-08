## Design

### Language choice

I choose Rust for this project as there is a mature framework for building wayland compositors written in the same language.
The language also has high performace with memory safety which is also ideal for a window manager.

### Overview

A Wayland compositor is a unified display server and window manager, combining the roles traditionally held by separate components in older systems like X11. 
As a display server, it communicates directly with the kernel and hardware to manage the screen and handle all user input. 
Simultaneously, as a window manager, it composites the final image from all application surfaces, controlling their placement, decorations, and behavior on the screen.

The architecture of a wayland compositor is message-based. A message (request) is sent by a client / user input / hardware events, and the server response to the it with another message (event).

### Logic flow

#### New window

When a new client requests to be registered

![](assets/new_window.jpg)

#### Update Layout

When updating the workspace layout is required

![](assets/update_layout.jpg)