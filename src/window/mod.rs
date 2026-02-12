use std::{
    mem,
    sync::{Arc, Mutex, MutexGuard},
};

use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper,
        element::{AsRenderElements, surface::WaylandSurfaceRenderElement},
    },
    desktop::Window,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Scale, Size},
    wayland::{
        compositor,
        shell::xdg::{SurfaceCachedState, ToplevelSurface},
    },
};

use crate::{
    input::grabs::floating_resize_grab::{ResizeEdge, ResizeGrabState},
    monitor::{TileTreeSearchKey, TileTreeWindow},
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
                location: (0, 0).into(),
                data: MappedWindowData {
                    window,
                    dirty: false,
                    focus,
                    floating,
                    opacity: 1.,
                    resize_state: ResizeGrabState::default(),
                },
            })),
        }
    }

    fn inner(&self) -> MutexGuard<'_, MappedWindowInner> {
        self.inner.lock().expect("Mapped window lock panic")
    }

    pub fn window(&self) -> Window {
        self.inner().data.window.clone()
    }

    pub fn toplevel(&self) -> ToplevelSurface {
        self.window().toplevel().expect("No X11 support").clone()
    }

    pub fn wl_surface(&self) -> WlSurface {
        self.toplevel().wl_surface().clone()
    }

    pub fn get_dirty(&self) -> bool {
        self.inner().data.dirty
    }

    pub fn set_dirty(&self, dirty: bool) {
        self.inner().data.dirty = dirty;
    }

    pub fn get_focus(&self) -> bool {
        self.inner().data.focus
    }

    pub fn set_focus(&self, focus: bool) {
        self.inner().data.focus = focus;
    }

    pub fn get_floating(&self) -> bool {
        self.inner().data.floating
    }

    pub fn set_floating(&self, floating: bool) {
        self.inner().data.floating = floating;
    }

    pub fn get_opacity(&self) -> f32 {
        self.inner().data.opacity
    }

    pub fn set_opacity(&self, opacity: f32) {
        self.inner().data.opacity = opacity;
    }

    pub fn set_resize_state(&self, state: ResizeGrabState) {
        self.inner().data.resize_state = state;
    }

    pub fn center_location(&self) -> Point<i32, Logical> {
        let location = self.inner().location;
        let size = self.get_size();
        Point::new(size.w / 2 + location.x, size.h / 2 + location.y)
    }

    pub fn render_location(&self) -> Point<i32, Logical> {
        let location = self.inner().location;
        let loc = self.window().geometry().loc;
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
        let opacity = self.get_opacity();
        self.window()
            .render_elements(renderer, location, scale, opacity)
    }
}

impl PartialEq for MappedWindow {
    fn eq(&self, other: &Self) -> bool {
        self.window() == other.window()
    }
}

impl TileTreeWindow for MappedWindow {
    fn match_id(&self, #[allow(unused_variables)] key: TileTreeSearchKey) -> bool {
        #[cfg(test)]
        {
            false
        }
        #[cfg(not(test))]
        {
            match key {
                TileTreeSearchKey::WlSurface(surface) => self.wl_surface() == *surface,
            }
        }
    }

    fn get_location(&self) -> Point<i32, Logical> {
        self.inner().location
    }

    fn get_size(&self) -> Size<i32, Logical> {
        self.window().geometry().size
    }

    fn set_location(&mut self, location: Point<i32, Logical>) {
        self.inner().location = location;
    }

    fn set_size(&mut self, size: Size<i32, Logical>) {
        let (min_size, max_size) = compositor::with_states(&self.wl_surface(), |states| {
            let mut guard = states.cached_state.get::<SurfaceCachedState>();
            let data = guard.current();
            (data.min_size, data.max_size)
        });

        let min_width = min_size.w.max(1);
        let min_height = min_size.h.max(1);

        let max_width = if max_size.w == 0 {
            i32::MAX
        } else {
            max_size.w
        };
        let max_height = if max_size.h == 0 {
            i32::MAX
        } else {
            max_size.h
        };

        self.toplevel().with_pending_state(|state| {
            state.size = Some(
                (
                    size.w.clamp(min_width, max_width),
                    size.h.clamp(min_height, max_height),
                )
                    .into(),
            );
        });
        self.set_dirty(true);
    }

    fn swap(&mut self, other: &mut Self) {
        let temp = self.get_size();
        self.set_size(other.get_size());
        other.set_size(temp);
        mem::swap(&mut self.inner().data, &mut other.inner().data);
    }
}

#[derive(Debug)]
pub struct MappedWindowInner {
    location: Point<i32, Logical>,
    data: MappedWindowData,
}

#[derive(Debug, Clone)]
pub struct MappedWindowData {
    window: Window,
    dirty: bool,
    focus: bool,
    floating: bool,
    opacity: f32,
    resize_state: ResizeGrabState,
}

impl MappedWindow {
    pub fn update_window(&mut self) {
        if self.inner().data.resize_state != ResizeGrabState::Idle {
            let mut location = self.get_location();
            let geometry = self.window().geometry();

            let mut new_x = None;
            let mut new_y = None;

            if let Some((edges, initial_rect)) = self.inner().data.resize_state.commit()
                && edges.intersects(ResizeEdge::TOP_LEFT)
            {
                if edges.intersects(ResizeEdge::LEFT) {
                    new_x = Some(initial_rect.loc.x + (initial_rect.size.w - geometry.size.w))
                };
                if edges.intersects(ResizeEdge::TOP) {
                    new_y = Some(initial_rect.loc.y + (initial_rect.size.h - geometry.size.h))
                };
            }

            if let Some(x) = new_x {
                location.x = x;
            }
            if let Some(y) = new_y {
                location.y = y;
            }

            self.set_location(location);
        }
    }
}
