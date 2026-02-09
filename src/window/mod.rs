use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper,
        element::{AsRenderElements, surface::WaylandSurfaceRenderElement},
    },
    desktop::Window,
    utils::{Logical, Point, Scale, Size},
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

// TODO
#[derive(Debug)]
pub struct MappedWindow {
    pub window: Window,
    pub focus: bool,
    pub floating: bool,
    pub location: Point<i32, Logical>,
    pub opacity: f32,
}

impl MappedWindow {
    pub fn new(window: Window, focus: bool) -> Self {
        Self {
            window,
            focus,
            floating: false,
            location: (0, 0).into(),
            opacity: 1.,
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
        self.window
            .render_elements(renderer, location, scale, self.opacity)
    }

    pub fn center_location(&self) -> Point<i32, Logical> {
        let size = self.window.geometry().size;
        Point::new(size.w / 2 + self.location.x, size.h / 2 + self.location.y)
    }
}

impl TileTreeWindow for MappedWindow {
    type Inner = Window;

    fn match_id(&self, id: TileTreeWindowId) -> bool {
        match id {
            TileTreeWindowId::WlSurface(surface) => self.toplevel().wl_surface() == surface,
            _ => false,
        }
    }

    fn get_location(&self) -> Point<i32, Logical> {
        self.location
    }

    fn get_size(&self) -> Size<i32, Logical> {
        self.window.geometry().size
    }

    fn set_location(&mut self, location: Point<i32, Logical>) {
        self.location = location;
    }

    fn set_size(&mut self, size: Size<i32, Logical>) {
        self.toplevel().with_pending_state(|state| {
            state.size = Some(size);
        });
        self.toplevel().send_pending_configure();
    }

    fn get_inner(&self) -> Self::Inner {
        self.window.clone()
    }

    fn set_inner(&mut self, inner: Self::Inner) {
        self.window = inner;
    }
}
