use std::collections::HashMap;

use smithay::{
    backend::renderer::{
        ImportAll, Renderer, Texture,
        element::{AsRenderElements, surface::WaylandSurfaceRenderElement},
    },
    desktop::{Window, space::SpaceElement},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Scale},
    wayland::shell::xdg::ToplevelSurface,
};

#[derive(Default)]
pub struct WindowRecord {
    pub mapped_windows: HashMap<WlSurface, MappedWindow>,
    pub unmapped_windows: HashMap<WlSurface, UnmappedWindow>,
}

impl WindowRecord {
    pub fn new_window(&mut self, window: Window) {
        let key = window.toplevel().unwrap().wl_surface().clone();
        self.unmapped_windows
            .insert(key, UnmappedWindow::new(window));
    }

    pub fn window_under<T: Into<Point<f64, Logical>>>(
        &self,
        point: T,
    ) -> Option<(&Window, Point<i32, Logical>)> {
        let point = point.into();
        self.mapped_windows
            .iter()
            .filter(|(_, w)| w.inner.bbox().to_f64().contains(point))
            .find_map(|(_, e)| {
                // we need to offset the point to the location where the surface is actually drawn
                let render_location = e.render_location();
                if e.inner
                    .is_in_input_region(&(point - render_location.to_f64()))
                {
                    Some((&e.inner, render_location))
                } else {
                    None
                }
            })
    }

    pub fn render_elements<R: Renderer + ImportAll>(
        &self,
        renderer: &mut R,
        scale: Scale<f64>,
    ) -> Vec<WaylandSurfaceRenderElement<R>>
    where
        R::TextureId: Texture + Clone + 'static,
    {
        self.mapped_windows
            .iter()
            .flat_map(|(_, w)| {
                let location = w.render_location();
                w.inner.render_elements(
                    renderer,
                    location.to_physical_precise_round(scale),
                    scale,
                    1.0,
                )
            })
            .collect()
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
    location: Point<i32, Logical>,
}

impl MappedWindow {
    pub fn new(window: Window) -> Self {
        Self {
            inner: window,
            location: (0, 0).into(),
        }
    }

    fn render_location(&self) -> Point<i32, Logical> {
        self.location - self.inner.geometry().loc
    }

    pub fn toplevel(&self) -> &ToplevelSurface {
        self.inner.toplevel().expect("No X11 support")
    }
}
