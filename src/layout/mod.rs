use std::{mem, time::Duration};

use smithay::{
    backend::renderer::element::surface::WaylandSurfaceRenderElement,
    output::Output,
    utils::{Logical, Point, Scale},
};

use crate::{types::MyRenderer, window::mapped::MappedWindow};

pub struct Layout {
    monitor_set: MonitorSet,
}

impl Layout {
    pub fn new() -> Self {
        Self {
            monitor_set: MonitorSet::NoOutputs {
                workspaces: Vec::new(),
            },
        }
    }

    pub fn add_output(&mut self, output: Output) {
        self.monitor_set = match mem::take(&mut self.monitor_set) {
            MonitorSet::Normal { .. } => {
                todo!()
            }
            MonitorSet::NoOutputs { workspaces } => MonitorSet::Normal {
                monitors: vec![Monitor {
                    output,
                    workspaces,
                    active_workspace: 0,
                }],
                active_monitor: 0,
            },
        }
    }

    pub fn add_window(&mut self, window: MappedWindow) {
        match &mut self.monitor_set {
            MonitorSet::Normal {
                monitors,
                active_monitor,
            } => {
                if let Some(monitor) = monitors.get_mut(*active_monitor) {
                    let workspace = if monitor.workspaces.is_empty() {
                        monitor.workspaces.push(Workspace::new());
                        monitor.workspaces.last_mut()
                    } else {
                        monitor.workspaces.get_mut(monitor.active_workspace)
                    }
                    .unwrap();
                    workspace.floating.tiles.push(Tile::new(window, (0, 0)));
                }
            }
            MonitorSet::NoOutputs { workspaces } => todo!(),
        }
    }

    pub fn render_elements<R: MyRenderer>(
        &self,
        renderer: &mut R,
        scale: Scale<f64>,
        time: Duration,
    ) -> Vec<WaylandSurfaceRenderElement<R>> {
        let mut vec = Vec::new();

        match &self.monitor_set {
            MonitorSet::Normal {
                monitors,
                active_monitor,
            } => {
                if let Some(monitor) = monitors.get(*active_monitor)
                    && let Some(workspace) = monitor.workspaces.get(monitor.active_workspace)
                {
                    vec.extend(workspace.render_elements(renderer, scale, &monitor.output, time));
                }
            }
            MonitorSet::NoOutputs { workspaces } => {}
        }

        vec
    }
}

enum MonitorSet {
    Normal {
        monitors: Vec<Monitor>,
        active_monitor: usize,
    },
    NoOutputs {
        workspaces: Vec<Workspace>,
    },
}

impl Default for MonitorSet {
    fn default() -> Self {
        Self::NoOutputs {
            workspaces: Vec::new(),
        }
    }
}

pub struct Monitor {
    pub output: Output,
    pub workspaces: Vec<Workspace>,
    pub active_workspace: usize,
}

pub struct Workspace {
    floating: FloatingSpace,
}

impl Workspace {
    pub fn new() -> Self {
        Self {
            floating: FloatingSpace { tiles: Vec::new() },
        }
    }

    pub fn render_elements<R: MyRenderer>(
        &self,
        renderer: &mut R,
        scale: Scale<f64>,
        output: &Output,
        time: Duration,
    ) -> Vec<WaylandSurfaceRenderElement<R>> {
        self.floating.render_elements(renderer, scale, output, time)
    }
}

pub struct FloatingSpace {
    tiles: Vec<Tile>,
}

impl FloatingSpace {
    pub fn render_elements<R: MyRenderer>(
        &self,
        renderer: &mut R,
        scale: Scale<f64>,
        output: &Output,
        time: Duration,
    ) -> Vec<WaylandSurfaceRenderElement<R>> {
        self.tiles
            .iter()
            .flat_map(|t| t.render_element(renderer, scale, output, time))
            .collect()
    }
}

pub struct Tile {
    window: MappedWindow,
    position: Point<i32, Logical>,
}

impl Tile {
    pub fn new<T: Into<Point<i32, Logical>>>(window: MappedWindow, position: T) -> Self {
        Self {
            window,
            position: position.into(),
        }
    }

    pub fn render_element<R: MyRenderer>(
        &self,
        renderer: &mut R,
        scale: Scale<f64>,
        output: &Output,
        time: Duration,
    ) -> Vec<WaylandSurfaceRenderElement<R>> {
        let location = self.position - self.window.geometry().loc;
        self.window.render_element(
            renderer,
            location.to_physical_precise_round(scale),
            scale,
            output,
            time,
        )
    }
}
