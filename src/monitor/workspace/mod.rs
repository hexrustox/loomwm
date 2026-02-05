use std::{cell::RefCell, rc::Rc};

use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper, element::surface::WaylandSurfaceRenderElement,
    },
    desktop::space::SpaceElement,
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Scale},
};

use crate::{monitor::workspace::tile::TileTree, window::MappedWindow};

mod tile;

pub use tile::{LayoutSet, TileTreeWindow, TileTreeWindowId, test_layout_set};

#[derive(Debug)]
pub struct Workspace {
    output: Rc<RefCell<Output>>,
    tiling: TileTree,
    floating: Vec<MappedWindow>,
}

impl Workspace {
    pub fn new(output: Rc<RefCell<Output>>, layouts: Rc<LayoutSet>, layout_name: &str) -> Self {
        Self {
            output,
            tiling: TileTree::new(layouts, layout_name),
            floating: Vec::new(),
        }
    }

    pub fn add_floating_window(&mut self, mapped: MappedWindow) {
        self.floating.insert(0, mapped);
    }

    pub fn add_tiling_window(&mut self, mapped: MappedWindow) {
        if let Some(mapped) = self.tiling.insert(mapped) {
            self.floating.insert(0, mapped);
        } else {
            let output = self.output.borrow();
            self.tiling.update_toplevel_state(
                output.current_location(),
                output
                    .current_mode()
                    .unwrap()
                    .size
                    .to_logical(output.current_scale().integer_scale()),
            );
        }
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

    fn remove_floating_window(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        self.floating
            .iter()
            .position(|mapped| mapped.toplevel().wl_surface() == surface)
            .map(|i| self.floating.remove(i))
    }

    fn remove_tiling_window(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        self.tiling.remove(surface).inspect(|_| {
            let output = self.output.borrow();
            self.tiling.update_toplevel_state(
                output.current_location(),
                output
                    .current_mode()
                    .unwrap()
                    .size
                    .to_logical(output.current_scale().integer_scale()),
            );
        })
    }

    pub fn remove_window(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        self.remove_floating_window(surface)
            .or(self.remove_tiling_window(surface))
    }

    pub fn move_window_to_floating(&mut self, surface: &WlSurface) {
        if let Some(mut mapped) = self.remove_tiling_window(surface) {
            mapped.floating = true;
            self.add_floating_window(mapped);
        }
    }

    pub fn move_window_to_tiling(&mut self, surface: &WlSurface) {
        if let Some(mut mapped) = self.remove_floating_window(surface) {
            mapped.floating = false;
            self.add_tiling_window(mapped);
        }
    }

    pub fn toggle_window_floating(&mut self, surface: &WlSurface) {
        if let Some(mut mapped) = self.remove_window(surface) {
            mapped.floating = !mapped.floating;
            println!("{}", mapped.floating);
            if mapped.floating {
                self.add_floating_window(mapped);
            } else {
                self.add_tiling_window(mapped);
            }
        }
    }

    pub fn mapped_window_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(&MappedWindow, Point<i32, Logical>)> {
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

    pub fn windows_iter(&self) -> impl Iterator<Item = &MappedWindow> {
        self.floating.iter().chain(self.tiling.windows_iter())
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
}
