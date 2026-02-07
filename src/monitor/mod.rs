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
    monitor::workspace::Workspace,
    state::WaylandState,
    window::{
        MappedWindow,
        rule::{WindowLocation, WindowProperties},
    },
};

pub use workspace::{LayoutSet, TileTreeWindow, TileTreeWindowId, WorkspaceName, test_layout_set};

mod workspace;

#[derive(Default)]
pub struct Monitors {
    monitors: Vec<Monitor>,
}

impl Monitors {
    pub fn push(&mut self, output: Output, layouts: Rc<LayoutSet>, layout_name: Rc<str>) {
        self.monitors
            .push(Monitor::new(output, layouts, layout_name));
    }

    // TODO
    pub fn get_monitor(&self) -> &Monitor {
        self.monitors.last().unwrap()
    }
    pub fn get_monitor_mut(&mut self) -> &mut Monitor {
        self.monitors.last_mut().unwrap()
    }
}

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
            .binary_search_by_key(name, |workspace| workspace.get_name())
            .ok()
    }

    pub fn get_workspace(&self, name: &WorkspaceName) -> &Workspace {
        if let Some(index) = self.find_workspace(name) {
            return &self.workspaces[index];
        }
        // TEMP
        unreachable!()
    }

    pub fn get_workspace_mut(&mut self, name: &WorkspaceName) -> &mut Workspace {
        if let Some(index) = self.find_workspace(name) {
            return &mut self.workspaces[index];
        }
        // TEMP
        unreachable!()
    }

    fn add_workspace(&mut self, name: WorkspaceName) {
        match name {
            WorkspaceName::Id(id) => {
                if let Err(index) = self.workspaces.binary_search_by_key(&id, |workspace| {
                    let WorkspaceName::Id(id) = workspace.get_name();
                    id
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
    pub fn add_window(
        &mut self,
        window: Window,
        WindowProperties {
            decoration,
            focus,
            float,
            workspace,
        }: WindowProperties,
    ) {
        let focus = focus.unwrap_or(true);
        let mut mapped = MappedWindow::new(window, focus, float.is_some());

        if let Some(decoration) = decoration {
            mapped.toplevel().with_pending_state(|state| {
                state.decoration_mode = Some(decoration.into());
            });
        }

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
        let workspace = &mut self.monitors.get_monitor_mut().get_workspace_mut(&name);

        if let Some(float) = float {
            match float.location {
                Some(WindowLocation::Location(x, y)) => {
                    mapped.location = (x, y).into();
                }
                Some(WindowLocation::Center) => {
                    let output_size = &workspace.output_size();
                    let window_size = float
                        .size
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
            let size = float
                .size
                .map(|(w, h)| (w, h).into())
                .unwrap_or(mapped.window.geometry().size);
            mapped.toplevel().with_pending_state(|state| {
                state.size = Some(size);
            });

            mapped.toplevel().send_pending_configure();
            workspace.add_floating_window(mapped);
        } else {
            mapped.toplevel().send_pending_configure();
            workspace.add_tiling_window(mapped);
        }

        if let Some(surface) = surface {
            self.focus_window(&surface);
        }
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
                    .map(|mapped| (mapped, workspace.get_name()))
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
                let name = workspace.get_name();
                workspace
                    .find_window_mut(surface)
                    .map(|mapped| (mapped, name))
            })
    }

    pub fn remove_mapped_window(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        self.monitors
            .get_monitor_mut()
            .workspaces
            .iter_mut()
            .find_map(|w| w.remove_window(surface))
    }

    pub fn focus_window(&mut self, surface: &WlSurface) {
        let keyboard = self.seat.get_keyboard().unwrap();

        if let Some(prev_surface) = keyboard.current_focus() {
            if prev_surface == *surface {
                return;
            }
            if let Some((mapped, ..)) = self.find_mapped_window_mut(&prev_surface) {
                mapped.focus = false;

                mapped.window.set_activated(false);
                mapped.toplevel().send_pending_configure();
            }
        }

        keyboard.set_focus(self, Some(surface.clone()), SERIAL_COUNTER.next_serial());
        let Some((mapped, name)) = self.find_mapped_window_mut(surface) else {
            return;
        };
        mapped.focus = true;

        mapped.window.set_activated(true);
        mapped.toplevel().send_pending_configure();

        let floating = mapped.floating;
        let window = mapped.window.clone();
        let monitor = self.monitors.get_monitor_mut();
        let workspace = monitor.get_workspace_mut(&name);
        workspace.focus_queue_insert(window);
        if floating {
            workspace.raise_floating_window(surface);
        }
        monitor.active_workspace = name;
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
        &mut self,
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

    pub fn focus_workspace(&mut self, name: WorkspaceName) {
        let monitor = self.monitors.get_monitor_mut();
        monitor.add_workspace(name.clone());
        monitor.active_workspace = name.clone();
        if let Some(window) = monitor.get_workspace(&name).last_focus_window() {
            let surface = window.toplevel().unwrap().wl_surface().clone();
            self.focus_window(&surface);
        }
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
