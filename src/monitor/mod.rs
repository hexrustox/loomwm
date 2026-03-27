use std::{cell::RefCell, rc::Rc};

use smithay::{
    backend::renderer::RendererSuper,
    desktop::WindowSurfaceType,
    output::Output,
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::protocol::wl_surface::WlSurface,
    },
    utils::{Logical, Point},
};

use crate::{
    state::WindowManagerState,
    utils::{
        get_monotonic_time,
        types::{RenderElements, Renderer},
    },
    window::MappedWindow,
};

pub use assistant::{BackendDevice, LayoutHistory};
pub use workspace::{
    LayoutSet, TileRatio, TileTreeSearchKey, TileTreeWindow, Workspace, WorkspaceName,
};

mod action;
mod assistant;
mod workspace;

#[derive(Debug, Default)]
pub struct Monitors {
    monitors: Vec<Monitor>,
}

impl Monitors {
    pub fn push(&mut self, output: Output, layouts: Rc<RefCell<LayoutSet>>, layout_name: &str) {
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
}

impl Monitor {
    fn new(output: Output, layouts: Rc<RefCell<LayoutSet>>, layout_name: &str) -> Self {
        let active_workspace = WorkspaceName::Id(1);
        Self {
            output: output.clone(),
            active_workspace: active_workspace.clone(),
            workspaces: vec![Workspace::new(
                output,
                active_workspace,
                layouts,
                layout_name,
            )],
        }
    }

    pub fn get_active_workspace_name(&self) -> &WorkspaceName {
        &self.active_workspace
    }

    fn find_workspace_index(&self, workspace_name: &WorkspaceName) -> Option<usize> {
        #[allow(irrefutable_let_patterns)]
        let WorkspaceName::Id(id) = workspace_name else {
            return None;
        };
        self.workspaces
            .binary_search_by_key(id, |workspace| workspace.get_name().as_id())
            .ok()
    }

    pub fn get_workspace(&self, workspace_name: &WorkspaceName) -> &Workspace {
        if let Some(index) = self.find_workspace_index(workspace_name) {
            return &self.workspaces[index];
        }
        unreachable!()
    }

    pub fn get_workspace_mut(&mut self, workspace_name: &WorkspaceName) -> &mut Workspace {
        if let Some(index) = self.find_workspace_index(workspace_name) {
            return &mut self.workspaces[index];
        }
        unreachable!()
    }

    fn add_workspace(
        &mut self,
        workspace_name: WorkspaceName,
        layouts: Rc<RefCell<LayoutSet>>,
        layout_name: &str,
    ) {
        match workspace_name {
            WorkspaceName::Id(id) => {
                if let Err(index) = self.workspaces.binary_search_by_key(&id, |workspace| {
                    let WorkspaceName::Id(id) = workspace.get_name();
                    *id
                }) {
                    self.workspaces.insert(
                        index,
                        Workspace::new(self.output.clone(), workspace_name, layouts, layout_name),
                    );
                }
            }
        }
    }

    fn remove_workspace(&mut self, workspace_name: &WorkspaceName) -> Option<Workspace> {
        match workspace_name {
            WorkspaceName::Id(id) => {
                if let Ok(index) = self.workspaces.binary_search_by_key(&id, |workspace| {
                    let WorkspaceName::Id(id) = workspace.get_name();
                    id
                }) {
                    return Some(self.workspaces.remove(index));
                }
            }
        }

        None
    }
}

pub struct FoundMappedWindow {
    pub mapped: MappedWindow,
    pub workspace_name: WorkspaceName,
    pub location: Option<Point<i32, Logical>>,
}

impl WindowManagerState {
    pub fn find_mapped_window_by_surface(&self, surface: &WlSurface) -> Option<FoundMappedWindow> {
        self.monitors
            .get_monitor()
            .workspaces
            .iter()
            .find_map(|workspace| {
                workspace
                    .find_window_by_surface(surface)
                    .map(|mapped| FoundMappedWindow {
                        mapped: mapped.clone(),
                        workspace_name: workspace.get_name().clone(),
                        location: None,
                    })
            })
    }

    pub fn remove_mapped_window(&mut self, surface: &WlSurface) -> Option<FoundMappedWindow> {
        self.monitors
            .get_monitor_mut()
            .workspaces
            .iter_mut()
            .find_map(|workspace| {
                let workspace_name = workspace.get_name().clone();
                workspace
                    .remove_window_by_surface(surface)
                    .map(|mapped| FoundMappedWindow {
                        mapped,
                        workspace_name,
                        location: None,
                    })
            })
    }

    pub fn find_mapped_window_at_point(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<FoundMappedWindow> {
        let monitor = self.monitors.get_monitor();
        let workspace_name = monitor.get_active_workspace_name();
        monitor
            .get_workspace(workspace_name)
            .window_at_point(point)
            .map(|(mapped, location)| FoundMappedWindow {
                mapped: mapped.clone(),
                workspace_name: workspace_name.clone(),
                location: Some(location),
            })
    }

    pub fn find_surface_at_point(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.find_mapped_window_at_point(point).and_then(
            |FoundMappedWindow {
                 mapped, location, ..
             }| {
                let Some(location) = location else {
                    unreachable!()
                };
                mapped
                    .window()
                    .surface_under(point - location.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, p)| (s, (p + location).to_f64()))
            },
        )
    }

    pub fn apply_window_rules(&self) {
        let monitor = self.monitors.get_monitor();
        for workspace in &monitor.workspaces {
            let workspace_name = workspace.get_name();
            for mapped in workspace.iter_all_windows() {
                let properties = self
                    .window_rules
                    .get_dynamic_properties(mapped, workspace_name.clone());

                use xdg_toplevel::State::*;
                mapped.toplevel().with_pending_state(|state| {
                    if mapped.is_floating() {
                        state.states.unset(TiledTop);
                        state.states.unset(TiledBottom);
                        state.states.unset(TiledLeft);
                        state.states.unset(TiledRight);
                    } else {
                        state.states.set(TiledTop);
                        state.states.set(TiledBottom);
                        state.states.set(TiledLeft);
                        state.states.set(TiledRight);
                    }

                    if mapped.is_fullscreen() {
                        state.states.set(Fullscreen);
                    } else {
                        state.states.unset(Fullscreen);
                    }

                    if mapped.is_maximized() {
                        state.states.set(Maximized);
                    } else {
                        state.states.unset(Maximized);
                    }
                });

                mapped.toplevel().with_pending_state(|state| {
                    state.decoration_mode = properties.decoration.map(|d| d.into());
                });

                mapped.set_border(properties.border);

                mapped.set_opacity(properties.opacity.unwrap_or(1.0));
            }
        }
    }

    pub fn update_tiling_layout(&mut self, layout_name: &str) {
        let monitor = self.monitors.get_monitor_mut();
        for workspace in &mut monitor.workspaces {
            workspace.update_tiling_layout(layout_name);
        }
    }

    pub fn refresh_windows(&self) {
        let monitor = self.monitors.get_monitor();
        for workspace in &monitor.workspaces {
            workspace.refresh_windows();
        }
    }

    pub fn send_frame_to_windows(&self) {
        let monitor = self.monitors.get_monitor();
        for workspace in &monitor.workspaces {
            let output = workspace.get_output();
            let time = get_monotonic_time();

            workspace.iter_all_windows().for_each(|mapped| {
                mapped
                    .window()
                    .send_frame(&output, time, None, |_, _| Some(output.clone()))
            });
        }
    }

    pub fn render_elements<R: Renderer>(&mut self, renderer: &mut R) -> Vec<RenderElements<R>>
    where
        <R as RendererSuper>::TextureId: Clone + 'static,
    {
        let monitor = self.monitors.get_monitor_mut();
        monitor
            .get_workspace_mut(&monitor.get_active_workspace_name().clone())
            .render_elements(renderer)
    }
}
