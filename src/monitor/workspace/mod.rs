use std::{cell::RefCell, rc::Rc};

use serde::Deserialize;
use smithay::{
    backend::renderer::RendererSuper,
    desktop::space::SpaceElement,
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Rectangle, Size},
};

use crate::{
    monitor::workspace::tile::TileTree,
    utils::{
        Direction,
        types::{RenderElements, Renderer},
    },
    window::{MappedWindow, WindowRole},
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

    special_window: Option<FullscreenWindow>,
}

#[derive(Debug)]
struct FullscreenWindow {
    mapped: MappedWindow,
    rect: Option<Rectangle<i32, Logical>>,
}

pub enum SpecialWindowAction {
    Toggle(MappedWindow),
    Set(MappedWindow),
    Unset,
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
            special_window: None,
        }
    }

    pub fn add_floating(&mut self, mapped: MappedWindow) {
        self.push_focus_queue_front(mapped.clone());
        self.floating.insert(0, mapped);
    }

    pub fn add_tiling(
        &mut self,
        mapped: MappedWindow,
        ratio: Option<TileRatio>,
    ) -> Option<MappedWindow> {
        if let Some(window) = self.tiling.insert(mapped.clone(), ratio) {
            return Some(window);
        } else {
            self.push_focus_queue_front(mapped);
            self.update_tiling_size();
        }
        None
    }

    fn remove_floating(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        let mapped = self
            .floating
            .iter()
            .position(|mapped| mapped.wl_surface() == *surface)
            .map(|i| self.floating.remove(i))?;
        self.remove_from_focus_queue(&mapped);
        Some(mapped)
    }

    fn remove_tiling(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        let mapped = self.tiling.remove(surface).inspect(|_| {
            let output = &self.output;
            self.tiling
                .recalculate_all_sizes(output.current_location(), self.get_output_size());
        })?;
        self.remove_from_focus_queue(&mapped);
        Some(mapped)
    }

    pub fn remove_window_by_surface(&mut self, surface: &WlSurface) -> Option<MappedWindow> {
        let mapped = self
            .remove_floating(surface)
            .or(self.remove_tiling(surface));
        if let Some(FullscreenWindow { mapped: m, .. }) = self.special_window.as_ref()
            && mapped.as_ref() == Some(m)
        {
            let role = m.role();
            self.set_special_window(SpecialWindowAction::Unset, role);
        }
        mapped
    }

    pub fn bring_floating_to_front(&mut self, surface: &WlSurface) {
        if let Some(index) = self
            .floating
            .iter()
            .position(|mapped| mapped.wl_surface() == *surface)
        {
            self.floating[0..=index].rotate_right(1);
        }
    }

    pub fn swap_tiling_windows(&mut self, lhs: &WlSurface, rhs: &WlSurface) {
        self.tiling.swap_windows(lhs, rhs);
    }

    pub fn resize_tiling(
        &mut self,
        surface: &WlSurface,
        direction: Direction,
        unit: impl Into<TileResizeUnit>,
    ) {
        self.tiling.resize_window(surface, direction, unit);
        self.update_tiling_size();
    }

    pub fn find_window_by_surface(&self, surface: &WlSurface) -> Option<&MappedWindow> {
        self.iter_all_windows()
            .find(|mapped| mapped.wl_surface() == *surface)
    }

    pub fn window_at_point(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(&MappedWindow, Point<i32, Logical>)> {
        self.iter_visible_windows().find_map(|mapped| {
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

    pub fn iter_all_windows(&self) -> impl Iterator<Item = &MappedWindow> {
        self.floating.iter().chain(self.tiling.iter_windows())
    }

    pub fn iter_floating_windows(&self) -> impl Iterator<Item = &MappedWindow> + Clone {
        self.floating.iter()
    }

    pub fn iter_tiling_windows(&self) -> impl Iterator<Item = &MappedWindow> + Clone {
        self.tiling.iter_windows()
    }

    fn iter_visible_windows(&self) -> Box<dyn Iterator<Item = &MappedWindow> + '_> {
        if let Some(FullscreenWindow { mapped, .. }) = &self.special_window {
            if mapped.role() == WindowRole::Fullscreen {
                return Box::new(std::iter::once(mapped));
            }
            return Box::new(
                {
                    if self.hide_floating
                        || self
                            .special_window
                            .as_ref()
                            .is_some_and(|o| o.mapped.is_floating())
                    {
                        return Box::new(std::iter::empty());
                    }
                    Box::new(self.floating.iter())
                }
                .chain(std::iter::once(mapped)),
            );
        }
        Box::new(
            {
                if self.hide_floating {
                    Box::new(std::iter::empty())
                } else {
                    Box::new(self.floating.iter()) as Box<dyn Iterator<Item = &MappedWindow>>
                }
            }
            .chain(self.tiling.iter_windows()),
        )
    }

    pub fn window_count(&self) -> usize {
        self.floating.len() + self.tiling.window_count() as usize
    }

    pub fn update_tiling_size(&mut self) {
        self.tiling
            .recalculate_all_sizes(self.output.current_location(), self.get_output_size());
    }

    pub fn update_tiling_layout(&mut self, layout_name: &str) {
        self.tiling.update_layout(layout_name);
        self.tiling
            .recalculate_all_sizes(self.output.current_location(), self.get_output_size());
    }

    pub fn last_focused_tiling_window(
        &self,
        surface: &WlSurface,
        direction: Direction,
    ) -> Option<&MappedWindow> {
        let mapped_list = self.tiling.find_windows_in_direction(surface, direction);
        self.focus_queue
            .iter()
            .rev()
            .find(|&m| mapped_list.contains(&m))
    }

    pub fn get_last_focused_window(&self) -> Option<&MappedWindow> {
        self.focus_queue.last()
    }

    pub fn push_focus_queue_front(&mut self, mapped: MappedWindow) {
        self.focus_queue.insert(0, mapped);
    }

    pub fn push_focus_queue_back(&mut self, mapped: MappedWindow) {
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

    fn restore_focus_after_hide(&mut self) {
        if let Some(mapped) = self
            .focus_queue
            .iter()
            .rfind(|mapped| mapped.is_floating() != self.hide_floating)
        {
            self.push_focus_queue_back(mapped.clone());
        }
    }

    fn remove_from_focus_queue(&mut self, mapped: &MappedWindow) {
        if let Some(index) = self.focus_queue.iter().position(|m| m == mapped) {
            self.focus_queue.remove(index);
        }
    }

    pub fn is_floating_hidden(&self) -> bool {
        self.hide_floating
    }

    pub fn hide_floating(&mut self, value: Option<bool>) {
        self.hide_floating = value.unwrap_or(!self.hide_floating);
        self.restore_focus_after_hide();
    }

    pub fn has_special_window(&self) -> bool {
        self.special_window.is_some()
    }

    pub fn get_special_window_role(&self) -> Option<WindowRole> {
        self.special_window.as_ref().map(|o| o.mapped.role())
    }

    pub fn special_window_is_floating(&self) -> bool {
        self.special_window
            .as_ref()
            .is_some_and(|o| o.mapped.is_floating())
    }

    pub fn set_special_window(&mut self, action: SpecialWindowAction, role: WindowRole) {
        match action {
            SpecialWindowAction::Toggle(mapped) => {
                if mapped.role() == role {
                    self.deactivate_special_window();
                } else {
                    self.activate_special_window(mapped, role);
                }
            }
            SpecialWindowAction::Set(mapped) => {
                self.activate_special_window(mapped, role);
            }
            SpecialWindowAction::Unset => {
                self.deactivate_special_window();
            }
        }
    }

    fn activate_special_window(&mut self, mut mapped: MappedWindow, role: WindowRole) {
        self.deactivate_special_window();

        let loc = mapped.get_location();
        let size = mapped.get_size();
        mapped.set_location((0, 0).into());
        mapped.set_size(self.get_output_size());
        match role {
            WindowRole::Maximized => mapped.set_maximized(true),
            WindowRole::Fullscreen => mapped.set_fullscreen(true),
            _ => {}
        }
        self.special_window = Some(FullscreenWindow {
            rect: if mapped.is_floating() {
                Some(Rectangle::new(loc, size))
            } else {
                None
            },
            mapped,
        });
    }

    fn deactivate_special_window(&mut self) {
        if let Some(FullscreenWindow {
            mut mapped, rect, ..
        }) = self.special_window.take()
        {
            if let Some(rect) = rect {
                mapped.set_location(rect.loc);
                mapped.set_size(rect.size);
            } else {
                self.update_tiling_size();
            }
            match mapped.role() {
                WindowRole::Maximized => mapped.set_maximized(false),
                WindowRole::Fullscreen => mapped.set_fullscreen(false),
                _ => {}
            }
        }
    }

    pub fn get_name(&self) -> &WorkspaceName {
        &self.name
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

    pub fn refresh_windows(&self) {
        self.iter_all_windows().for_each(|mapped| {
            mapped.window().refresh();
            mapped.toplevel().send_pending_configure();
        });
    }

    pub fn render_elements<R: Renderer>(&mut self, renderer: &mut R) -> Vec<RenderElements<R>>
    where
        <R as RendererSuper>::TextureId: Clone + 'static,
    {
        let scale = self.output.current_scale().fractional_scale().into();
        self.iter_visible_windows()
            .flat_map(|mapped| mapped.render_elements::<R>(renderer, scale))
            .collect()
    }
}
