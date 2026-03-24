use smithay::{
    desktop::Window, reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::SERIAL_COUNTER,
};

use crate::{
    monitor::{FoundMappedWindow, workspace::TileResizeUnit},
    state::WindowManagerState,
    utils::{Direction, apply_rule_to_mapped_window, get_app_id_and_title},
    window::{
        MappedWindow,
        rule::{
            WindowLocation, WindowOpeningProperties, WindowProperties, WindowRuleCandidate,
            WindowState,
        },
    },
};

pub use super::workspace::{TileTreeWindow, WorkspaceName};

impl WindowManagerState {
    pub fn add_window(&mut self, window: Window, properties: WindowProperties) {
        let Some(WindowOpeningProperties {
            focus,
            state,
            workspace_name,
        }) = properties.opening
        else {
            #[cfg(test)]
            panic!("Missing window opening properties");
            #[allow(unreachable_code)]
            return;
        };

        let focus = focus.unwrap_or(true);
        let floating = matches!(state, Some(WindowState::Float { .. }));
        let mut mapped = MappedWindow::new(window, focus, floating);

        let monitor = self.monitors.get_monitor_mut();
        let workspace_name = workspace_name
            .inspect(|name| {
                monitor.add_workspace(name.clone(), self.layout_set.clone(), &self.default_layout);
            })
            .unwrap_or(monitor.get_active_workspace_name().clone());
        if focus {
            monitor.active_workspace = workspace_name.clone();
        }
        let workspace = monitor.get_workspace_mut(&workspace_name);

        match state.unwrap_or_default() {
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

                apply_rule_to_mapped_window(&mapped, properties.dynamic);
                workspace.add_floating_window(mapped.clone());
            }
            WindowState::Tile { ratio } => {
                apply_rule_to_mapped_window(&mapped, properties.dynamic);
                let mapped = workspace.add_tiling_window(mapped.clone(), ratio);
                self.handle_tiling_layout_full(mapped.clone());
                if mapped.is_some() {
                    return;
                }
            }
        }

        if mapped.get_focus() {
            self.focus_window(&mapped.wl_surface());
        }

        self.save_layout_history();
    }

    fn handle_tiling_layout_full(&mut self, mapped: Option<MappedWindow>) {
        let Some(mapped) = mapped else {
            return;
        };
        let (app_id, title) = get_app_id_and_title(&mapped.wl_surface());

        let candidate = WindowRuleCandidate {
            app_id,
            title,
            focus: true,
            float: true,
            is_swap_source: false,
            is_swap_target: false,
            workspace_name: self
                .monitors
                .get_monitor()
                .get_active_workspace_name()
                .clone(),
        };
        let mut properties = self.window_rules.get_properties(candidate, true);
        properties = properties.merge(WindowProperties {
            opening: Some(WindowOpeningProperties {
                state: Some(WindowState::Float {
                    location: None,
                    size: None,
                }),
                ..Default::default()
            }),
            ..Default::default()
        });

        self.add_window(mapped.window(), properties);
    }

    pub fn focus_window(&mut self, surface: &WlSurface) {
        let keyboard = self.get_keyboard();

        if let Some(old_surface) = keyboard.current_focus() {
            if old_surface == *surface {
                return;
            }
            if let Some(FoundMappedWindow {
                mapped,
                workspace_name,
                ..
            }) = self.find_mapped_window(&old_surface)
            {
                mapped.set_focus(false);
                mapped.window().set_activated(false);

                let (app_id, title) = get_app_id_and_title(&old_surface);
                let properties = self.window_rules.get_properties(
                    WindowRuleCandidate {
                        app_id,
                        title,
                        focus: mapped.get_focus(),
                        float: mapped.get_floating(),
                        is_swap_source: false,
                        is_swap_target: false,
                        workspace_name,
                    },
                    false,
                );
                apply_rule_to_mapped_window(&mapped, properties.dynamic);
            }
        }
        keyboard.set_focus(self, Some(surface.clone()), SERIAL_COUNTER.next_serial());
        let Some(FoundMappedWindow {
            mapped,
            workspace_name,
            ..
        }) = self.find_mapped_window(surface)
        else {
            return;
        };
        mapped.set_focus(true);
        mapped.window().set_activated(true);

        let (app_id, title) = get_app_id_and_title(surface);
        let properties = self.window_rules.get_properties(
            WindowRuleCandidate {
                app_id,
                title,
                focus: mapped.get_focus(),
                float: mapped.get_floating(),
                is_swap_source: false,
                is_swap_target: false,
                workspace_name: workspace_name.clone(),
            },
            false,
        );
        apply_rule_to_mapped_window(&mapped, properties.dynamic);

        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&workspace_name);
        workspace.append_to_focus_queue(mapped.clone());
        if mapped.get_floating() {
            workspace.raise_floating_window(surface);
        }
    }

    pub fn restore_workspace_focus(&mut self, workspace_name: &WorkspaceName) {
        let monitor = self.monitors.get_monitor_mut();
        if let Some(mapped) = monitor
            .get_workspace(workspace_name)
            .get_last_focused_window()
            .cloned()
        {
            self.focus_window(&mapped.wl_surface());
        } else {
            self.get_keyboard()
                .set_focus(self, None, SERIAL_COUNTER.next_serial());
        }
    }

    pub fn switch_or_create_active_workspace(&mut self, workspace_name: WorkspaceName) {
        let monitor = self.monitors.get_monitor_mut();

        let old_workspace_name = monitor.get_active_workspace_name().clone();
        let old_workspace = monitor.get_workspace(&old_workspace_name);
        if old_workspace.windows_count() == 0 {
            monitor.remove_workspace(&old_workspace_name);
        }

        monitor.add_workspace(
            workspace_name.clone(),
            self.layout_set.clone(),
            &self.default_layout,
        );
        monitor.active_workspace = workspace_name.clone();

        self.restore_workspace_focus(&workspace_name);
    }

    // FIXME use switch/create active workspace method
    fn goto_workspace_by_delta(&mut self, delta: i32) {
        let monitor = self.monitors.get_monitor_mut();

        let old_workspace_name = monitor.get_active_workspace_name();
        let Some(index) = monitor.find_workspace_index(old_workspace_name) else {
            return;
        };

        let len = monitor.workspaces.len() as i32;
        let new_index = (index as i32 + delta).rem_euclid(len) as usize;

        let new_workspace_name = monitor.workspaces[new_index].get_name().clone();
        monitor.active_workspace = new_workspace_name.clone();
        self.restore_workspace_focus(&new_workspace_name);
    }

    pub fn goto_next_workspace(&mut self) {
        self.goto_workspace_by_delta(1);
    }

    pub fn goto_prev_workspace(&mut self) {
        self.goto_workspace_by_delta(-1);
    }

    pub fn move_focused_window_to_workspace(&mut self, workspace_name: WorkspaceName, focus: bool) {
        let keyboard = self.get_keyboard();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some(FoundMappedWindow {
            mapped,
            workspace_name: old_workspace_name,
            ..
        }) = self.remove_mapped_window(&surface)
        else {
            return;
        };

        if !focus {
            self.restore_workspace_focus(&old_workspace_name);
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

        let (app_id, title) = get_app_id_and_title(&mapped.wl_surface());
        let properties = self.window_rules.get_properties(
            WindowRuleCandidate {
                app_id,
                title,
                focus,
                float: mapped.get_floating(),
                is_swap_source: false,
                is_swap_target: false,
                workspace_name: workspace_name.clone(),
            },
            false,
        );
        apply_rule_to_mapped_window(&mapped, properties.dynamic);

        if mapped.get_floating() {
            workspace.add_floating_window(mapped.clone());
        } else {
            let mapped = workspace.add_tiling_window(mapped.clone(), None);
            self.handle_tiling_layout_full(mapped);
        }

        if focus {
            self.focus_window(&mapped.wl_surface());
        }

        self.save_layout_history();
    }

    pub fn toggle_focused_window_floating(&mut self, value: Option<bool>) {
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

        mapped.set_floating(value.unwrap_or(!mapped.get_floating()));

        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&workspace_name);

        let (app_id, title) = get_app_id_and_title(&mapped.wl_surface());
        let properties = self.window_rules.get_properties(
            WindowRuleCandidate {
                app_id,
                title,
                focus: true,
                float: mapped.get_floating(),
                is_swap_source: false,
                is_swap_target: false,
                workspace_name: workspace_name.clone(),
            },
            false,
        );
        apply_rule_to_mapped_window(&mapped, properties.dynamic);

        if mapped.get_floating() {
            workspace.add_floating_window(mapped);
        } else {
            let mapped = workspace.add_tiling_window(mapped, None);
            self.handle_tiling_layout_full(mapped);
        }

        self.save_layout_history();
    }

    pub fn get_focused_workspace_floating_window_hidden(&self) -> bool {
        let monitor = self.monitors.get_monitor();
        let workspace_name = monitor.get_active_workspace_name().clone();
        let workspace = monitor.get_workspace(&workspace_name);
        workspace.get_floating_window_hidden()
    }

    pub fn set_focused_workspace_floating_window_hidden(
        &mut self,
        value: Option<bool>,
        focus: Option<bool>,
    ) {
        let monitor = self.monitors.get_monitor_mut();
        let workspace_name = monitor.get_active_workspace_name().clone();
        let workspace = monitor.get_workspace_mut(&workspace_name);
        workspace.set_floating_window_hidden(value);
        if focus.unwrap_or(true) || workspace.get_floating_window_hidden() {
            self.restore_workspace_focus(&workspace_name);
        }
    }

    pub fn close_focused_window(&mut self) {
        let keyboard = self.get_keyboard();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some(FoundMappedWindow { mapped, .. }) = self.find_mapped_window(&surface) else {
            return;
        };

        mapped.toplevel().send_close();

        self.save_layout_history();
    }

    pub fn focus_tiling_window_in_direction(&mut self, direction: Direction) {
        let keyboard = self.get_keyboard();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some(FoundMappedWindow {
            mapped,
            workspace_name,
            ..
        }) = self.find_mapped_window(&surface)
        else {
            return;
        };
        if mapped.get_floating() {
            return;
        }
        let monitor = self.monitors.get_monitor();
        let workspace = monitor.get_workspace(&workspace_name);
        if let Some(mapped) = workspace.last_focused_tiling_window_in_direction(&surface, direction)
        {
            self.focus_window(&mapped.wl_surface());
        }
    }

    pub fn swap_tiling_window(&mut self, lhs: &WlSurface, rhs: &WlSurface) {
        let Some(FoundMappedWindow {
            mapped: mapped_lhs,
            workspace_name,
            ..
        }) = self.find_mapped_window(lhs)
        else {
            return;
        };
        let Some(FoundMappedWindow {
            mapped: mapped_rhs, ..
        }) = self.find_mapped_window(rhs)
        else {
            return;
        };
        if mapped_lhs.get_floating() || mapped_rhs.get_floating() {
            return;
        }

        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&workspace_name);
        workspace.swap_tiling_window(lhs, rhs);

        self.save_layout_history();
    }

    pub fn swap_focused_tiling_window_in_direction(&mut self, direction: Direction) {
        let keyboard = self.get_keyboard();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some(FoundMappedWindow {
            mapped: mapped_lhs,
            workspace_name,
            ..
        }) = self.find_mapped_window(&surface)
        else {
            return;
        };
        if mapped_lhs.get_floating() {
            return;
        }

        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&workspace_name);
        if let Some(mapped_rhs) = workspace
            .last_focused_tiling_window_in_direction(&surface, direction)
            .cloned()
        {
            workspace.swap_tiling_window(&mapped_lhs.wl_surface(), &mapped_rhs.wl_surface());
        }

        self.save_layout_history();
    }

    pub fn resize_focused_tiling_window(
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
        }) = self.find_mapped_window(&surface)
        else {
            return;
        };
        if mapped.get_floating() {
            return;
        }
        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&workspace_name);
        workspace.resize_tiling_window(&surface, direction, unit);
    }
}
