use std::{cell::RefCell, rc::Rc};

use serde::Deserialize;
use smithay::{
    backend::renderer::RendererSuper,
    desktop::space::SpaceElement,
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Size},
};

use crate::{
    monitor::workspace::tile::{TileInsertion, TileTree},
    utils::{
        Direction, apply_rule_to_mapped_window, get_app_id_and_title,
        types::{RenderElements, Renderer},
    },
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
    focus_queue: Vec<MappedWindow>,

    hide_floating: bool,
}

impl Workspace {
    pub fn new(
        output: Output,
        name: WorkspaceName,
        layouts: Rc<RefCell<LayoutSet>>,
        layout_name: &str,
    ) -> Self {
        Self {
            output,
            name,
            tiling: TileTree::new(layouts, layout_name),
            floating: Vec::new(),
            focus_queue: Vec::new(),
            hide_floating: false,
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
        if let Some(insertion) = self.tiling.insert(TileInsertion::Window {
            window: mapped.clone(),
            ratio,
        }) {
            return Some(insertion.into_window());
        } else {
            self.insert_into_focus_queue(mapped);
            self.update_tiling_windows_size();
        }
        None
    }

    pub fn update_tiling_windows_size(&mut self) {
        let output = &self.output;
        self.tiling
            .update_tile_size(output.current_location(), self.get_output_size());
    }

    pub fn windows_iter(&self) -> impl Iterator<Item = &MappedWindow> {
        (if self.hide_floating {
            Box::new(std::iter::empty())
        } else {
            Box::new(self.floating.iter()) as Box<dyn Iterator<Item = _>>
        })
        .chain(self.tiling.windows_iter())
    }

    pub fn floating_windows_iter(&self) -> impl Iterator<Item = &MappedWindow> + Clone {
        self.floating.iter()
    }

    pub fn tiling_windows_iter(&self) -> impl Iterator<Item = &MappedWindow> + Clone {
        self.tiling.windows_iter()
    }

    pub fn windows_count(&self) -> usize {
        self.floating.len() + self.tiling.windows_count() as usize
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
        let mapped_list = self
            .tiling
            .find_nearest_windows_in_direction(surface, direction);
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
        self.update_tiling_windows_size();
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
        if let Some(m) = self.focus_queue.last()
            && *m == mapped
        {
            return;
        }
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

    fn restore_focus_after_window_hidden(&mut self) {
        if let Some(mapped) = self
            .focus_queue
            .iter()
            .rfind(|mapped| mapped.get_floating() != self.hide_floating)
        {
            self.append_to_focus_queue(mapped.clone());
        }
    }

    pub fn get_floating_window_hidden(&self) -> bool {
        self.hide_floating
    }

    pub fn set_floating_window_hidden(&mut self, value: Option<bool>) {
        self.hide_floating = value.unwrap_or(!self.hide_floating);
        self.restore_focus_after_window_hidden();
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
                    is_swap_source: false,
                    is_swap_target: false,
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

    pub fn refresh(&self) {
        self.windows_iter().for_each(|mapped| {
            mapped.window().refresh();
            mapped.toplevel().send_pending_configure();
        });
    }

    pub fn get_output(&self) -> Output {
        self.output.clone()
    }

    pub fn get_output_size(&self) -> Size<i32, Logical> {
        let output = &self.output;
        output
            .current_mode()
            .unwrap()
            .size
            .to_logical(output.current_scale().integer_scale())
    }

    pub fn render_elements<R: Renderer>(&mut self, renderer: &mut R) -> Vec<RenderElements<R>>
    where
        <R as RendererSuper>::TextureId: Clone + 'static,
    {
        let scale = self.output.current_scale().fractional_scale().into();
        self.windows_iter()
            .flat_map(|mapped| mapped.render_elements::<R>(renderer, scale))
            .collect()
    }
}
