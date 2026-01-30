use std::{collections::HashMap, rc::Rc};

use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper, element::surface::WaylandSurfaceRenderElement,
    },
    desktop::{Window, space::SpaceElement},
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Scale},
};

use crate::{
    window::MappedWindow,
    workspace::tile::{TileTree, test_layout_set},
};

mod tile;

pub use tile::{TileTreeWindow, TileTreeWindowId};

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

    pub fn window_lookup(&self, surface: &WlSurface) -> Option<&MappedWindow> {
        for workspace in self.workspaces.values() {
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
                    tiling: TileTree::new(Rc::new(test_layout_set()), "master"),
                    floating: Vec::new(),
                },
            )]),
        }
    }
}

#[derive(Debug)]
pub struct Workspace {
    tiling: TileTree,
    floating: Vec<MappedWindow>,
}

impl Workspace {
    pub fn new_window(&mut self, mapped: MappedWindow) {
        self.tiling.insert(mapped);
        // self.floating.insert(0, mapped);
    }

    pub fn render_elements<R: Renderer + ImportAll>(
        &mut self,
        output: &Output,
        renderer: &mut R,
        scale: Scale<f64>,
    ) -> Vec<WaylandSurfaceRenderElement<R>>
    where
        <R as RendererSuper>::TextureId: Clone + 'static,
    {
        self.tiling.update_toplevel_state(
            output.current_location(),
            output
                .current_mode()
                .unwrap()
                .size
                .to_logical(output.current_scale().integer_scale()),
        );
        self.windows_iter()
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

    pub fn windows_iter(&self) -> Box<dyn Iterator<Item = &MappedWindow> + '_> {
        Box::new(self.floating.iter().chain(self.tiling.windows()))
    }

    pub fn window_lookup(&self, surface: &WlSurface) -> Option<&MappedWindow> {
        self.windows_iter()
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
