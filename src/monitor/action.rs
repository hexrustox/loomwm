use smithay::{
    desktop::Window, reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::SERIAL_COUNTER,
};

use crate::{
    monitor::{FoundMappedWindow, workspace::TileResizeUnit},
    state::WindowManagerState,
    utils::Direction,
    window::{
        MappedWindow, WindowRole,
        rule::{WindowLocation, WindowOpeningProperties, WindowProperties, WindowState},
    },
};

pub use super::workspace::{SpecialWindowAction, TileTreeWindow, WorkspaceName};

impl WindowManagerState {
    pub fn register_new_window(&mut self, window: Window, properties: WindowProperties) {
        let Some(WindowOpeningProperties {
            open_with_focus,
            open_as,
            layout_state,
            open_in_workspace,
        }) = properties.opening
        else {
            unreachable!();
        };
        let layout_state = layout_state.unwrap();

        let floating = match layout_state {
            WindowState::Float { .. } => true,
            WindowState::Tile { .. } => false,
        };
        let role = open_as.unwrap_or(WindowRole::Normal);
        let mut mapped = MappedWindow::new(window, open_with_focus.unwrap(), floating, role);

        let monitor = self.monitors.get_monitor_mut();
        let workspace_name = open_in_workspace
            .inspect(|name| {
                monitor.add_workspace(name.clone(), self.layout_set.clone(), &self.default_layout);
            })
            .unwrap_or(monitor.get_active_workspace_name().clone());
        if mapped.is_focused() {
            monitor.active_workspace = workspace_name.clone();
        }
        let workspace = monitor.get_workspace_mut(&workspace_name);

        match layout_state {
            WindowState::Float { location, size } => {
                let window_size = size
                    .map(|(w, h)| (w, h).into())
                    .unwrap_or(mapped.get_geometry_size());
                match location {
                    Some(WindowLocation::Location(x, y)) => {
                        mapped.set_location((x, y).into());
                    }
                    Some(WindowLocation::Center) => {
                        let output_size = &workspace.get_output_size();
                        mapped.set_location(
                            (
                                output_size.w / 2 - window_size.w / 2,
                                output_size.h / 2 - window_size.h / 2,
                            )
                                .into(),
                        );
                    }
                    _ => {}
                }
                mapped.set_size(mapped.clamp_size(window_size));

                workspace.add_floating(mapped.clone());
            }
            WindowState::Tile { ratio } => {
                let mapped = workspace.add_tiling(mapped.clone(), ratio);
                self.fallback_if_tiling_fails(mapped.clone());
                if mapped.is_some() {
                    return;
                }
            }
        }

        if mapped.is_focused() {
            self.focus_to_window(&mapped.wl_surface());
        }
        if mapped.is_maximized() {
            self.toggle_window_role(&mapped.wl_surface(), WindowRole::Maximized, Some(true));
        }
        if mapped.is_fullscreen() {
            self.toggle_window_role(&mapped.wl_surface(), WindowRole::Fullscreen, Some(true));
        }

        self.save_layout_history();
    }

    pub fn send_close_to_window(&mut self) {
        let keyboard = self.get_keyboard();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some(FoundMappedWindow { mapped, .. }) = self.find_mapped_window_by_surface(&surface)
        else {
            return;
        };

        mapped.toplevel().send_close();

        self.save_layout_history();
    }

    pub fn focus_to_window(&mut self, surface: &WlSurface) {
        let keyboard = self.get_keyboard();

        if let Some(old_surface) = keyboard.current_focus() {
            if old_surface == *surface {
                return;
            }
            if let Some(FoundMappedWindow {
                mapped,
                workspace_name: _,
                ..
            }) = self.find_mapped_window_by_surface(&old_surface)
            {
                mapped.set_focus(false);
                mapped.window().set_activated(false);
            }
        }
        keyboard.set_focus(self, Some(surface.clone()), SERIAL_COUNTER.next_serial());
        let Some(FoundMappedWindow {
            mapped,
            workspace_name,
            ..
        }) = self.find_mapped_window_by_surface(surface)
        else {
            return;
        };
        mapped.set_focus(true);
        mapped.window().set_activated(true);

        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&workspace_name);
        workspace.push_focus_queue_back(mapped.clone());
        if mapped.is_floating() {
            workspace.bring_floating_to_front(surface);
        }
    }

    pub fn focus_last_window_in_workspace(&mut self, workspace_name: &WorkspaceName) {
        let monitor = self.monitors.get_monitor_mut();
        if let Some(mapped) = monitor
            .get_workspace(workspace_name)
            .get_last_focused_window()
            .cloned()
        {
            self.focus_to_window(&mapped.wl_surface());
        } else {
            self.get_keyboard()
                .set_focus(self, None, SERIAL_COUNTER.next_serial());
        }
    }

    pub fn switch_workspace(&mut self, workspace_name: WorkspaceName) {
        let monitor = self.monitors.get_monitor_mut();

        let old_workspace_name = monitor.get_active_workspace_name().clone();
        let old_workspace = monitor.get_workspace(&old_workspace_name);
        if old_workspace.window_count() == 0 {
            monitor.remove_workspace(&old_workspace_name);
        }

        monitor.add_workspace(
            workspace_name.clone(),
            self.layout_set.clone(),
            &self.default_layout,
        );
        monitor.active_workspace = workspace_name.clone();

        self.focus_last_window_in_workspace(&workspace_name);
    }

    fn next_workspace_by_delta(&mut self, delta: i32) {
        let monitor = self.monitors.get_monitor_mut();

        let old_workspace_name = monitor.get_active_workspace_name();
        let Some(index) = monitor.find_workspace_index(old_workspace_name) else {
            return;
        };

        let len = monitor.workspaces.len() as i32;
        let new_index = (index as i32 + delta).rem_euclid(len) as usize;

        let new_workspace_name = monitor.workspaces[new_index].get_name().clone();
        self.switch_workspace(new_workspace_name);
    }

    pub fn next_workspace(&mut self) {
        self.next_workspace_by_delta(1);
    }

    pub fn prev_workspace(&mut self) {
        self.next_workspace_by_delta(-1);
    }

    pub fn move_window_to_workspace(&mut self, workspace_name: WorkspaceName, focus: bool) {
        let keyboard = self.get_keyboard();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let occupant_role = self.find_mapped_window_by_surface(&surface).and_then(|f| {
            if f.mapped.is_maximized() {
                Some(WindowRole::Maximized)
            } else if f.mapped.is_fullscreen() {
                Some(WindowRole::Fullscreen)
            } else {
                None
            }
        });
        let Some(FoundMappedWindow {
            mapped,
            workspace_name: old_workspace_name,
            ..
        }) = self.remove_mapped_window(&surface)
        else {
            return;
        };

        if !focus {
            self.focus_last_window_in_workspace(&old_workspace_name);
        }

        let monitor = self.monitors.get_monitor_mut();
        monitor.add_workspace(
            workspace_name.clone(),
            self.layout_set.clone(),
            &self.default_layout,
        );
        if focus {
            monitor.active_workspace = workspace_name.clone();
        }
        let workspace = monitor.get_workspace_mut(&workspace_name);

        if mapped.is_floating() {
            workspace.add_floating(mapped.clone());
        } else {
            let mapped = workspace.add_tiling(mapped.clone(), None);
            // FIXME
            self.fallback_if_tiling_fails(mapped);
        }

        if let Some(role) = occupant_role {
            let monitor = self.monitors.get_monitor_mut();
            let workspace = monitor.get_workspace_mut(&workspace_name);
            workspace.set_special_window(SpecialWindowAction::Set(mapped.clone()), role);
        }

        if focus {
            self.focus_to_window(&mapped.wl_surface());
        }

        self.save_layout_history();
    }

    pub fn toggle_window_floating_state(&mut self, value: Option<bool>) {
        let keyboard = self.get_keyboard();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some(FoundMappedWindow {
            mapped,
            workspace_name,
            ..
        }) = self.remove_mapped_window(&surface)
        else {
            return;
        };

        mapped.set_floating(value.unwrap_or(!mapped.is_floating()));

        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&workspace_name);

        if mapped.is_floating() {
            workspace.add_floating(mapped);
        } else {
            let mapped = workspace.add_tiling(mapped, None);
            self.fallback_if_tiling_fails(mapped);
        }

        self.save_layout_history();
    }

    pub fn is_floating_window_hidden(&self) -> bool {
        let monitor = self.monitors.get_monitor();
        let workspace_name = monitor.get_active_workspace_name().clone();
        let workspace = monitor.get_workspace(&workspace_name);
        workspace.is_floating_hidden()
    }

    pub fn set_floating_window_visibility(&mut self, value: Option<bool>, focus: Option<bool>) {
        let monitor = self.monitors.get_monitor();
        let workspace_name = monitor.get_active_workspace_name().clone();
        let workspace = monitor.get_workspace(&workspace_name);
        if workspace.has_special_window() {
            let is_maximized_tiling = workspace.get_special_window_role()
                == Some(WindowRole::Maximized)
                && !workspace.special_window_is_floating();
            if !is_maximized_tiling {
                return;
            }
        }

        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&workspace_name);
        workspace.hide_floating(value);
        if focus.unwrap_or(true) || workspace.is_floating_hidden() {
            self.focus_last_window_in_workspace(&workspace_name);
        }
    }

    fn fallback_if_tiling_fails(&mut self, mapped: Option<MappedWindow>) {
        let Some(mapped) = mapped else {
            return;
        };

        let workspace_name = self
            .monitors
            .get_monitor()
            .get_active_workspace_name()
            .clone();

        let properties = self.window_rules.get_opening_properties(
            &mapped.wl_surface(),
            workspace_name,
            Some(WindowState::Float {
                location: None,
                size: None,
            }),
        );

        self.register_new_window(mapped.window(), properties);
    }

    pub fn focus_adjacent_tiling_window(&mut self, direction: Direction) {
        let keyboard = self.get_keyboard();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some(FoundMappedWindow {
            mapped,
            workspace_name,
            ..
        }) = self.find_mapped_window_by_surface(&surface)
        else {
            return;
        };
        if mapped.is_floating() {
            return;
        }

        let monitor = self.monitors.get_monitor();
        let workspace = monitor.get_workspace(&workspace_name);
        if workspace.has_special_window() {
            return;
        }

        if let Some(mapped) = workspace.last_focused_tiling_window(&surface, direction) {
            self.focus_to_window(&mapped.wl_surface());
        }
    }

    pub fn swap_tiling_window(&mut self, lhs: &WlSurface, rhs: &WlSurface) {
        let Some(FoundMappedWindow {
            mapped: mapped_lhs,
            workspace_name,
            ..
        }) = self.find_mapped_window_by_surface(lhs)
        else {
            return;
        };
        let Some(FoundMappedWindow {
            mapped: mapped_rhs, ..
        }) = self.find_mapped_window_by_surface(rhs)
        else {
            return;
        };
        if mapped_lhs.is_floating() || mapped_rhs.is_floating() {
            return;
        }

        let monitor = self.monitors.get_monitor();
        let workspace = monitor.get_workspace(&workspace_name);
        if workspace.has_special_window() {
            return;
        }

        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&workspace_name);
        workspace.swap_tiling_windows(lhs, rhs);

        self.save_layout_history();
    }

    pub fn swap_with_adjacent_tiling_window(&mut self, direction: Direction) {
        let keyboard = self.get_keyboard();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some(FoundMappedWindow {
            mapped: mapped_lhs,
            workspace_name,
            ..
        }) = self.find_mapped_window_by_surface(&surface)
        else {
            return;
        };
        if mapped_lhs.is_floating() {
            return;
        }

        let monitor = self.monitors.get_monitor();
        let workspace = monitor.get_workspace(&workspace_name);
        if workspace.has_special_window() {
            return;
        }

        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&workspace_name);
        if let Some(mapped_rhs) = workspace
            .last_focused_tiling_window(&surface, direction)
            .cloned()
        {
            workspace.swap_tiling_windows(&mapped_lhs.wl_surface(), &mapped_rhs.wl_surface());
        }

        self.save_layout_history();
    }

    pub fn resize_adjacent_tiling_window(
        &mut self,
        direction: Direction,
        unit: impl Into<TileResizeUnit>,
    ) {
        let keyboard = self.get_keyboard();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some(FoundMappedWindow {
            mapped,
            workspace_name,
            ..
        }) = self.find_mapped_window_by_surface(&surface)
        else {
            return;
        };
        if mapped.is_floating() {
            return;
        }

        let monitor = self.monitors.get_monitor();
        let workspace = monitor.get_workspace(&workspace_name);
        if workspace.has_special_window() {
            return;
        }

        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&workspace_name);
        workspace.resize_tiling(&surface, direction, unit);
    }

    pub fn toggle_window_role(
        &mut self,
        surface: &WlSurface,
        role: WindowRole,
        value: Option<bool>,
    ) {
        let Some(FoundMappedWindow {
            mapped,
            workspace_name,
            ..
        }) = self.find_mapped_window_by_surface(surface)
        else {
            return;
        };
        let action = match value {
            Some(true) => SpecialWindowAction::Set(mapped),
            Some(false) => SpecialWindowAction::Unset,
            None => SpecialWindowAction::Toggle(mapped),
        };
        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&workspace_name);
        workspace.set_special_window(action, role);
    }
}
