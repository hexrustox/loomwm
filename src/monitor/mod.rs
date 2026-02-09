use std::rc::Rc;

use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper, element::surface::WaylandSurfaceRenderElement,
    },
    desktop::{Window, WindowSurfaceType},
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, SERIAL_COUNTER, Scale},
};

use crate::{
    input::{WindowDirection, WindowUnit},
    monitor::workspace::Workspace,
    state::WaylandState,
    utils::get_app_id_and_title,
    window::{
        MappedWindow,
        rule::{
            WindowDynamicProperties, WindowLocation, WindowOpeningProperties, WindowProperties,
            WindowRuleCandidate, WindowState,
        },
    },
};

pub use workspace::{
    LayoutSet, TileRatio, TileTreeWindow, TileTreeWindowId, WorkspaceName, test_layout_set,
};

mod workspace;

#[derive(Debug, Default)]
pub struct Monitors {
    monitors: Vec<Monitor>,
}

impl Monitors {
    pub fn push(&mut self, output: Output, layouts: Rc<LayoutSet>, layout_name: Rc<str>) {
        self.monitors
            .push(Monitor::new(output, layouts, layout_name));
    }

    pub fn get_monitor(&self) -> &Monitor {
        self.monitors.last().unwrap()
    }
    pub fn get_monitor_mut(&mut self) -> &mut Monitor {
        self.monitors.last_mut().unwrap()
    }
}

#[derive(Debug)]
pub struct Monitor {
    output: Output,

    active_workspace: WorkspaceName,
    workspaces: Vec<Workspace>,

    layouts: Rc<LayoutSet>,
    layout_name: Rc<str>,
}

impl Monitor {
    fn new(output: Output, layouts: Rc<LayoutSet>, layout_name: Rc<str>) -> Self {
        let active_workspace = WorkspaceName::Id(1);
        Self {
            output: output.clone(),
            active_workspace: active_workspace.clone(),
            workspaces: vec![Workspace::new(
                output,
                active_workspace,
                layouts.clone(),
                &layout_name,
            )],
            layouts,
            layout_name,
        }
    }

    pub fn get_active_workspace_name(&self) -> &WorkspaceName {
        &self.active_workspace
    }

    fn find_workspace(&self, name: &WorkspaceName) -> Option<usize> {
        self.workspaces
            .binary_search_by_key(&name, |workspace| workspace.get_name())
            .ok()
    }

    pub fn get_workspace(&self, name: &WorkspaceName) -> &Workspace {
        if let Some(index) = self.find_workspace(name) {
            return &self.workspaces[index];
        }
        unreachable!()
    }

    pub fn get_workspace_mut(&mut self, name: &WorkspaceName) -> &mut Workspace {
        if let Some(index) = self.find_workspace(name) {
            return &mut self.workspaces[index];
        }
        unreachable!()
    }

    fn add_workspace(&mut self, name: WorkspaceName) {
        match name {
            WorkspaceName::Id(id) => {
                if let Err(index) = self.workspaces.binary_search_by_key(&id, |workspace| {
                    let WorkspaceName::Id(id) = workspace.get_name();
                    *id
                }) {
                    self.workspaces.insert(
                        index,
                        Workspace::new(
                            self.output.clone(),
                            name,
                            self.layouts.clone(),
                            &self.layout_name,
                        ),
                    );
                }
            }
        }
    }
}

impl WaylandState {
    pub fn add_window(&mut self, window: Window, properties: WindowProperties) {
        let WindowOpeningProperties {
            focus,
            state,
            workspace,
        } = properties.opening.unwrap();

        let focus = focus.unwrap_or(true);
        let mut mapped = MappedWindow::new(window, focus);

        let surface = if focus {
            Some(mapped.toplevel().wl_surface().clone())
        } else {
            None
        };

        let monitor = self.monitors.get_monitor_mut();
        let name = workspace
            .inspect(|name| {
                monitor.add_workspace(name.clone());
            })
            .unwrap_or(monitor.get_active_workspace_name().clone());
        if focus {
            monitor.active_workspace = name.clone();
        }
        let workspace = monitor.get_workspace_mut(&name);

        mapped.is_floating = state
            .as_ref()
            .is_some_and(|state| matches!(state, WindowState::Float { .. }));
        match state.unwrap_or_default() {
            WindowState::Float { location, size } => {
                match location {
                    Some(WindowLocation::Location(x, y)) => {
                        mapped.location = (x, y).into();
                    }
                    Some(WindowLocation::Center) => {
                        let output_size = &workspace.output_size();
                        let window_size = size
                            .map(|(w, h)| (w, h).into())
                            .unwrap_or(mapped.window.geometry().size);
                        mapped.location = (
                            output_size.w / 2 - window_size.w / 2,
                            output_size.h / 2 - window_size.h / 2,
                        )
                            .into();
                    }
                    _ => {}
                }
                let size = size
                    .map(|(w, h)| (w, h).into())
                    .unwrap_or(mapped.window.geometry().size);
                mapped.toplevel().with_pending_state(|state| {
                    state.size = Some(size);
                });

                apply_rule_to_mapped_window(&mut mapped, properties.dynamic);
                workspace.add_floating_window(mapped);
            }
            WindowState::Tile(ratio) => {
                apply_rule_to_mapped_window(&mut mapped, properties.dynamic);
                let mapped = workspace.add_tiling_window(mapped, ratio);
                self.handle_tilting_layout_full(mapped);
            }
        }

        if let Some(surface) = surface {
            self.focus_window(&surface);
        }
    }

    fn handle_tilting_layout_full(&mut self, mapped: Option<MappedWindow>) {
        let Some(mapped) = mapped else {
            return;
        };
        let (app_id, title) = get_app_id_and_title(mapped.toplevel().wl_surface());

        let candidate = WindowRuleCandidate {
            app_id,
            title,
            focus: true,
            float: true,
            workspace: self
                .monitors
                .get_monitor()
                .get_active_workspace_name()
                .clone(),
        };
        let mut properties = self.window_rules.get_properties(candidate, true);
        if let Some(opening) = properties.opening.as_mut() {
            opening.state = Some(WindowState::Float {
                location: None,
                size: None,
            })
        } else {
            properties.opening = Some(WindowOpeningProperties {
                state: Some(WindowState::Float {
                    location: None,
                    size: None,
                }),
                ..Default::default()
            })
        }

        self.add_window(mapped.window, properties);
    }

    pub fn find_mapped_window(
        &self,
        surface: &WlSurface,
    ) -> Option<(&MappedWindow, WorkspaceName)> {
        self.monitors
            .get_monitor()
            .workspaces
            .iter()
            .find_map(|workspace| {
                workspace
                    .find_window(surface)
                    .map(|mapped| (mapped, workspace.get_name().clone()))
            })
    }

    pub fn find_mapped_window_mut(
        &mut self,
        surface: &WlSurface,
    ) -> Option<(&mut MappedWindow, WorkspaceName)> {
        self.monitors
            .get_monitor_mut()
            .workspaces
            .iter_mut()
            .find_map(|workspace| {
                let name = workspace.get_name().clone();
                workspace
                    .find_window_mut(surface)
                    .map(|mapped| (mapped, name))
            })
    }

    pub fn remove_mapped_window(
        &mut self,
        surface: &WlSurface,
    ) -> Option<(MappedWindow, WorkspaceName)> {
        self.monitors
            .get_monitor_mut()
            .workspaces
            .iter_mut()
            .find_map(|workspace| {
                let name = workspace.get_name().clone();
                workspace
                    .remove_window(surface)
                    .map(|mapped| (mapped, name))
            })
    }

    pub fn focus_window(&mut self, surface: &WlSurface) {
        let keyboard = self.seat.get_keyboard().unwrap();

        if let Some(old_surface) = keyboard.current_focus() {
            if old_surface == *surface {
                return;
            }
            if let Some((mapped, name)) = self
                .monitors
                .get_monitor_mut()
                .workspaces
                .iter_mut()
                .find_map(|workspace| {
                    let name = workspace.get_name().clone();
                    workspace
                        .find_window_mut(&old_surface)
                        .map(|mapped| (mapped, name))
                })
            {
                mapped.is_focused = false;

                mapped.window.set_activated(false);

                let (app_id, title) = get_app_id_and_title(&old_surface);
                let focus = mapped.is_focused;
                let float = mapped.is_floating;
                let properties = self.window_rules.get_properties(
                    WindowRuleCandidate {
                        app_id,
                        title,
                        focus,
                        float,
                        workspace: name,
                    },
                    false,
                );
                apply_rule_to_mapped_window(mapped, properties.dynamic);
            }
        }

        keyboard.set_focus(self, Some(surface.clone()), SERIAL_COUNTER.next_serial());
        let Some((mapped, name)) = self
            .monitors
            .get_monitor_mut()
            .workspaces
            .iter_mut()
            .find_map(|workspace| {
                let name = workspace.get_name().clone();
                workspace
                    .find_window_mut(surface)
                    .map(|mapped| (mapped, name))
            })
        else {
            return;
        };
        mapped.is_focused = true;

        mapped.window.set_activated(true);

        let (app_id, title) = get_app_id_and_title(surface);
        let focus = mapped.is_focused;
        let float = mapped.is_floating;
        let properties = self.window_rules.get_properties(
            WindowRuleCandidate {
                app_id,
                title,
                focus,
                float,
                workspace: name.clone(),
            },
            false,
        );
        apply_rule_to_mapped_window(mapped, properties.dynamic);

        let floating = mapped.is_floating;
        let window = mapped.window.clone();
        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&name);
        workspace.update_focus_queue(window);
        if floating {
            workspace.raise_floating_window(surface);
        }
    }

    pub fn mapped_window_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(&MappedWindow, Point<i32, Logical>)> {
        let monitor = self.monitors.get_monitor();
        monitor
            .get_workspace(monitor.get_active_workspace_name())
            .mapped_window_under(point)
    }

    pub fn surface_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.mapped_window_under(point)
            .and_then(|(window, location)| {
                window
                    .window
                    .surface_under(point - location.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, p)| (s, (p + location).to_f64()))
            })
    }

    pub fn restore_workspace_focus(&mut self, name: &WorkspaceName) {
        let monitor = self.monitors.get_monitor_mut();
        if let Some(window) = monitor.get_workspace(name).last_focus_window() {
            let surface = window.toplevel().unwrap().wl_surface().clone();
            self.focus_window(&surface);
        } else {
            self.seat
                .get_keyboard()
                .unwrap()
                .set_focus(self, None, SERIAL_COUNTER.next_serial());
        }
    }

    pub fn switch_to_workspace(&mut self, name: WorkspaceName) {
        let monitor = self.monitors.get_monitor_mut();
        monitor.add_workspace(name.clone());
        monitor.active_workspace = name.clone();
        self.restore_workspace_focus(&name);
    }

    pub fn move_focused_window_to_workspace(&mut self, name: WorkspaceName, focus: bool) {
        let keyboard = self.seat.get_keyboard().unwrap();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some((mut mapped, old_name)) = self.remove_mapped_window(&surface) else {
            return;
        };

        let mut surface = None;
        if focus {
            surface = Some(mapped.toplevel().wl_surface().clone());
        } else {
            self.restore_workspace_focus(&old_name);
        }

        let (app_id, title) = get_app_id_and_title(mapped.toplevel().wl_surface());
        let properties = self.window_rules.get_properties(
            WindowRuleCandidate {
                app_id,
                title,
                focus,
                float: mapped.is_floating,
                workspace: name.clone(),
            },
            false,
        );

        apply_rule_to_mapped_window(&mut mapped, properties.dynamic);

        let monitor = self.monitors.get_monitor_mut();
        monitor.add_workspace(name.clone());
        if focus {
            monitor.active_workspace = name.clone();
        }
        let workspace = monitor.get_workspace_mut(&name);
        if mapped.is_floating {
            workspace.add_floating_window(mapped);
        } else {
            let mapped = workspace.add_tiling_window(mapped, None);
            self.handle_tilting_layout_full(mapped);
        }

        if let Some(surface) = surface {
            self.focus_window(&surface);
        }
    }

    pub fn toggle_focused_window_floating(&mut self) {
        let keyboard = self.seat.get_keyboard().unwrap();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some((mut mapped, name)) = self.remove_mapped_window(&surface) else {
            return;
        };

        let (app_id, title) = get_app_id_and_title(mapped.toplevel().wl_surface());
        mapped.is_floating = !mapped.is_floating;
        let properties = self.window_rules.get_properties(
            WindowRuleCandidate {
                app_id,
                title,
                focus: true,
                float: mapped.is_floating,
                workspace: name.clone(),
            },
            false,
        );

        apply_rule_to_mapped_window(&mut mapped, properties.dynamic);

        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&name);
        if mapped.is_floating {
            workspace.add_floating_window(mapped);
        } else {
            let mapped = workspace.add_tiling_window(mapped, None);
            self.handle_tilting_layout_full(mapped);
        }
    }

    pub fn close_focused_window(&self) {
        let keyboard = self.seat.get_keyboard().unwrap();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some((mapped, _)) = self.find_mapped_window(&surface) else {
            return;
        };

        mapped.toplevel().send_close();
    }

    pub fn focus_window_in_direction(&mut self, direction: WindowDirection) {
        let keyboard = self.seat.get_keyboard().unwrap();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some((mapped, name)) = self.find_mapped_window(&surface) else {
            return;
        };
        if mapped.is_floating {
            return;
        }
        let monitor = self.monitors.get_monitor();
        let workspace = monitor.get_workspace(&name);
        if let Some(window) = workspace.last_window_in_direction(&surface, direction) {
            let window = window.clone();
            self.focus_window(window.toplevel().unwrap().wl_surface());
        }
    }

    pub fn swap_window_in_direction(&mut self, direction: WindowDirection) {
        let keyboard = self.seat.get_keyboard().unwrap();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some((mapped, name)) = self.find_mapped_window(&surface) else {
            return;
        };
        if mapped.is_floating {
            return;
        }
        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&name);
        if let Some(window) = workspace.last_window_in_direction(&surface, direction) {
            let window = window.clone();
            workspace.swap_tiling_window(window.toplevel().unwrap().wl_surface(), &surface);

            let Some((mapped, _)) = self.find_mapped_window(&surface) else {
                return;
            };
            let temp = mapped.window.geometry().size;
            mapped.toplevel().with_pending_state(|state| {
                state.size = Some(window.geometry().size);
            });
            mapped.toplevel().send_pending_configure();
            let toplevel = window.toplevel().unwrap();
            toplevel.with_pending_state(|state| {
                state.size = Some(temp);
            });
            toplevel.send_pending_configure();
        }
    }

    pub fn resize_window_in_edge(&mut self, edge: WindowDirection, unit: WindowUnit) {
        let keyboard = self.seat.get_keyboard().unwrap();
        let Some(surface) = keyboard.current_focus() else {
            return;
        };
        let Some((mapped, name)) = self.find_mapped_window(&surface) else {
            return;
        };
        if mapped.is_floating {
            return;
        }
        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&name);
        workspace.resize_tiling_window(&surface, edge, unit);
    }

    pub fn active_windows_iter(&self) -> impl Iterator<Item = &MappedWindow> {
        let monitor = self.monitors.get_monitor();
        monitor
            .get_workspace(monitor.get_active_workspace_name())
            .windows_iter()
    }

    pub fn render_elements<R>(
        &mut self,
        renderer: &mut R,
        scale: Scale<f64>,
    ) -> Vec<WaylandSurfaceRenderElement<R>>
    where
        R: Renderer + ImportAll,
        <R as RendererSuper>::TextureId: Clone + 'static,
    {
        let monitor = self.monitors.get_monitor_mut();
        monitor
            .get_workspace_mut(&monitor.get_active_workspace_name().clone())
            .render_elements(renderer, scale)
    }
}

fn apply_rule_to_mapped_window(mapped: &mut MappedWindow, properties: WindowDynamicProperties) {
    mapped.toplevel().with_pending_state(|state| {
        state.decoration_mode = properties.decoration.map(|d| d.into());
    });
    if let Some(opacity) = properties.opacity {
        mapped.opacity = opacity;
    }
    mapped.toplevel().send_pending_configure();
}
