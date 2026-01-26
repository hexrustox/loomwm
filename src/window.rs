use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper,
        element::{AsRenderElements, surface::WaylandSurfaceRenderElement},
    },
    desktop::Window,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Scale},
    wayland::shell::xdg::ToplevelSurface,
};

use crate::state::WaylandState;

impl WaylandState {
    pub fn new_window(&mut self, window: Window) {
        let key = window.toplevel().unwrap().wl_surface().clone();
        self.unmapped_windows
            .insert(key, UnmappedWindow::new(window));
    }

    pub fn mapped_window_lookup(&mut self, wl_surface: &WlSurface) -> Option<&mut MappedWindow> {
        self.workspaces.window_lookup(wl_surface)
    }
}

pub struct UnmappedWindow {
    pub inner: Window,
    pub state: UnmappedWindowConfigurationState,
}

impl UnmappedWindow {
    fn new(window: Window) -> Self {
        Self {
            inner: window,
            state: UnmappedWindowConfigurationState::NotConfigured,
        }
    }

    pub fn toplevel(&self) -> &ToplevelSurface {
        self.inner.toplevel().expect("No X11 support")
    }

    pub fn configured(&self) -> bool {
        matches!(self.state, UnmappedWindowConfigurationState::Configured)
    }
}

pub enum UnmappedWindowConfigurationState {
    Configured,
    NotConfigured,
}

#[derive(Debug)]
pub struct MappedWindow {
    pub inner: Window,
    pub location: Point<i32, Logical>,
}

impl MappedWindow {
    pub fn new(window: Window) -> Self {
        Self {
            inner: window,
            location: (0, 0).into(),
        }
    }

    pub fn render_location(&self) -> Point<i32, Logical> {
        self.location - self.inner.geometry().loc
    }

    pub fn toplevel(&self) -> &ToplevelSurface {
        self.inner.toplevel().expect("No X11 support")
    }

    pub fn render_elements<R: Renderer + ImportAll>(
        &self,
        renderer: &mut R,
        scale: Scale<f64>,
    ) -> Vec<WaylandSurfaceRenderElement<R>>
    where
        <R as RendererSuper>::TextureId: std::clone::Clone + 'static,
    {
        let loc = self.render_location().to_physical_precise_round(scale);
        self.inner.render_elements(renderer, loc, scale, 1.0)
    }
}
