use std::collections::HashMap;

use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper, element::surface::WaylandSurfaceRenderElement,
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::Scale,
};

use crate::window::MappedWindow;

pub struct Workspaces {
    active: String,
    workspaces: HashMap<String, Workspace>,
}

impl Workspaces {
    pub fn get_active(&mut self) -> &mut Workspace {
        self.workspaces
            .get_mut(&self.active)
            .expect("No active workspace")
    }
}

const DEFAULT_NAME: &str = "1";

impl Default for Workspaces {
    fn default() -> Self {
        Self {
            active: DEFAULT_NAME.to_string(),
            workspaces: HashMap::from_iter([(
                DEFAULT_NAME.to_string(),
                Workspace {
                    floating: Vec::new(),
                },
            )]),
        }
    }
}

pub struct Workspace {
    floating: Vec<WlSurface>,
}

impl Workspace {
    pub fn new_window(&mut self, wl_surface: WlSurface) {
        self.floating.push(wl_surface);
    }

    pub fn render_elements<R: Renderer + ImportAll>(
        &self,
        mapped_windows: &HashMap<WlSurface, MappedWindow>,
        renderer: &mut R,
        scale: Scale<f64>,
    ) -> Vec<WaylandSurfaceRenderElement<R>>
    where
        <R as RendererSuper>::TextureId: Clone + 'static,
    {
        self.floating
            .iter()
            .flat_map(|w| {
                let m = mapped_windows.get(w).unwrap();
                m.render_elements::<R>(renderer, scale)
            })
            .collect()
    }
}
