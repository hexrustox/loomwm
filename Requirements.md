## Requirements Capture

### Market Research

#### Previous solutions

Existing window managers usually have the following features:

*   **Keyboard-Driven Control:** Users can bind different actions (moving, resizing window) to specific keybind.
*   **Tiling Layouts:** Automatic arrangement of windows in non-overlapping tiles (redefined or customizable) to maximize screen real estate, reducing manual adjustments.
*   **Multi-Workspace**: Virtual desktops or workspaces to organize tasks logically.

Some also supports:

*   **Multi-Monitor**: Handling multiple displays with per-monitor workspaces.
*   **Touchpad gestures**: Swipe your fingers on the touchpad in a certain way to trigger an action.

#### Social media analysis

Users expect a window manager to be generally more performant and use less resources than a desktop environment.

## Project Requirements

### Functional Requirements

| ID     | Requirement                                | Description                                                                                                                                                                                                                             |
| :----- | :----------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **1** | **Floating Window Management**             | The system MUST allow users to manually position and resize windows, freeing them from the tiling layout.                                                                                                                                                |
| **2** | **Tiling Window Management**               | The system MUST automatically arrange windows in a non-overlapping grid layout.                                                                                                                                                                                 |
| **3** | **Customizable Tiling Layouts**            | The system MUST provide users with the ability to define and save custom tiling layouts.                                                                                                                                                                 |
| **4** | **Virtual Workspace Management**           | The system MUST support multiple virtual workspaces to allow users to organize their windows into separate sets of tasks.                                                                                                                                |
| **5** | **Window Rules**                           | The system MUST allow users to define rules based on window properties (e.g., application name, title, class) to automatically set their state (floating/tiled), size, position, or assign them to a specific workspace.                                 |
| **6** | **Customizable Hotkey Bindings & Actions** | The system MUST provide a comprehensive set of actions that can be bound to custom hotkeys.                                                                                                                                                              |
| **6.1** | *Action: Move Focus*                       | The system MUST provide an action to move keyboard focus between windows.                                                                                                                                                                              |
| **6.2** | *Action: Move/Swap Window*                 | The system MUST provide actions to move the currently focused window to a different position in the layout or swap its position with another window.                                                                                                   |
| **6.3** | *Action: Resize Window*                    | The system MUST provide actions to resize the currently focused window within the tiling layout.                                                                                                                                                       |
| **6.4** | *Action: Close Window*                     | The system MUST provide an action to close the currently focused window.                                                                                                                                                                               |
| **6.5** | *Action: Workspace Navigation*             | The system MUST provide actions to switch focus between different virtual workspaces.                                                                                                                                                                  |
| **6.6** | *Action: Move Window to Workspace*         | The system MUST provide an action to move the currently focused window to a different virtual workspace.                                                                                                                                               |
| **6.7** | *Action: Swap Workspaces*                  | The system MUST provide an action to swap the entire contents of the current workspace with another one.                                                                                                                                               |
| **6.8** | *Action: Toggle Window State*              | The system MUST provide actions to toggle a window between floating and tiled states, and between normal and fullscreen modes.                                                                                                                         |
| **6.9** | *Action: Undo & Redo*                      | The system MUST provide actions to undo and redo the last window layout change (e.g., move, swap, resize).                                                                                                                                             |
| **6.10** | *Action: Execute Program*                  | The system MUST provide an action to execute a specified program.                                                                                                                                                                                     |
| **6.11** | *Action: Assistant*                        | The system MUST provide an action to run an AI model to automatically organise window(s) in the tiling/floating layout based on user habit.                                                                                                           |
| **7** | **Multi-Monitor Support (Optional)**       | The system SHOULD support multiple monitors.                                                                                                                                                                                                             |
| **7.1** | *Feature: Hot-plug*                        | The system SHOULD handle monitors being connected or disconnected at runtime without requiring a restart.                                                                                                                                              |
| **7.2** | *Feature: Monitor Actions*                 | The system SHOULD provide actions to move focus between monitors and move workspaces to other monitors.                                                                                                                                                |
| **8** | **Customizable Touchpad Gestures**         | The system MUST allow users to bind custom actions (e.g., switch workspace, move focus) to multi-finger touchpad gestures.                                                                                                                               |
| **9** | **Customizable Window Border (Optional)**  | The system SHOULD allow users to customize the appearance of the window border, including its thickness, color and radius.                                                                                                                               |
| **10** | **Configuration File** | The system MUST have a plain text configuration file that users can modify to alter the behavior of the compositor.
| **10.1** | *Feature: Hot Reload*          | The system MUST allow users to apply changes to the configuration file without restarting the compositor.                                                                                                                                           |

### Non-Functional Requirements

| ID     | Requirement               | Description                                                                                                                                                                                                                             |
| :----- | :------------------------ | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **1** | **Performance**           | All window operations (focus change, move, resize, workspace switch) MUST be executed with minimal perceptible delay to ensure a smooth and responsive user experience.                                                                  |
| **2** | **Reliability**           | The compositor MUST be stable and not crash under normal usage conditions. It should handle unexpected events (e.g., application crashes) gracefully.                                                                                |
| **3** | **Resource Efficiency**   | The compositor MUST have a low memory and CPU footprint to ensure it does not impact the performance of other running applications.                                                                                                  |
| **4** | **Compatibility**         | The system MUST implement the core Wayland protocols and interoperate correctly with common toolkits and applications.                                                                                                                   |

## Links
*   http://adereth.github.io/blog/2013/10/02/why-you-should-try-a-tiling-window-manager/
*   https://www.reddit.com/r/linux4noobs/comments/1dp95lh/why_do_so_many_people_prefer_window_managers_over/