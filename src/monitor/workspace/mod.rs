use std::rc::Rc;

use serde::Deserialize;
use smithay::{
    backend::renderer::{
        ImportAll, Renderer, RendererSuper,
        element::{surface::WaylandSurfaceRenderElement, utils::CropRenderElement},
    },
    desktop::space::SpaceElement,
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Scale, Size},
};

use crate::{
    monitor::{
        apply_rule_to_mapped_window,
        workspace::tile::{TileInsertion, TileTree},
    },
    utils::{Direction, get_app_id_and_title},
    window::{
        MappedWindow,
        rule::{WindowRuleCandidate, WindowRules},
    },
};

mod tile;

pub use tile::{LayoutSet, TileRatio, TileResizeUnit, TileTreeSearchKey, TileTreeWindow};

// TODO named workspace
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Hash)]
#[serde(untagged)]
pub enum WorkspaceName {
    Id(u8),
    // Name(String)
}

impl WorkspaceName {
    pub fn as_id(&self) -> u8 {
        match self {
            Self::Id(x) => *x,
        }
    }
}

#[derive(Debug)]
pub struct Workspace {
    output: Output,

    name: WorkspaceName,

    tiling: TileTree,
    floating: Vec<MappedWindow>,
    // TODO hide floating
    focus_queue: Vec<MappedWindow>,
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
        self.insert_into_focus_queue(mapped.clone());
        self.floating.insert(0, mapped);
    }

    pub fn add_tiling_window(
        &mut self,
        mapped: MappedWindow,
        ratio: Option<TileRatio>,
    ) -> Option<MappedWindow> {
        self.insert_into_focus_queue(mapped.clone());
        if let Some(insertion) = self.tiling.insert(TileInsertion::Window {
            window: mapped,
            ratio,
        }) {
            return Some(insertion.into_window());
        } else {
            self.update_tiling_window_size();
        }
        None
    }

    fn update_tiling_window_size(&mut self) {
        let output = &self.output;
        self.tiling
            .update_tile_size(output.current_location(), self.get_output_size());
    }

    pub fn windows_iter(&self) -> impl Iterator<Item = &MappedWindow> {
        self.floating.iter().chain(self.tiling.windows_iter())
    }

    pub fn find_window(&self, surface: &WlSurface) -> Option<&MappedWindow> {
        self.windows_iter()
            .find(|mapped| mapped.wl_surface() == *surface)
    }

    pub fn last_focused_tiling_window_in_direction(
        &self,
        surface: &WlSurface,
        direction: Direction,
    ) -> Option<&MappedWindow> {
        // TODO do closest distance instead of last focus as well
        let mapped_list = self.tiling.find_windows_in_direction(surface, direction);
        self.focus_queue
            .iter()
            .rev()
            .find(|&m| mapped_list.contains(&m))
    }

    pub fn swap_tiling_window(&mut self, lhs: &WlSurface, rhs: &WlSurface) {
        self.tiling.swap_window(lhs, rhs);
    }

    pub fn resize_tiling_window(
        &mut self,
        surface: &WlSurface,
        direction: Direction,
        unit: impl Into<TileResizeUnit>,
    ) {
        self.tiling.resize_tile(surface, direction, unit);
        self.update_tiling_window_size();
    }

    pub fn raise_floating_window(&mut self, surface: &WlSurface) {
        if let Some(index) = self
            .floating
            .iter()
            .position(|mapped| mapped.wl_surface() == *surface)
        {
            self.floating[0..=index].rotate_right(1);
        }
    }

    pub fn insert_into_focus_queue(&mut self, mapped: MappedWindow) {
        self.focus_queue.insert(0, mapped);
    }

    pub fn append_to_focus_queue(&mut self, mapped: MappedWindow) {
        if let Some(index) = self.focus_queue.iter().position(|m| *m == mapped) {
            self.focus_queue.remove(index);
        }
        self.focus_queue.push(mapped);
    }

    pub fn get_last_focused_window(&self) -> Option<&MappedWindow> {
        self.focus_queue.last()
    }

    fn remove_from_focus_queue(&mut self, mapped: &MappedWindow) {
        if let Some(index) = self.focus_queue.iter().position(|m| m == mapped) {
            self.focus_queue.remove(index);
        }
    }

    fn remove_floating_window(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        let mapped = self
            .floating
            .iter()
            .position(|mapped| mapped.wl_surface() == *surface)
            .map(|i| self.floating.remove(i))?;
        self.remove_from_focus_queue(&mapped);
        Some(mapped)
    }

    fn remove_tiling_window(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        let mapped = self.tiling.remove(surface).inspect(|_| {
            let output = &self.output;
            self.tiling
                .update_tile_size(output.current_location(), self.get_output_size());
        })?;
        self.remove_from_focus_queue(&mapped);
        Some(mapped)
    }

    pub fn remove_window(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        self.remove_floating_window(surface)
            .or(self.remove_tiling_window(surface))
    }

    pub fn apply_rule_to_windows(&mut self, window_rules: &WindowRules) {
        let workspace_name = self.get_name().clone();
        for mapped in self
            .floating
            .iter_mut()
            .chain(self.tiling.windows_iter_mut())
        {
            let (app_id, title) = get_app_id_and_title(&mapped.wl_surface());
            let properties = window_rules.get_properties(
                WindowRuleCandidate {
                    app_id,
                    title,
                    focus: mapped.get_focus(),
                    float: mapped.get_floating(),
                    workspace_name: workspace_name.clone(),
                },
                false,
            );
            apply_rule_to_mapped_window(mapped, properties.dynamic);
        }
    }

    pub fn find_mapped_window_under(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(&MappedWindow, Point<i32, Logical>)> {
        self.windows_iter().find_map(|mapped| {
            let render_location = mapped.render_location();
            if mapped
                .window()
                .is_in_input_region(&(point - render_location.to_f64()))
            {
                Some((mapped, render_location))
            } else {
                None
            }
        })
    }

    pub fn get_output_size(&self) -> Size<i32, Logical> {
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
    ) -> Vec<CropRenderElement<WaylandSurfaceRenderElement<R>>>
    where
        <R as RendererSuper>::TextureId: Clone + 'static,
    {
        self.windows_iter()
            .flat_map(|mapped| mapped.render_elements::<R>(renderer, scale))
            .collect()
    }
}
