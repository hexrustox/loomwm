use std::collections::HashMap;

use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper, element::surface::WaylandSurfaceRenderElement,
    },
    desktop::{Window, space::SpaceElement},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Scale},
};

use crate::window::MappedWindow;

#[derive(Debug)]
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

    pub fn window_lookup(&mut self, surface: &WlSurface) -> Option<&mut MappedWindow> {
        for workspace in self.workspaces.values_mut() {
            let window = workspace.window_lookup(surface);
            if window.is_some() {
                return window;
            }
        }
        None
    }

    pub fn remove_window(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        for workspace in self.workspaces.values_mut() {
            let window = workspace.remove_window(surface);
            if window.is_some() {
                return window;
            }
        }
        None
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

#[derive(Debug)]
pub struct Workspace {
    floating: Vec<MappedWindow>,
}

impl Workspace {
    pub fn new_mapped_window(&mut self, mapped: MappedWindow) {
        self.floating.insert(0, mapped);
    }

    pub fn render_elements<R: Renderer + ImportAll>(
        &self,
        renderer: &mut R,
        scale: Scale<f64>,
    ) -> Vec<WaylandSurfaceRenderElement<R>>
    where
        <R as RendererSuper>::TextureId: Clone + 'static,
    {
        self.floating
            .iter()
            .flat_map(|mapped| mapped.render_elements::<R>(renderer, scale))
            .collect()
    }

    pub fn window_under<T: Into<Point<f64, Logical>>>(
        &self,
        point: T,
    ) -> Option<(&Window, Point<i32, Logical>)> {
        let point = point.into();
        self.floating.iter().find_map(|mapped| {
            let render_location = mapped.render_location();
            if mapped
                .inner
                .is_in_input_region(&(point - render_location.to_f64()))
            {
                Some((&mapped.inner, render_location))
            } else {
                None
            }
        })
    }

    pub fn windows_iter(&mut self) -> std::slice::IterMut<'_, MappedWindow> {
        self.floating.iter_mut()
    }

    pub fn window_lookup(&mut self, surface: &WlSurface) -> Option<&mut MappedWindow> {
        self.floating
            .iter_mut()
            .find(|w| w.toplevel().wl_surface() == surface)
    }

    pub fn raise_floating_window(&mut self, surface: &WlSurface) {
        if let Some(index) = self
            .floating
            .iter()
            .position(|mapped| mapped.toplevel().wl_surface() == surface)
        {
            self.floating[0..=index].rotate_right(1);
        }
    }

    pub fn remove_window(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        if let Some(index) = self
            .floating
            .iter()
            .position(|mapped| mapped.toplevel().wl_surface() == surface)
        {
            return Some(self.floating.remove(index));
        }
        None
    }
}
