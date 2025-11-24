use std::time::Duration;

use smithay::{
    backend::renderer::element::{AsRenderElements, surface::WaylandSurfaceRenderElement},
    desktop::Window,
    output::Output,
    utils::{Logical, Physical, Point, Rectangle, Scale},
    wayland::shell::xdg::ToplevelSurface,
};

use crate::types::MyRenderer;

#[derive(Debug)]
pub struct MappedWindow {
    pub window: Window,
    is_focused: bool,
}

impl MappedWindow {
    pub fn new(window: Window) -> Self {
        Self {
            window,
            is_focused: false,
        }
    }

    pub fn toplevel(&self) -> &ToplevelSurface {
        self.window.toplevel().expect("No X11 support")
    }

    pub fn geometry(&self) -> Rectangle<i32, Logical> {
        self.window.geometry()
    }

    pub fn render_element<R: MyRenderer>(
        &self,
        renderer: &mut R,
        location: Point<i32, Physical>,
        scale: Scale<f64>,
        output: &Output,
        time: Duration,
    ) -> Vec<WaylandSurfaceRenderElement<R>> {
        let elements = self.window.render_elements(renderer, location, scale, 1.0);
        self.window
            .send_frame(output, time, None, |_, _| Some(output.clone()));
        elements
    }
}
