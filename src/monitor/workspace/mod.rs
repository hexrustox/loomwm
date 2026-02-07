use std::rc::Rc;

use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper, element::surface::WaylandSurfaceRenderElement,
    },
    desktop::{Window, space::SpaceElement},
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Scale, Size},
};

use crate::{monitor::workspace::tile::TileTree, window::MappedWindow};

mod tile;

pub use tile::{LayoutSet, TileRatio, TileTreeWindow, TileTreeWindowId, test_layout_set};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorkspaceName {
    Id(u8),
    // Name(String)
}

#[derive(Debug)]
pub struct Workspace {
    output: Output,

    name: WorkspaceName,

    tiling: TileTree,
    floating: Vec<MappedWindow>,

    focus_queue: Vec<Window>,
}

impl Workspace {
    pub fn new(
        output: Output,
        name: WorkspaceName,
        layouts: Rc<LayoutSet>,
        layout_name: &str,
    ) -> Self {
        Self {
            output,
            name,
            tiling: TileTree::new(layouts, layout_name),
            floating: Vec::new(),
            focus_queue: Vec::new(),
        }
    }

    pub fn get_name(&self) -> &WorkspaceName {
        &self.name
    }

    pub fn add_floating_window(&mut self, mapped: MappedWindow) {
        self.insert_focus_queue(mapped.window.clone());
        self.floating.insert(0, mapped);
    }

    pub fn add_tiling_window(&mut self, mapped: MappedWindow, ratio: Option<TileRatio>) {
        self.insert_focus_queue(mapped.window.clone());
        if let Some(mapped) = self.tiling.insert(mapped, ratio) {
            self.floating.insert(0, mapped);
        } else {
            let output = &self.output;
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

    pub fn windows_iter(&self) -> impl Iterator<Item = &MappedWindow> {
        self.floating.iter().chain(self.tiling.windows_iter())
    }

    pub fn find_window(&self, surface: &WlSurface) -> Option<&MappedWindow> {
        self.windows_iter()
            .find(|mapped| mapped.toplevel().wl_surface() == surface)
    }

    pub fn find_window_mut(&mut self, surface: &WlSurface) -> Option<&mut MappedWindow> {
        self.floating
            .iter_mut()
            .find(|mapped| mapped.toplevel().wl_surface() == surface)
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

    pub fn insert_focus_queue(&mut self, window: Window) {
        self.focus_queue.insert(0, window);
    }

    pub fn update_focus_queue(&mut self, window: Window) {
        if let Some(index) = self.focus_queue.iter().position(|w| *w == window) {
            self.focus_queue.remove(index);
        }
        self.focus_queue.push(window);
    }

    pub fn last_focus_window(&self) -> Option<&Window> {
        self.focus_queue.last()
    }

    fn remove_focus_queue(&mut self, mapped: &MappedWindow) {
        if let Some(index) = self
            .focus_queue
            .iter()
            .position(|window| *window == mapped.window)
        {
            self.focus_queue.remove(index);
        }
    }

    fn remove_floating_window(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        let mapped = self
            .floating
            .iter()
            .position(|mapped| mapped.toplevel().wl_surface() == surface)
            .map(|i| self.floating.remove(i))?;
        self.remove_focus_queue(&mapped);
        Some(mapped)
    }

    fn remove_tiling_window(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        let mapped = self.tiling.remove(surface).inspect(|_| {
            let output = &self.output;
            self.tiling.update_toplevel_state(
                output.current_location(),
                output
                    .current_mode()
                    .unwrap()
                    .size
                    .to_logical(output.current_scale().integer_scale()),
            );
        })?;
        self.remove_focus_queue(&mapped);
        Some(mapped)
    }

    pub fn remove_window(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        self.remove_floating_window(surface)
            .or(self.remove_tiling_window(surface))
    }

    pub fn mapped_window_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(&MappedWindow, Point<i32, Logical>)> {
        self.windows_iter().find_map(|mapped| {
            let render_location = mapped.render_location();
            if mapped
                .window
                .is_in_input_region(&(point - render_location.to_f64()))
            {
                Some((mapped, render_location))
            } else {
                None
            }
        })
    }

    pub fn output_size(&self) -> Size<i32, Logical> {
        let output = &self.output;
        output
            .current_mode()
            .unwrap()
            .size
            .to_logical(output.current_scale().integer_scale())
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
            .flat_map(|mapped| {
                if mapped.render {
                    mapped.render_elements::<R>(renderer, scale)
                } else {
                    Vec::new()
                }
            })
            .collect()
    }
}
