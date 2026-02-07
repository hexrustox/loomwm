use std::{cell::RefCell, rc::Rc};

use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper, element::surface::WaylandSurfaceRenderElement,
    },
    desktop::{Window, WindowSurfaceType},
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, SERIAL_COUNTER, Scale, Serial},
};

use crate::{
    monitor::workspace::Workspace,
    state::WaylandState,
    window::{
        MappedWindow,
        rule::{WindowLocation, WindowProperties},
    },
};

pub use workspace::{LayoutSet, TileTreeWindow, TileTreeWindowId, test_layout_set};

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
    fn get_monitor(&self) -> &Monitor {
        self.monitors.last().unwrap()
    }
    fn get_monitor_mut(&mut self) -> &mut Monitor {
        self.monitors.last_mut().unwrap()
    }
}

pub struct Monitor {
    output: Rc<RefCell<Output>>,

    active_workspace: u8,
    workspaces: Vec<(u8, Workspace)>,

    layouts: Rc<LayoutSet>,
    layout_name: Rc<str>,
}

impl Monitor {
    fn new(output: Output, layouts: Rc<LayoutSet>, layout_name: Rc<str>) -> Self {
        let output = Rc::new(RefCell::new(output));
        Self {
            output: output.clone(),
            active_workspace: 1,
            workspaces: vec![(1, Workspace::new(output, layouts.clone(), &layout_name))],
            layouts,
            layout_name,
        }
    }

    fn get_active_workspace(&self) -> &Workspace {
        for (name, workspace) in self.workspaces.iter() {
            if *name == self.active_workspace {
                return workspace;
            }
        }
        panic!("No active workspace")
    }

    fn get_active_workspace_mut(&mut self) -> &mut Workspace {
        for (name, workspace) in self.workspaces.iter_mut() {
            if *name == self.active_workspace {
                return workspace;
            }
        }
        panic!("No active workspace")
    }

    fn insert_workspace(&mut self, index: usize, name: u8) {
        self.workspaces.insert(
            index,
            (
                name,
                Workspace::new(self.output.clone(), self.layouts.clone(), &self.layout_name),
            ),
        );
    }

    fn add_workspace(&mut self, name: u8) -> &mut Workspace {
        let index = match self.workspaces.binary_search_by_key(&name, |(n, _)| *n) {
            Ok(i) => i,
            Err(i) => {
                self.insert_workspace(i, name);
                i
            }
        };
        &mut self.workspaces[index].1
    }

    fn switch_workspace(&mut self, name: u8) {
        self.add_workspace(name);
        self.active_workspace = name;
    }

    fn move_window_to_workspace(&mut self, surface: &WlSurface, name: u8, focus: bool) {
        let Some(mapped) = self.remove_window(surface) else {
            return;
        };

        let idx = {
            match self.workspaces.binary_search_by_key(&name, |(n, _)| *n) {
                Ok(idx) => idx,
                Err(idx) => {
                    self.insert_workspace(idx, name);
                    idx
                }
            }
        };
        self.workspaces[idx].1.add_tiling_window(mapped);
        if focus {
            self.switch_workspace(name);
        }
    }

    fn find_window(&self, surface: &WlSurface) -> Option<&MappedWindow> {
        for workspace in self.workspaces.iter().map(|(_, w)| w) {
            if let window @ Some(_) = workspace.find_window(surface) {
                return window;
            }
        }
        None
    }

    fn find_window_mut(&mut self, surface: &WlSurface) -> Option<&mut MappedWindow> {
        for workspace in self.workspaces.iter_mut().map(|(_, w)| w) {
            if let window @ Some(_) = workspace.find_window_mut(surface) {
                return window;
            }
        }
        None
    }

    fn remove_window(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        for workspace in self.workspaces.iter_mut().map(|(_, w)| w) {
            if let window @ Some(_) = workspace.remove_window(surface) {
                return window;
            }
        }
        None
    }
}

impl WaylandState {
    pub fn switch_workspace(&mut self, name: u8) {
        self.monitors.get_monitor_mut().switch_workspace(name);
    }

    pub fn move_window_to_workspace(&mut self, surface: &WlSurface, name: u8, focus: bool) {
        self.monitors
            .get_monitor_mut()
            .move_window_to_workspace(surface, name, focus);
    }

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
        let mut mapped = MappedWindow::new(window, float.is_some());

        if let Some(decoration) = decoration {
            mapped.toplevel().with_pending_state(|state| {
                state.decoration_mode = Some(decoration.into());
            });
        }

        let focus = focus.unwrap_or(true);
        let surface = if focus {
            Some(mapped.toplevel().wl_surface().clone())
        } else {
            None
        };

        let workspace = if let Some(name) = workspace {
            if focus {
                self.switch_workspace(name);
                self.monitors.get_monitor_mut().get_active_workspace_mut()
            } else {
                self.monitors.get_monitor_mut().add_workspace(name)
            }
        } else {
            self.monitors.get_monitor_mut().get_active_workspace_mut()
        };
        if let Some(float) = float {
            match float.location {
                Some(WindowLocation::Location(x, y)) => {
                    mapped.location = (x, y).into();
                }
                Some(WindowLocation::Center) => {
                    let output = workspace.output.borrow();
                    let output_size = output
                        .current_mode()
                        .unwrap()
                        .size
                        .to_logical(output.current_scale().integer_scale());
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
            self.focus_window(&surface, None);
        }
    }

    pub fn find_mapped_window(&self, surface: &WlSurface) -> Option<&MappedWindow> {
        self.monitors.get_monitor().find_window(surface)
    }

    pub fn find_mapped_window_mut(&mut self, surface: &WlSurface) -> Option<&mut MappedWindow> {
        self.monitors.get_monitor_mut().find_window_mut(surface)
    }

    pub fn remove_mapped_window(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        self.monitors.get_monitor_mut().remove_window(surface)
    }

    pub fn focus_window(&mut self, surface: &WlSurface, serial: Option<Serial>) {
        let keyboard = self.seat.get_keyboard().unwrap();
        let serial = serial.unwrap_or(SERIAL_COUNTER.next_serial());

        if let Some(surface) = keyboard.current_focus()
            && let Some(mapped) = self.find_mapped_window(&surface)
        {
            mapped.window.set_activated(false);
            mapped.toplevel().send_pending_configure();
        }
        keyboard.set_focus(self, Some(surface.clone()), serial);

        if let Some(mapped) = self
            .monitors
            .get_monitor_mut()
            .get_active_workspace_mut()
            .find_window(surface)
        {
            mapped.window.set_activated(true);
            mapped.toplevel().send_pending_configure();
        }
        self.monitors
            .get_monitor_mut()
            .get_active_workspace_mut()
            .raise_floating_window(surface);
    }

    pub fn toggle_window_floating(&mut self, surface: &WlSurface) {
        self.monitors
            .get_monitor_mut()
            .get_active_workspace_mut()
            .toggle_window_floating(surface);
    }

    pub fn mapped_window_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(&MappedWindow, Point<i32, Logical>)> {
        self.monitors
            .get_monitor()
            .get_active_workspace()
            .mapped_window_under(point)
    }

    pub fn windows_iter(&self) -> impl Iterator<Item = &MappedWindow> {
        self.monitors
            .get_monitor()
            .get_active_workspace()
            .windows_iter()
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

    pub fn render_elements<R>(
        &mut self,
        renderer: &mut R,
        scale: Scale<f64>,
    ) -> Vec<WaylandSurfaceRenderElement<R>>
    where
        R: Renderer + ImportAll,
        <R as RendererSuper>::TextureId: Clone + 'static,
    {
        self.monitors
            .get_monitor_mut()
            .get_active_workspace_mut()
            .render_elements(renderer, scale)
    }
}
