This project aims to develop a Wayland compositor that integrates dynamic and manual tiling.
Its core innovation will be an AI-assisted organization feature that automatically arranges windows based on user context and habits, while still allowing for full manual override. 
Complemented by hot-reloading configurations, customizable keybinds and touchpad gestures, and productivity features like floating windows, virtual workspace etc.

## Requirements Capture

### Market Research

An analysis of existing solutions and community feedback was performed.

*   **Previous Solutions:**
    *   **Sway:** Highly respected for their simplicity, stability, and powerful tiling logic. However, their configuration can have a steep learning curve, and they lack native support some modern aesthetics.
    *   **Hyprland:** A modern Wayland compositor known for its animations, dynamic tiling, and extensive customization options. It is very popular but its rapid development cycle can sometimes lead to instability.
*   **Social Media Analysis:**
    *   There is a [discussion](https://bbs.archlinux.org/viewtopic.php?id=92687) on users' preference on dynamiuc vs manual tiling.
    *   Users often praise window managers that allow for **"hot-reloading"** of configuration files, as it dramatically speeds up the tweaking process.
    *   There is a clear demand for intuitive yet powerful **touchpad gesture** support on laptops.

### User Evaluation

According to this [youtube video](https://www.youtube.com/watch?v=aeifzxaDOVo), a tiling window manager has the following advantages.
 
*   **Maximise Screen Usage:** Window(s) are automatically resized and moved to fit the size of the monitor to use the entireity of the screen.
*   **Keyboard Centric Control:** Every action can be done with a keyboard shortcut(s) which is more efficient than using the mouse.

## Project Requirements

### Functional Requirements

| ID     | Requirement                                | Description & Justification                                                                                                                                                                                                                             |
| :----- | :----------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **1** | **Floating Window Management**             | The system MUST allow users to manually position and resize windows, freeing them from the tiling layout.                                      |
| **2** | **Tiling Window Management**               | The system MUST automatically arrange windows in a non-overlapping grid.                                                                                     |
| **3** | **Customizable Tiling Layouts**            | The system MUST provide users with the ability to define and save custom tiling layouts.                                                                        |
| **4** | **Virtual Workspace Management**           | The system MUST support multiple virtual workspaces to allow users to organize their windows into separate sets of tasks.                                                                                              |
| **5** | **Window Rules**                           | The system MUST allow users to define rules based on window properties (e.g., application name, title, class) to automatically set their state (floating/tiled), size, position, or assign them to a specific workspace. |
| **6** | **Customizable Hotkey Bindings & Actions** | The system MUST provide a comprehensive set of actions that can be bound to custom hotkeys.                                                                                                                       |
| **6.1** | *Action: Move Focus*                       | The system MUST provide an action to move keyboard focus between windows.                                                                                                                                             |
| **6.2** | *Action: Move/Swap Window*                 | The system MUST provide actions to move the currently focused window to a different position in the layout or swap its position with another window.                                                                    |
| **6.3** | *Action: Resize Window*                    | The system MUST provide actions to resize the currently focused window within the tiling layout.                                                                                                                      |
| **6.4** | *Action: Close Window*                     | The system MUST provide an action to close the currently focused window.                                                                                                                                                                                 |
| **6.5** | *Action: Workspace Navigation*             | The system MUST provide actions to switch focus between different virtual workspaces.                                                                                                                                                                    |
| **6.6** | *Action: Move Window to Workspace*         | The system MUST provide an action to move the currently focused window to a different virtual workspace.                                                                                                              |
| **6.7** | *Action: Swap Workspaces*                  | The system MUST provide an action to swap the entire contents of the current workspace with another one.                                                                                                                                                 |
| **6.8** | *Action: Toggle Window State*              | The system MUST provide actions to toggle a window between floating and tiled states, and between normal and fullscreen modes.                                                                                                                            |
| **6.9** | *Action: Undo & Redo*                      | The system MUST provide actions to undo and redo the last window layout change (e.g., move, swap, resize).                                                                                                            |
| **6.10** | *Action: Execute Program*                  | The system MUST provide an action to execute a specified program.                                                                                                                                                                                         |
| **6.11** | *Action: Assistant*                        | The system MUST provide an action to run an AI model to automatically organise window(s) in the tiling/floating layout based on user habit.                                                                                                                         |
| **7** | **Multi-Monitor Support (Optional)**       | The system SHOULD support multiple monitors.                                                                                                                                                                                                             |
| **7.1** | *Feature: Hot-plug*                        | The system SHOULD handle monitors being connected or disconnected at runtime without requiring a restart.                                                                                                                                               |
| **7.2** | *Feature: Monitor Actions*                 | The system SHOULD provide actions to move focus between monitors and move workspaces to other monitors.                                                                                                                                                  |
| **8** | **Customizable Touchpad Gestures**         | The system MUST allow users to bind custom actions (e.g., switch workspace, move focus) to multi-finger touchpad gestures.                                                                                           |
| **9** | **Customizable Window Border (Optional)**  | The system SHOULD allow users to customize the appearance of the window border, including its thickness, color and radius.                                              |
| **10** | **Hot Reload Configuration File**          | The system MUST allow users to apply changes to the configuration file without restarting the window manager.                                                                                                        |

### Non-Functional Requirements

| ID     | Requirement               | Description                                                                                                                                                                                                                             |
| :----- | :------------------------ | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **1** | **Performance**           | All window operations (focus change, move, resize, workspace switch) MUST be executed with minimal perceptible delay to ensure a smooth and responsive user experience.                                                    |
| **2** | **Configurability**       | All aspects of the window manager's behavior and appearance MUST be configurable via a human-readable text file. The configuration syntax should be clear and well-documented.                                           |
| **3** | **Reliability**           | The window manager MUST be stable and not crash under normal usage conditions. It should handle unexpected events (e.g., application crashes) gracefully.                                                                                 |
| **4** | **Resource Efficiency**   | The window manager MUST have a low memory and CPU footprint to ensure it does not impact the performance of other running applications.                                                                                                   |
| **5** | **Compatibility**         | The system MUST implement the core Wayland protocols and interoperate correctly with common toolkits and applications.                                                                        |