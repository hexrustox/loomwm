use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper,
        element::{AsRenderElements, surface::WaylandSurfaceRenderElement},
    },
    desktop::Window,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, SERIAL_COUNTER, Scale, Serial},
    wayland::shell::xdg::ToplevelSurface,
};

use crate::{
    state::WaylandState,
    workspace::{TileTreeWindow, TileTreeWindowId},
};

impl WaylandState {
    pub fn new_window(&mut self, window: Window) {
        let key = window.toplevel().unwrap().wl_surface().clone();
        self.unmapped_windows
            .insert(key, UnmappedWindow::new(window));
    }

    pub fn mapped_window_lookup(&self, surface: &WlSurface) -> Option<&MappedWindow> {
        self.workspaces.window_lookup(surface)
    }

    pub fn remove_mapped_window(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        self.workspaces.remove_window(surface)
    }

    pub fn focus_window(&mut self, surface: &WlSurface, serial: Option<Serial>) {
        let keyboard = self.seat.get_keyboard().unwrap();
        let serial = serial.unwrap_or(SERIAL_COUNTER.next_serial());

        if let Some(surface) = keyboard.current_focus()
            && let Some(mapped) = self.mapped_window_lookup(&surface)
        {
            mapped.inner.set_activated(false);
            mapped.toplevel().send_pending_configure();
        }
        keyboard.set_focus(self, Some(surface.clone()), serial);

        if let Some(mapped) = self.workspaces.get_active().window_lookup(surface) {
            mapped.inner.set_activated(true);
            mapped.toplevel().send_pending_configure();
        }
        self.workspaces.get_active().raise_floating_window(surface);
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

impl TileTreeWindow for MappedWindow {
    fn match_id(&self, id: TileTreeWindowId) -> bool {
        match id {
            TileTreeWindowId::WlSurface(wl_surface) => self.toplevel().wl_surface() == wl_surface,
            _ => false,
        }
    }

    fn update_location(&mut self, location: Point<i32, Logical>) {
        self.location = location;
    }

    fn update_size(&mut self, size: smithay::utils::Size<i32, Logical>) {
        self.toplevel().with_pending_state(|state| {
            state.size = Some(size);
        });
        self.toplevel().send_configure();
    }
}
