use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper,
        element::{AsRenderElements, surface::WaylandSurfaceRenderElement},
    },
    desktop::Window,
    utils::{Logical, Point, Scale},
    wayland::shell::xdg::ToplevelSurface,
};

use crate::{
    monitor::{TileTreeWindow, TileTreeWindowId},
    window::rule::WindowProperties,
};

pub mod rule;

pub struct UnmappedWindow {
    pub window: Window,
    pub state: UnmappedWindowState,
}

impl UnmappedWindow {
    pub fn new(window: Window) -> Self {
        Self {
            window,
            state: UnmappedWindowState::NotConfigured,
        }
    }

    pub fn toplevel(&self) -> &ToplevelSurface {
        self.window.toplevel().expect("No X11 support")
    }

    pub fn configured(&self) -> bool {
        matches!(self.state, UnmappedWindowState::Configured { .. })
    }
}

pub enum UnmappedWindowState {
    Configured(WindowProperties),
    NotConfigured,
}

#[derive(Debug)]
pub struct MappedWindow {
    pub window: Window,
    pub focus: bool,
    pub floating: bool,
    pub location: Point<i32, Logical>,
    pub render: bool,
}

impl MappedWindow {
    pub fn new(window: Window, focus: bool, floating: bool) -> Self {
        Self {
            window,
            focus,
            floating,
            location: (0, 0).into(),
            render: false,
        }
    }

    pub fn toplevel(&self) -> &ToplevelSurface {
        self.window.toplevel().expect("No X11 support")
    }

    pub fn render_location(&self) -> Point<i32, Logical> {
        self.location - self.window.geometry().loc
    }

    pub fn render_elements<R: Renderer + ImportAll>(
        &self,
        renderer: &mut R,
        scale: Scale<f64>,
    ) -> Vec<WaylandSurfaceRenderElement<R>>
    where
        <R as RendererSuper>::TextureId: std::clone::Clone + 'static,
    {
        let location = self.render_location().to_physical_precise_round(scale);
        self.window.render_elements(renderer, location, scale, 1.0)
    }

    pub fn center_location(&self) -> Point<i32, Logical> {
        let size = self.window.geometry().size;
        Point::new(size.w / 2 + self.location.x, size.h / 2 + self.location.y)
    }
}

impl TileTreeWindow for MappedWindow {
    fn match_id(&self, id: TileTreeWindowId) -> bool {
        match id {
            TileTreeWindowId::WlSurface(surface) => self.toplevel().wl_surface() == surface,
            _ => false,
        }
    }

    fn update_location(&mut self, location: Point<i32, Logical>) {
        self.location = location;
    }

    fn update_size(&mut self, size: smithay::utils::Size<i32, Logical>) {
        // TODO
        self.toplevel().with_pending_state(|state| {
            state.size = Some(size);
        });
        self.toplevel().send_pending_configure();
    }
}
