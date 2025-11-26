use smithay::{
    backend::renderer::{
        ImportAll, Renderer, Texture,
        element::{AsRenderElements, surface::WaylandSurfaceRenderElement},
    },
    desktop::Window,
    utils::{Logical, Point, Scale},
};

#[derive(Default)]
pub struct WindowRecord {
    windows: Vec<MappedWindow>,
}

impl WindowRecord {
    pub fn new_window(&mut self, window: Window) {
        self.windows.push(MappedWindow::new(window));
    }

    pub fn iter(&self) -> impl Iterator<Item = &Window> {
        self.windows.iter().map(|w| &w.inner)
    }

    pub fn render_elements<R: Renderer + ImportAll>(
        &self,
        renderer: &mut R,
        scale: Scale<f64>,
    ) -> Vec<WaylandSurfaceRenderElement<R>>
    where
        R::TextureId: Texture + Clone + 'static,
    {
        self.windows
            .iter()
            .flat_map(|w| {
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

pub struct MappedWindow {
    inner: Window,
    location: Point<i32, Logical>,
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
}
