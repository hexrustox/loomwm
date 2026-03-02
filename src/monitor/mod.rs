use std::rc::Rc;

use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper,
        element::{surface::WaylandSurfaceRenderElement, utils::CropRenderElement},
    },
    desktop::WindowSurfaceType,
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Scale},
};

use crate::{monitor::workspace::Workspace, state::WindowManagerState, window::MappedWindow};

pub use assistant::LayoutRecord;
pub use workspace::{LayoutSet, TileRatio, TileTreeSearchKey, TileTreeWindow, WorkspaceName};

mod action;
mod assistant;
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

    fn add_workspace(&mut self, workspace_name: WorkspaceName) {
        match workspace_name {
            WorkspaceName::Id(id) => {
                if let Err(index) = self.workspaces.binary_search_by_key(&id, |workspace| {
                    let WorkspaceName::Id(id) = workspace.get_name();
                    *id
                }) {
                    self.workspaces.insert(
                        index,
                        Workspace::new(
                            self.output.clone(),
                            workspace_name,
                            self.layouts.clone(),
                            &self.layout_name,
                        ),
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
    pub fn find_mapped_window(&self, surface: &WlSurface) -> Option<FoundMappedWindow> {
        self.monitors
            .get_monitor()
            .workspaces
            .iter()
            .find_map(|workspace| {
                workspace
                    .find_window(surface)
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
                    .remove_window(surface)
                    .map(|mapped| FoundMappedWindow {
                        mapped,
                        workspace_name,
                        location: None,
                    })
            })
    }

    pub fn find_mapped_window_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<FoundMappedWindow> {
        let monitor = self.monitors.get_monitor();
        let workspace_name = monitor.get_active_workspace_name();
        monitor
            .get_workspace(workspace_name)
            .find_mapped_window_under(point)
            .map(|(mapped, location)| FoundMappedWindow {
                mapped: mapped.clone(),
                workspace_name: workspace_name.clone(),
                location: Some(location),
            })
    }

    pub fn find_surface_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.find_mapped_window_under(point).and_then(
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

    pub fn apply_rule_to_mapped_windows(&mut self) {
        let monitor = self.monitors.get_monitor_mut();
        monitor.workspaces.iter_mut().for_each(|workspace| {
            workspace.apply_rule_to_windows(&self.window_rules);
        });
    }

    pub fn windows_in_active_workspace_iter(&self) -> impl Iterator<Item = &MappedWindow> {
        let monitor = self.monitors.get_monitor();
        monitor
            .get_workspace(monitor.get_active_workspace_name())
            .windows_iter()
    }

    pub fn render_elements<R>(
        &mut self,
        renderer: &mut R,
        scale: Scale<f64>,
    ) -> Vec<CropRenderElement<WaylandSurfaceRenderElement<R>>>
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
