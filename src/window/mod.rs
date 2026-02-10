use std::{cell::RefCell, rc::Rc};

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

type Inner = Rc<RefCell<MappedWindowInner>>;

#[derive(Debug, Clone)]
pub struct MappedWindow {
    inner: Inner,
}

impl MappedWindow {
    pub fn new(window: Window, focus: bool, floating: bool) -> Self {
        Self {
            inner: Rc::new(RefCell::new(MappedWindowInner {
                window,
                focus,
                floating,
                location: (0, 0).into(),
                opacity: 1.,
            })),
        }
    }

    pub fn get_focus(&self) -> bool {
        self.inner.borrow().focus
    }

    pub fn set_focus(&self, value: bool) {
        self.inner.borrow_mut().focus = value;
    }

    pub fn get_floating(&self) -> bool {
        self.inner.borrow().floating
    }

    pub fn set_floating(&self, value: bool) {
        self.inner.borrow_mut().floating = value;
    }

    pub fn get_opacity(&self) -> f32 {
        self.inner.borrow().opacity
    }

    pub fn set_opacity(&self, value: f32) {
        self.inner.borrow_mut().opacity = value;
    }

    pub fn window(&self) -> Window {
        self.inner.borrow().window.clone()
    }

    pub fn toplevel(&self) -> ToplevelSurface {
        self.inner
            .borrow()
            .window
            .toplevel()
            .expect("No X11 support")
            .clone()
    }

    pub fn wl_surface(&self) -> WlSurface {
        self.toplevel().wl_surface().clone()
    }

    pub fn center_location(&self) -> Point<i32, Logical> {
        let location = self.inner.borrow().location;
        let size = self.inner.borrow().window.geometry().size;
        Point::new(size.w / 2 + location.x, size.h / 2 + location.y)
    }

    pub fn render_location(&self) -> Point<i32, Logical> {
        self.inner.borrow().location - self.inner.borrow().window.geometry().loc
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
        self.inner.borrow().window.render_elements(
            renderer,
            location,
            scale,
            self.inner.borrow().opacity,
        )
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
        self.inner.borrow().location
    }

    fn get_size(&self) -> Size<i32, Logical> {
        self.inner.borrow().window.geometry().size
    }

    fn set_location(&mut self, location: Point<i32, Logical>) {
        self.inner.borrow_mut().location = location;
    }

    fn set_size(&mut self, size: Size<i32, Logical>) {
        self.toplevel().with_pending_state(|state| {
            state.size = Some(size);
        });
        self.toplevel().send_pending_configure();
    }

    fn get_buffer(&self) -> Self::Buffer {
        self.inner.borrow().window.clone()
    }

    fn set_buffer(&mut self, buffer: Self::Buffer) {
        self.inner.borrow_mut().window = buffer;
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
