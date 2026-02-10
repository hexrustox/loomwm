use std::sync::{Arc, Mutex, MutexGuard};

use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper,
        element::{AsRenderElements, surface::WaylandSurfaceRenderElement},
    },
    desktop::Window,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
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

#[derive(Debug, Clone)]
pub struct MappedWindow {
    inner: Arc<Mutex<MappedWindowInner>>,
}

impl MappedWindow {
    pub fn new(window: Window, focus: bool, floating: bool) -> Self {
        Self {
            inner: Arc::new(Mutex::new(MappedWindowInner {
                window,
                focus,
                floating,
                location: (0, 0).into(),
                opacity: 1.,
            })),
        }
    }

    fn inner(&self) -> MutexGuard<'_, MappedWindowInner> {
        self.inner.lock().expect("Mapped window lock panic")
    }

    pub fn get_focus(&self) -> bool {
        self.inner().focus
    }

    pub fn set_focus(&self, focus: bool) {
        self.inner().focus = focus;
    }

    pub fn get_floating(&self) -> bool {
        self.inner().floating
    }

    pub fn set_floating(&self, floating: bool) {
        self.inner().floating = floating;
    }

    pub fn get_opacity(&self) -> f32 {
        self.inner().opacity
    }

    pub fn set_opacity(&self, opacity: f32) {
        self.inner().opacity = opacity;
    }

    pub fn window(&self) -> Window {
        self.inner().window.clone()
    }

    pub fn toplevel(&self) -> ToplevelSurface {
        self.window().toplevel().expect("No X11 support").clone()
    }

    pub fn wl_surface(&self) -> WlSurface {
        self.toplevel().wl_surface().clone()
    }

    pub fn center_location(&self) -> Point<i32, Logical> {
        let inner = self.inner();
        let location = inner.location;
        let size = inner.window.geometry().size;
        Point::new(size.w / 2 + location.x, size.h / 2 + location.y)
    }

    pub fn render_location(&self) -> Point<i32, Logical> {
        let inner = self.inner();
        let location = inner.location;
        let loc = inner.window.geometry().loc;
        location - loc
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
        let opacity = self.inner().opacity;
        self.inner()
            .window
            .render_elements(renderer, location, scale, opacity)
    }
}

impl PartialEq for MappedWindow {
    fn eq(&self, other: &Self) -> bool {
        self.window() == other.window()
    }
}

impl TileTreeWindow for MappedWindow {
    type Buffer = Window;

    fn match_id(&self, id: TileTreeWindowId) -> bool {
        match id {
            TileTreeWindowId::WlSurface(surface) => self.toplevel().wl_surface() == surface,
            _ => false,
        }
    }

    fn get_location(&self) -> Point<i32, Logical> {
        self.inner().location
    }

    fn get_size(&self) -> Size<i32, Logical> {
        self.inner().window.geometry().size
    }

    fn set_location(&mut self, location: Point<i32, Logical>) {
        self.inner().location = location;
    }

    fn set_size(&mut self, size: Size<i32, Logical>) {
        self.toplevel().with_pending_state(|state| {
            state.size = Some(size);
        });
        self.toplevel().send_pending_configure();
    }

    fn get_buffer(&self) -> Self::Buffer {
        self.inner().window.clone()
    }

    fn set_buffer(&mut self, buffer: Self::Buffer) {
        self.inner().window = buffer;
    }
}

#[derive(Debug)]
pub struct MappedWindowInner {
    window: Window,
    focus: bool,
    floating: bool,
    location: Point<i32, Logical>,
    opacity: f32,
}
