use std::{collections::HashMap, rc::Rc};

use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper, element::surface::WaylandSurfaceRenderElement,
    },
    desktop::space::SpaceElement,
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Scale},
};

use crate::{window::MappedWindow, workspace::tile::TileTree};

mod tile;

pub use tile::{LayoutSet, TileTreeWindow, TileTreeWindowId, test_layout_set};

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

    pub fn find_window(&self, surface: &WlSurface) -> Option<&MappedWindow> {
        for workspace in self.workspaces.values() {
            if let window @ Some(_) = workspace.find_window(surface) {
                return window;
            }
        }
        None
    }

    pub fn find_window_mut(&mut self, surface: &WlSurface) -> Option<&mut MappedWindow> {
        for workspace in self.workspaces.values_mut() {
            if let window @ Some(_) = workspace.find_window_mut(surface) {
                return window;
            }
        }
        None
    }

    pub fn remove_window(&mut self, output: &Output, surface: &WlSurface) -> Option<MappedWindow> {
        for workspace in self.workspaces.values_mut() {
            if let window @ Some(_) = workspace.remove_window(output, surface) {
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
    pub fn add_window(&mut self, output: &Output, mapped: MappedWindow) {
        // if let Some(mapped) = self.tiling.insert(mapped) {
        self.floating.insert(0, mapped);
        // } else {
        //     self.tiling.update_toplevel_state(
        //         output.current_location(),
        //         output
        //             .current_mode()
        //             .unwrap()
        //             .size
        //             .to_logical(output.current_scale().integer_scale()),
        //     );
        // }
    }

    pub fn render_elements<R: Renderer + ImportAll>(
        &mut self,
        renderer: &mut R,
        scale: Scale<f64>,
    ) -> Vec<WaylandSurfaceRenderElement<R>>
    where
        <R as RendererSuper>::TextureId: Clone + 'static,
    {
        self.windows_iter()
            .flat_map(|mapped| mapped.render_elements::<R>(renderer, scale))
            .collect()
    }

    pub fn mapped_window_under<T: Into<Point<f64, Logical>>>(
        &self,
        point: T,
    ) -> Option<(&MappedWindow, Point<i32, Logical>)> {
        let point = point.into();
        self.windows_iter().find_map(|mapped| {
            let render_location = mapped.render_location();
            if mapped
                .inner
                .is_in_input_region(&(point - render_location.to_f64()))
            {
                Some((mapped, render_location))
            } else {
                None
            }
        })
    }

    pub fn windows_iter(&self) -> Box<dyn Iterator<Item = &MappedWindow> + '_> {
        Box::new(self.floating.iter().chain(self.tiling.windows_iter()))
    }

    pub fn find_window(&self, surface: &WlSurface) -> Option<&MappedWindow> {
        self.windows_iter()
            .find(|w| w.toplevel().wl_surface() == surface)
    }

    pub fn find_window_mut(&mut self, surface: &WlSurface) -> Option<&mut MappedWindow> {
        self.floating
            .iter_mut()
            .find(|w| w.toplevel().wl_surface() == surface)
            .or(self.tiling.find_window_mut(surface))
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

    pub fn remove_window(&mut self, output: &Output, surface: &WlSurface) -> Option<MappedWindow> {
        if let Some(index) = self
            .floating
            .iter()
            .position(|mapped| mapped.toplevel().wl_surface() == surface)
        {
            return Some(self.floating.remove(index));
        }
        if let window @ Some(_) = self.tiling.remove(surface) {
            self.tiling.update_toplevel_state(
                output.current_location(),
                output
                    .current_mode()
                    .unwrap()
                    .size
                    .to_logical(output.current_scale().integer_scale()),
            );
            return window;
        }
        None
    }
}
