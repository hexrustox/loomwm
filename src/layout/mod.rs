use std::{mem, time::Duration};

use smithay::{
    backend::renderer::{
        ImportAll, Renderer, Texture,
        element::{AsRenderElements, surface::WaylandSurfaceRenderElement},
    },
    output::Output,
    utils::{Logical, Point, Scale},
};

use crate::window::mapped::MappedWindow;

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
            },
        }
    }

    pub fn add_window(&mut self, window: MappedWindow) {
        match &mut self.monitor_set {
            MonitorSet::Normal { monitors } => {
                let ws = &mut monitors[0].workspaces;
                if ws.is_empty() {
                    ws.push(Workspace::new());
                }
                ws[0].floating.tiles.push(Tile {
                    window,
                    position: Point::new(0, 0),
                });
            }
            MonitorSet::NoOutputs { workspaces } => todo!(),
        }
    }

    pub fn render_elements<R: Renderer + ImportAll>(
        &self,
        renderer: &mut R,
        scale: Scale<f64>,
        alpha: f32,
        output: &Output,
        time: Duration,
    ) -> Vec<WaylandSurfaceRenderElement<R>>
    where
        R::TextureId: Clone + Texture + 'static,
    {
        let mut vec = Vec::new();

        match &self.monitor_set {
            MonitorSet::Normal { monitors } => {
                let ws = &monitors[0].workspaces;
                if !ws.is_empty() {
                    for t in &ws[0].floating.tiles {
                        let location = t.position - t.window.window.geometry().loc;
                        vec.extend(t.window.window.render_elements(
                            renderer,
                            location.to_physical_precise_round(scale),
                            scale,
                            alpha,
                        ));
                        t.window
                            .window
                            .send_frame(output, time, None, |_, _| Some(output.clone()));
                    }
                }
            }
            MonitorSet::NoOutputs { workspaces } => {}
        }

        vec
    }
}

enum MonitorSet {
    Normal { monitors: Vec<Monitor> },
    NoOutputs { workspaces: Vec<Workspace> },
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
}

pub struct FloatingSpace {
    tiles: Vec<Tile>,
}

pub struct Tile {
    window: MappedWindow,
    position: Point<i32, Logical>,
}
