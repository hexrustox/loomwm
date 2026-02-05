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
    window::rule::WindowFloat,
};

pub mod rule;

pub struct UnmappedWindow {
    pub inner: Window,
    pub state: UnmappedWindowConfigurationState,
}

impl UnmappedWindow {
    pub fn new(window: Window) -> Self {
        Self {
            inner: window,
            state: UnmappedWindowConfigurationState::NotConfigured,
        }
    }

    pub fn toplevel(&self) -> &ToplevelSurface {
        self.inner.toplevel().expect("No X11 support")
    }

    pub fn configured(&self) -> bool {
        matches!(
            self.state,
            UnmappedWindowConfigurationState::Configured { .. }
        )
    }
}

pub enum UnmappedWindowConfigurationState {
    Configured {
        focus: bool,
        floating: Option<WindowFloat>,
        workspace: Option<u8>,
    },
    NotConfigured,
}

#[derive(Debug)]
pub struct MappedWindow {
    pub inner: Window,
    pub location: Point<i32, Logical>,
    pub floating: bool,
}

impl MappedWindow {
    pub fn new(window: Window, floating: bool) -> Self {
        Self {
            inner: window,
            location: (0, 0).into(),
            floating,
        }
    }

    pub fn toplevel(&self) -> &ToplevelSurface {
        self.inner.toplevel().expect("No X11 support")
    }

    pub fn render_location(&self) -> Point<i32, Logical> {
        self.location - self.inner.geometry().loc
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

    pub fn center_location(&self) -> Point<i32, Logical> {
        let size = self.inner.geometry().size;
        Point::new(size.w / 2 + self.location.x, size.h / 2 + self.location.y)
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
        // TODO
        self.toplevel().with_pending_state(|state| {
            // println!("{size:?}");
            // state.size = Some(size);
            // state.size = Some((887, 1094).into());
        });
        self.toplevel().send_pending_configure();
    }
}
