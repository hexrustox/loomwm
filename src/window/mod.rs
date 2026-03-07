use std::{
    mem,
    sync::{Arc, Mutex, MutexGuard},
};

use smithay::{
    backend::renderer::{
        RendererSuper,
        element::{
            AsRenderElements,
            solid::{SolidColorBuffer, SolidColorRenderElement},
            surface::WaylandSurfaceRenderElement,
            utils::CropRenderElement,
        },
    },
    desktop::Window,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Rectangle, Scale, Size},
    wayland::{
        compositor,
        shell::xdg::{SurfaceCachedState, ToplevelSurface},
    },
};

use crate::{
    input::grabs::floating_resize_grab::ResizeGrabState,
    monitor::{TileTreeSearchKey, TileTreeWindow},
    utils::{
        Direction,
        types::{RenderElements, Renderer},
    },
    window::rule::{RGBAColor, WindowBorder, WindowProperties},
};

pub mod rule;

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
pub enum UnmappedWindowState {
    Configured(WindowProperties),
    NotConfigured,
}

#[derive(Debug, Clone)]
pub struct MappedWindow {
    inner: Arc<Mutex<MappedWindowInner>>,
}

#[derive(Debug)]
pub struct MappedWindowInner {
    window: Window,
    location: Point<i32, Logical>,
    configured_size: Size<i32, Logical>,
    focus: bool,
    floating: bool,
    border: Option<WindowBorder>,
    opacity: f32,
    resize_state: ResizeGrabState,
}

impl MappedWindow {
    pub fn new(window: Window, focus: bool, floating: bool) -> Self {
        Self {
            inner: Arc::new(Mutex::new(MappedWindowInner {
                window,
                location: (0, 0).into(),
                configured_size: (0, 0).into(),
                focus,
                floating,
                border: None,
                opacity: 1.,
                resize_state: ResizeGrabState::default(),
            })),
        }
    }

    fn inner(&self) -> MutexGuard<'_, MappedWindowInner> {
        self.inner.lock().expect("Mapped window lock panic")
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

    pub fn get_geometry_size(&self) -> Size<i32, Logical> {
        self.window().geometry().size
    }

    pub fn clamp_size(&self, size: Size<i32, Logical>) -> Size<i32, Logical> {
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

        (
            size.w.clamp(min_width, max_width),
            size.h.clamp(min_height, max_height),
        )
            .into()
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

    fn get_border_width(&self) -> i32 {
        self.inner().border.clone().map_or(0, |b| b.width) as i32
    }

    fn get_border_color(&self) -> Option<RGBAColor> {
        self.inner().border.clone().map(|b| b.color)
    }

    pub fn set_border(&self, border: Option<WindowBorder>) {
        self.inner().border = border;
    }

    pub fn get_opacity(&self) -> f32 {
        self.inner().opacity
    }

    pub fn set_opacity(&self, opacity: f32) {
        self.inner().opacity = opacity;
    }

    pub fn set_resize_state(&self, state: ResizeGrabState) {
        self.inner().resize_state = state;
    }

    pub fn center_location(&self) -> Point<i32, Logical> {
        let location = self.inner().location;
        let size = self.get_geometry_size();
        Point::new(size.w / 2 + location.x, size.h / 2 + location.y)
    }

    pub fn render_location(&self) -> Point<i32, Logical> {
        let location = self.inner().location;
        let loc = self.window().geometry().loc;
        location - loc
    }

    fn inner_location(&self, location: Point<i32, Logical>) -> Point<i32, Logical> {
        Point::new(
            location.x + self.get_border_width(),
            location.y + self.get_border_width(),
        )
    }

    fn inner_size(&self) -> Size<i32, Logical> {
        let size = self.get_size();
        Size::new(
            0.max(size.w - 2 * self.get_border_width()),
            0.max(size.h - 2 * self.get_border_width()),
        )
    }

    pub fn render_elements<R: Renderer>(
        &self,
        renderer: &mut R,
        scale: Scale<f64>,
    ) -> Vec<RenderElements<R>>
    where
        <R as RendererSuper>::TextureId: Clone + 'static,
    {
        let location = self
            .inner_location(self.render_location())
            .to_physical_precise_round(scale);
        let opacity = self.get_opacity();

        let mut elems = self
            .window()
            .render_elements::<WaylandSurfaceRenderElement<R>>(renderer, location, scale, opacity)
            .into_iter()
            .flat_map(|elem| {
                let mut elems = Vec::new();

                if let Some(elem) = CropRenderElement::from_element(
                    elem,
                    scale,
                    Rectangle::new(
                        self.inner_location(self.get_location())
                            .to_physical_precise_round(scale),
                        self.inner_size().to_physical_precise_round(scale),
                    ),
                ) {
                    elems.push(RenderElements::Window(elem));
                }

                elems
            })
            .collect::<Vec<_>>();

        if let Some(color) = self.get_border_color() {
            elems.push(RenderElements::Border(
                SolidColorRenderElement::from_buffer(
                    &SolidColorBuffer::new(
                        self.get_size(),
                        [
                            color.r() as f32 / 255.,
                            color.g() as f32 / 255.,
                            color.b() as f32 / 255.,
                            1.,
                        ],
                    ),
                    self.get_location().to_physical_precise_round(scale),
                    scale,
                    color.a() as f32 / 255.,
                    smithay::backend::renderer::element::Kind::Unspecified,
                ),
            ));
        }

        elems
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

    fn set_location(&mut self, location: Point<i32, Logical>) {
        self.inner().location = location;
    }

    fn get_size(&self) -> Size<i32, Logical> {
        self.inner().configured_size
    }

    fn set_size(&mut self, size: Size<i32, Logical>) {
        self.inner().configured_size = size;
        self.toplevel().with_pending_state(|state| {
            state.size = Some(self.inner_size());
        });
    }

    fn swap_location_size(&mut self, other: &mut Self) {
        if self.window() == other.window() {
            return;
        }
        mem::swap(&mut self.inner().location, &mut other.inner().location);
        let temp = self.get_size();
        self.set_size(other.get_size());
        other.set_size(temp);
    }
}

impl MappedWindow {
    pub fn update_window(&mut self) {
        if self.inner().resize_state != ResizeGrabState::Idle {
            let mut location = self.get_location();
            let size = self.get_size();

            let mut new_x = None;
            let mut new_y = None;

            if let Some((direction, initial_rect)) = self.inner().resize_state.commit()
                && direction.intersects(Direction::TOP_LEFT)
            {
                if direction.intersects(Direction::LEFT) {
                    new_x = Some(initial_rect.loc.x + (initial_rect.size.w - size.w))
                };
                if direction.intersects(Direction::TOP) {
                    new_y = Some(initial_rect.loc.y + (initial_rect.size.h - size.h))
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
