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

pub struct Monitor {
    output: Rc<RefCell<Output>>,

    active_workspace: u8,
    workspaces: Vec<(u8, Workspace)>,

    layouts: Rc<LayoutSet>,
    layout_name: Rc<str>,
}

impl Monitor {
    pub fn new(output: Output, layouts: Rc<LayoutSet>, layout_name: Rc<str>) -> Self {
        let output = Rc::new(RefCell::new(output));
        Self {
            output: output.clone(),
            active_workspace: 1,
            workspaces: vec![(1, Workspace::new(output, layouts.clone(), &layout_name))],
            layouts,
            layout_name,
        }
    }

    pub fn get_active_workspace(&mut self) -> &mut Workspace {
        for (name, workspace) in self.workspaces.iter_mut() {
            if *name == self.active_workspace {
                return workspace;
            }
        }
        panic!("No active workspace")
    }

    fn insert_workspace(&mut self, index: usize, name: u8) {
        self.workspaces.insert(
            index,
            (
                name,
                Workspace::new(self.output.clone(), self.layouts.clone(), &self.layout_name),
            ),
        );
    }

    fn add_workspace(&mut self, name: u8) -> &mut Workspace {
        match self.workspaces.binary_search_by_key(&name, |(n, _)| *n) {
            Ok(_) => panic!("Workspace {name} exist"),
            Err(idx) => {
                self.insert_workspace(idx, name);
                &mut self.workspaces[idx].1
            }
        }
    }

    fn find_workspace(&self, name: u8) -> Option<&Workspace> {
        self.workspaces
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, w)| w)
    }

    pub fn switch_workspace(&mut self, name: u8) {
        if self.find_workspace(name).is_none() {
            self.add_workspace(name);
        }
        self.active_workspace = name;
    }

    pub fn move_window_to_workspace(&mut self, surface: &WlSurface, name: u8, focus: bool) {
        let Some(mapped) = self.remove_window(surface) else {
            return;
        };

        let idx = {
            match self.workspaces.binary_search_by_key(&name, |(n, _)| *n) {
                Ok(idx) => idx,
                Err(idx) => {
                    self.insert_workspace(idx, name);
                    idx
                }
            }
        };
        self.workspaces[idx].1.add_window(mapped);
        if focus {
            self.switch_workspace(name);
        }
    }

    pub fn find_window(&self, surface: &WlSurface) -> Option<&MappedWindow> {
        for workspace in self.workspaces.iter().map(|(_, w)| w) {
            if let window @ Some(_) = workspace.find_window(surface) {
                return window;
            }
        }
        None
    }

    pub fn find_window_mut(&mut self, surface: &WlSurface) -> Option<&mut MappedWindow> {
        for workspace in self.workspaces.iter_mut().map(|(_, w)| w) {
            if let window @ Some(_) = workspace.find_window_mut(surface) {
                return window;
            }
        }
        None
    }

    pub fn remove_window(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        for workspace in self.workspaces.iter_mut().map(|(_, w)| w) {
            if let window @ Some(_) = workspace.remove_window(surface) {
                return window;
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct Workspace {
    output: Rc<RefCell<Output>>,
    tiling: TileTree,
    floating: Vec<MappedWindow>,
}

impl Workspace {
    fn new(output: Rc<RefCell<Output>>, layouts: Rc<LayoutSet>, layout_name: &str) -> Self {
        Self {
            output,
            tiling: TileTree::new(layouts, layout_name),
            floating: Vec::new(),
        }
    }

    pub fn add_window(&mut self, mapped: MappedWindow) {
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

    pub fn remove_window(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        if let Some(index) = self
            .floating
            .iter()
            .position(|mapped| mapped.toplevel().wl_surface() == surface)
        {
            return Some(self.floating.remove(index));
        }
        if let window @ Some(_) = self.tiling.remove(surface) {
            let output = self.output.borrow();
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
