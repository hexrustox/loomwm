use std::rc::Rc;

use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper, element::surface::WaylandSurfaceRenderElement,
    },
    desktop::space::SpaceElement,
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Scale},
};

use crate::{config::LayoutConfig, window::MappedWindow, workspace::tile::TileTree};

mod tile;

pub use tile::{LayoutSet, TileTreeWindow, TileTreeWindowId, test_layout_set};

#[derive(Debug)]
pub struct Workspaces {
    active: u8,
    normal: Vec<(u8, Workspace)>,

    layouts: Rc<LayoutSet>,
    default: String,
}

impl Workspaces {
    pub fn new(config: LayoutConfig) -> Self {
        let layouts = Rc::new(config.layouts);
        Self {
            active: 1,
            normal: vec![(1, Workspace::new(layouts.clone(), &config.default))],
            layouts,
            default: config.default,
        }
    }

    pub fn get_active(&mut self) -> &mut Workspace {
        for (name, workspace) in self.normal.iter_mut() {
            if *name == self.active {
                return workspace;
            }
        }
        panic!("No active workspace")
    }

    fn add_workspace(&mut self, new_name: u8) {
        let mut index = self.normal.len();
        for (i, name) in self
            .normal
            .iter()
            .enumerate()
            .map(|(index, (name, _))| (index, name))
        {
            if new_name == *name {
                panic!("Workspace {name} exist");
            }
            if new_name < *name {
                index = i;
            }
        }
        self.normal.insert(
            index,
            (
                new_name,
                Workspace::new(self.layouts.clone(), &self.default),
            ),
        );
    }

    fn find_workspace(&self, name: u8) -> Option<&Workspace> {
        self.normal.iter().find(|(n, _)| *n == name).map(|(_, w)| w)
    }

    pub fn switch_workspace(&mut self, new_name: u8) {
        if self.find_workspace(new_name).is_none() {
            self.add_workspace(new_name);
        }
        self.active = new_name;
    }

    pub fn find_window(&self, surface: &WlSurface) -> Option<&MappedWindow> {
        for workspace in self.normal.iter().map(|(_, w)| w) {
            if let window @ Some(_) = workspace.find_window(surface) {
                return window;
            }
        }
        None
    }

    pub fn find_window_mut(&mut self, surface: &WlSurface) -> Option<&mut MappedWindow> {
        for workspace in self.normal.iter_mut().map(|(_, w)| w) {
            if let window @ Some(_) = workspace.find_window_mut(surface) {
                return window;
            }
        }
        None
    }

    pub fn remove_window(&mut self, output: &Output, surface: &WlSurface) -> Option<MappedWindow> {
        for workspace in self.normal.iter_mut().map(|(_, w)| w) {
            if let window @ Some(_) = workspace.remove_window(output, surface) {
                return window;
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct Workspace {
    tiling: TileTree,
    floating: Vec<MappedWindow>,
}

impl Workspace {
    fn new(layouts: Rc<LayoutSet>, layout_name: &str) -> Self {
        Self {
            tiling: TileTree::new(layouts, layout_name),
            floating: Vec::new(),
        }
    }

    pub fn add_window(&mut self, output: &Output, mapped: MappedWindow) {
        if let Some(mapped) = self.tiling.insert(mapped) {
            self.floating.insert(0, mapped);
        } else {
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

    pub fn windows_iter(&self) -> impl Iterator<Item = &MappedWindow> {
        self.floating.iter().chain(self.tiling.windows_iter())
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
