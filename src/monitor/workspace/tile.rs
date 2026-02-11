use crate::{
    input::{WindowDirection, WindowUnit},
    utils::floats_to_ints,
    window::MappedWindow,
};
use serde::Deserialize;
use slotmap::{SlotMap, new_key_type};
use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Size},
};
use std::{borrow::Cow, collections::HashMap, fmt::Debug, ops::Neg, rc::Rc};

new_key_type! { struct TileId; }

type TileArena<T> = SlotMap<TileId, Tile<T>>;

#[derive(Debug)]
pub struct TileTree<T = MappedWindow>
where
    T: TileTreeWindow,
{
    arena: TileArena<T>,
    root: TileId,
    current_tile: TileId,
    layouts: Rc<LayoutSet>,
    layout_name: String,
    // TODO better structure
    layout_trace: Vec<TileLayoutTrace>,
}

#[derive(Debug)]
struct Tile<T> {
    kind: TileKind<T>,
    ratio: TileRatio,
    parent: Option<TileId>,
}

impl<T> Tile<T> {
    fn into_window(self) -> T {
        match self.kind {
            TileKind::Window(window) => window,
            _ => unreachable!(),
        }
    }

    fn as_window(&self) -> &T {
        match &self.kind {
            TileKind::Window(window) => window,
            _ => unreachable!(),
        }
    }

    fn as_window_mut(&mut self) -> &mut T {
        match &mut self.kind {
            TileKind::Window(window) => window,
            _ => unreachable!(),
        }
    }

    fn into_layout_tiles(self) -> Vec<TileId> {
        match self.kind {
            TileKind::Layout { tiles, .. } => tiles,
            _ => unreachable!(),
        }
    }

    fn as_layout_tiles(&self) -> &Vec<TileId> {
        match &self.kind {
            TileKind::Layout { tiles, .. } => tiles,
            _ => unreachable!(),
        }
    }

    fn as_layout_tiles_mut(&mut self) -> &mut Vec<TileId> {
        match &mut self.kind {
            TileKind::Layout { tiles, .. } => tiles,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(transparent)]
pub struct TileRatio(pub f64);

impl Default for TileRatio {
    fn default() -> Self {
        Self(1.0)
    }
}

impl std::hash::Hash for TileRatio {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl Neg for TileRatio {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

#[derive(Debug)]
enum TileKind<T> {
    Window(T),
    Layout {
        split: TileSplit,
        orientation: TileOrientation,
        tiles: Vec<TileId>,
        location: Point<i32, Logical>,
        size: Size<i32, Logical>,
    },
}

pub trait TileTreeWindow: Debug + Clone {
    fn match_id(&self, key: TileTreeSearchKey) -> bool;
    fn get_location(&self) -> Point<i32, Logical>;
    fn get_size(&self) -> Size<i32, Logical>;
    fn set_location(&mut self, location: Point<i32, Logical>);
    fn set_size(&mut self, size: Size<i32, Logical>);
    fn swap(&mut self, other: &mut Self);
}

#[derive(Clone, Copy)]
pub enum TileTreeSearchKey<'a> {
    #[cfg(test)]
    Id(u32),
    #[allow(dead_code)]
    WlSurface(&'a WlSurface),
}

#[cfg(test)]
impl From<u32> for TileTreeSearchKey<'_> {
    fn from(value: u32) -> Self {
        Self::Id(value)
    }
}

impl<'a> From<&'a WlSurface> for TileTreeSearchKey<'a> {
    fn from(value: &'a WlSurface) -> Self {
        Self::WlSurface(value)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TileSplit {
    #[default]
    Vertical,
    Horizontal,
}

#[derive(Debug, Default, Clone, Copy, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "snake_case")]
enum TileOrientation {
    #[default]
    BottomRight,
    TopLeft,
}

#[derive(Debug, Default, Deserialize)]
#[serde(transparent)]
pub struct LayoutSet(HashMap<String, LayoutSchema>);

impl LayoutSet {
    fn get(&self, layout_name: &str) -> Cow<'_, LayoutSchema> {
        #[cfg(test)]
        {
            Cow::Borrowed(
                self.0
                    .get(layout_name)
                    .unwrap_or_else(|| panic!(r#"Unknown layout: "{layout_name}""#)),
            )
        }
        #[cfg(not(test))]
        {
            self.0
                .get(layout_name)
                .map(Cow::Borrowed)
                .unwrap_or_default()
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LayoutSchema {
    #[serde(default)]
    split: TileSplit,
    #[serde(default)]
    orientation: TileOrientation,
    nodes: Vec<LayoutNode>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LayoutNode {
    layout: Option<String>,
    repeat: TileRepeat,
    ratio: TileRatio,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
struct TileRepeat(usize);

impl Default for TileRepeat {
    fn default() -> Self {
        TileRepeat(1)
    }
}

#[derive(Debug)]
struct TileLayoutTrace {
    layout: String,
    index: usize,
    repeat: usize,
}

impl TileLayoutTrace {
    fn new(layout_name: &str) -> Self {
        Self {
            layout: layout_name.to_string(),
            index: 0,
            repeat: 0,
        }
    }
}

pub trait SearchKey<'a>: Into<TileTreeSearchKey<'a>> + Copy {}
impl<'a, T> SearchKey<'a> for T where T: Into<TileTreeSearchKey<'a>> + Copy {}

impl<T: TileTreeWindow> TileTree<T> {
    pub fn new(layouts: Rc<LayoutSet>, layout_name: &str) -> Self {
        let mut arena = SlotMap::with_key();
        let layout = layouts.get(layout_name);

        let new_tile = Tile {
            kind: TileKind::Layout {
                split: layout.split,
                orientation: layout.orientation,
                tiles: Vec::new(),
                location: (0, 0).into(),
                size: (0, 0).into(),
            },
            ratio: TileRatio::default(),
            parent: None,
        };
        let root = arena.insert(new_tile);

        Self {
            arena,
            root,
            current_tile: root,
            layouts,
            layout_name: layout_name.to_string(),
            layout_trace: vec![TileLayoutTrace::new(layout_name)],
        }
    }

    pub fn insert(&mut self, window: T, ratio: Option<TileRatio>) -> Option<T> {
        let Some(trace) = self.layout_trace.last_mut() else {
            return Some(window);
        };

        let layout = self.layouts.get(&trace.layout);
        for i in trace.index..layout.nodes.len() {
            trace.index = i;
            let node = &layout.nodes[i];
            if trace.repeat < node.repeat.0 {
                if let Some(layout_name) = &node.layout {
                    let layout = self.layouts.get(layout_name);
                    let new_tile = Tile {
                        kind: TileKind::Layout {
                            split: layout.split,
                            orientation: layout.orientation,
                            tiles: Vec::new(),
                            location: (0, 0).into(),
                            size: (0, 0).into(),
                        },
                        ratio: node.ratio,
                        parent: Some(self.current_tile),
                    };
                    let tile_id = self.arena.insert(new_tile);

                    self.arena[self.current_tile]
                        .as_layout_tiles_mut()
                        .push(tile_id);

                    self.current_tile = tile_id;
                    trace.repeat += 1;
                    self.layout_trace.push(TileLayoutTrace::new(layout_name));
                    self.insert(window, ratio);
                } else {
                    let new_tile = Tile {
                        kind: TileKind::Window(window),
                        ratio: ratio.unwrap_or(node.ratio),
                        parent: Some(self.current_tile),
                    };
                    let tile_id = self.arena.insert(new_tile);

                    self.arena[self.current_tile]
                        .as_layout_tiles_mut()
                        .push(tile_id);

                    trace.repeat += 1;
                }
                return None;
            }
            trace.repeat = 0;
        }

        let Some(parent) = self.arena[self.current_tile].parent else {
            return Some(window);
        };
        self.current_tile = parent;
        self.layout_trace.pop();
        if let Some(pre_trace) = self.layout_trace.last_mut() {
            let layout = self.layouts.get(&pre_trace.layout);
            let node = &layout.nodes[pre_trace.index];
            if pre_trace.repeat >= node.repeat.0 {
                pre_trace.repeat = 0;
                pre_trace.index += 1;
            }
        };
        self.insert(window, ratio)
    }

    // TODO improve efficiency
    fn find_tile_id(&self, key: TileTreeSearchKey) -> Option<TileId> {
        fn traverse<T: TileTreeWindow>(
            arena: &TileArena<T>,
            current_id: TileId,
            key: TileTreeSearchKey,
        ) -> Option<TileId> {
            match &arena[current_id].kind {
                TileKind::Window(window) => {
                    if window.match_id(key) {
                        return Some(current_id);
                    }
                }
                TileKind::Layout { tiles, .. } => {
                    for &child_id in tiles.iter() {
                        if let res @ Some(_) = traverse(arena, child_id, key) {
                            return res;
                        }
                    }
                }
            }
            None
        }

        traverse(&self.arena, self.root, key)
    }

    // TODO improve efficiency
    pub fn remove<'a>(&mut self, key: impl SearchKey<'a>) -> Option<T> {
        struct RemoveTile {
            parent: TileId,
            index: usize,
        }

        fn traverse<T: TileTreeWindow>(
            arena: &TileArena<T>,
            layout_id: TileId,
            key: TileTreeSearchKey<'_>,
        ) -> Option<RemoveTile> {
            for (index, tile_id) in arena[layout_id].as_layout_tiles().iter().enumerate() {
                match &arena[*tile_id] {
                    Tile {
                        kind: TileKind::Window(window),
                        ..
                    } => {
                        if window.match_id(key) {
                            return Some(RemoveTile {
                                parent: layout_id,
                                index,
                            });
                        }
                    }
                    _ => {
                        if let res @ Some(_) = traverse(arena, *tile_id, key) {
                            return res;
                        }
                    }
                }
            }

            None
        }

        if let Some(RemoveTile { parent, index }) = traverse(&self.arena, self.root, key.into()) {
            let remove_id = self.arena[parent].as_layout_tiles_mut().remove(index);
            let window = self.arena.remove(remove_id).unwrap().into_window();

            let mut handle_ids = Vec::new();
            for id in self.arena[self.root].as_layout_tiles() {
                handle_ids.push(*id);
            }

            let mut extracted_windows = Vec::new();
            let mut i = 0;
            while i < handle_ids.len() {
                let id = handle_ids[i];
                match self.arena[id] {
                    Tile {
                        kind: TileKind::Window(..),
                        ratio,
                        ..
                    } => {
                        extracted_windows
                            .push((self.arena.remove(id).unwrap().into_window(), ratio));
                    }
                    Tile {
                        kind: TileKind::Layout { .. },
                        ..
                    } => {
                        handle_ids.splice(
                            i + 1..i + 1,
                            self.arena.remove(id).unwrap().into_layout_tiles(),
                        );
                    }
                }

                i += 1;
            }

            *self = Self::new(self.layouts.clone(), &self.layout_name);
            for (window, ratio) in extracted_windows {
                self.insert(window, Some(ratio));
            }

            return Some(window);
        }

        None
    }

    pub fn update_window_size(&mut self, location: Point<i32, Logical>, size: Size<i32, Logical>) {
        enum Update {
            Win(TileId, Point<i32, Logical>, Size<i32, Logical>),
            Lay(TileId, Point<i32, Logical>, Size<i32, Logical>),
        }

        fn traverse<T>(
            arena: &TileArena<T>,
            id: TileId,
            mut location: Point<i32, Logical>,
            size: Size<i32, Logical>,
            updates: &mut Vec<Update>,
        ) {
            updates.push(Update::Lay(id, location, size));

            let TileKind::Layout {
                split,
                orientation,
                ref tiles,
                ..
            } = arena[id].kind
            else {
                return;
            };

            let total_ratio: f64 = tiles.iter().map(|&tid| arena[tid].ratio.0).sum();
            let total_len = match split {
                TileSplit::Vertical => size.w,
                TileSplit::Horizontal => size.h,
            };

            let floats: Vec<_> = tiles
                .iter()
                .map(|&tid| total_len as f64 * arena[tid].ratio.0 / total_ratio)
                .collect();
            let ints = floats_to_ints(&floats, total_len);

            let mut children: Vec<_> = tiles.iter().copied().zip(ints).collect();
            if matches!(orientation, TileOrientation::TopLeft) {
                children.reverse();
            }

            for (tid, t_len) in children {
                let t_sz = match split {
                    TileSplit::Vertical => Size::new(t_len, size.h),
                    TileSplit::Horizontal => Size::new(size.w, t_len),
                };

                match &arena[tid].kind {
                    TileKind::Window(_) => updates.push(Update::Win(tid, location, t_sz)),
                    TileKind::Layout { .. } => traverse(arena, tid, location, t_sz, updates),
                }

                match split {
                    TileSplit::Vertical => location.x += t_sz.w,
                    TileSplit::Horizontal => location.y += t_sz.h,
                }
            }
        }

        let mut updates = Vec::new();
        traverse(&self.arena, self.root, location, size, &mut updates);

        for update in updates {
            match update {
                Update::Win(id, loc, sz) => {
                    let window = self.arena[id].as_window_mut();
                    window.set_location(loc);
                    window.set_size(sz);
                }
                Update::Lay(id, loc, sz) => {
                    if let TileKind::Layout { location, size, .. } = &mut self.arena[id].kind {
                        *location = loc;
                        *size = sz;
                    }
                }
            }
        }
    }

    pub fn windows_iter(&self) -> impl Iterator<Item = &T> + '_ {
        struct WindowsIter<'a, T> {
            arena: &'a TileArena<T>,
            stack: Vec<TileId>,
        }

        impl<'a, T: TileTreeWindow> Iterator for WindowsIter<'a, T> {
            type Item = &'a T;

            fn next(&mut self) -> Option<Self::Item> {
                while let Some(tile_id) = self.stack.pop() {
                    match &self.arena[tile_id] {
                        Tile {
                            kind: TileKind::Window(window),
                            ..
                        } => return Some(window),
                        _ => {
                            for &child_id in self.arena[tile_id].as_layout_tiles().iter().rev() {
                                self.stack.push(child_id);
                            }
                        }
                    }
                }
                None
            }
        }

        let mut stack = Vec::new();
        for &id in self.arena[self.root].as_layout_tiles().iter().rev() {
            stack.push(id);
        }

        WindowsIter {
            arena: &self.arena,
            stack,
        }
    }

    pub fn windows_iter_mut(&mut self) -> impl Iterator<Item = &mut T> + '_ {
        fn collect_window_ids<T: TileTreeWindow>(
            arena: &TileArena<T>,
            tile_id: TileId,
            ids: &mut Vec<TileId>,
        ) {
            match &arena[tile_id] {
                Tile {
                    kind: TileKind::Window(_),
                    ..
                } => ids.push(tile_id),
                tile => {
                    for &child_id in tile.as_layout_tiles() {
                        collect_window_ids(arena, child_id, ids);
                    }
                }
            }
        }

        let mut window_ids = Vec::new();
        for &id in self.arena[self.root].as_layout_tiles() {
            collect_window_ids(&self.arena, id, &mut window_ids);
        }

        let arena = &mut self.arena as *mut TileArena<T>;

        window_ids.into_iter().filter_map(move |tile_id| {
            // SAFETY: each tile_id appears at most once, so all returned references are disjoint
            let arena = unsafe { &mut *arena };
            match &mut arena[tile_id] {
                Tile {
                    kind: TileKind::Window(window),
                    ..
                } => Some(window),
                _ => None,
            }
        })
    }

    pub fn find_windows_in_direction<'a>(
        &self,
        key: impl SearchKey<'a>,
        direction: WindowDirection,
    ) -> Vec<&T> {
        #[derive(Debug)]
        struct Rect {
            x: i32,
            y: i32,
            w: i32,
            h: i32,
        }

        impl Rect {
            fn new(location: Point<i32, Logical>, size: Size<i32, Logical>) -> Self {
                Self {
                    x: location.x,
                    y: location.y,
                    w: size.w,
                    h: size.h,
                }
            }
            fn left(&self) -> i32 {
                self.x
            }
            fn right(&self) -> i32 {
                self.x + self.w
            }
            fn top(&self) -> i32 {
                self.y
            }
            fn bottom(&self) -> i32 {
                self.y + self.h
            }

            fn overlaps_horizontally(&self, other: &Rect) -> bool {
                self.left() <= other.right() && self.right() >= other.left()
            }

            fn overlaps_vertically(&self, other: &Rect) -> bool {
                self.top() <= other.bottom() && self.bottom() >= other.top()
            }

            fn is_in_direction(&self, direction: WindowDirection, target: &Rect) -> bool {
                use WindowDirection::*;
                match direction {
                    Up => self.bottom() <= target.top() && self.overlaps_horizontally(target),
                    Down => self.top() >= target.bottom() && self.overlaps_horizontally(target),
                    Left => self.right() <= target.left() && self.overlaps_vertically(target),
                    Right => self.left() >= target.right() && self.overlaps_vertically(target),
                }
            }
        }

        let Some(target_id) = self.find_tile_id(key.into()) else {
            return Vec::new();
        };
        let target_tile = &self.arena[target_id];
        let target_window = target_tile.as_window();

        let target_rect = Rect::new(target_window.get_location(), target_window.get_size());

        let mut candidates = Vec::new();

        let mut stack = Vec::new();
        stack.extend(self.arena[self.root].as_layout_tiles().iter().rev());

        while let Some(tile_id) = stack.pop() {
            match &self.arena[tile_id].kind {
                TileKind::Window(window) => {
                    if tile_id != target_id {
                        let window_rect = Rect::new(window.get_location(), window.get_size());
                        if window_rect.is_in_direction(direction, &target_rect) {
                            candidates.push(window);
                        }
                    }
                }
                TileKind::Layout { tiles, .. } => {
                    stack.extend(tiles.iter().rev());
                }
            }
        }

        candidates
    }

    pub fn swap_window<'a>(&mut self, lhs: impl SearchKey<'a>, rhs: impl SearchKey<'a>) {
        let lhs_id = match self.find_tile_id(lhs.into()) {
            Some(id) => id,
            None => return,
        };
        let rhs_id = match self.find_tile_id(rhs.into()) {
            Some(id) => id,
            None => return,
        };
        if lhs_id == rhs_id {
            return;
        }

        let mut lhs_inner = self.arena[lhs_id].as_window().clone();
        let mut rhs_inner = self.arena[rhs_id].as_window().clone();
        lhs_inner.swap(&mut rhs_inner);
    }

    fn adjust_adjacent_ratios(
        &mut self,
        parent_id: TileId,
        child_id: TileId,
        edge: WindowDirection,
        unit: WindowUnit,
        in_loop: bool,
    ) {
        let parent = &self.arena[parent_id];
        let TileKind::Layout {
            tiles,
            split,
            orientation,
            size,
            ..
        } = &parent.kind
        else {
            return;
        };

        let mut offset = match edge {
            WindowDirection::Left | WindowDirection::Up => -1,
            WindowDirection::Right | WindowDirection::Down => 1,
        };
        offset *= match orientation {
            TileOrientation::BottomRight => 1,
            TileOrientation::TopLeft => -1,
        };

        let index = tiles
            .iter()
            .position(|t| *t == child_id)
            .expect("Cannot find tile in its parent") as i32
            + offset;

        if (0..(tiles.len() as i32)).contains(&index) {
            let total_ratio = tiles
                .iter()
                .fold(0., |acc, id| acc + self.arena[*id].ratio.0);
            let other_id = tiles[index as usize];

            let size_component = match split {
                TileSplit::Vertical => size.w,
                TileSplit::Horizontal => size.h,
            } as f64;

            let ratio_offset = match unit {
                WindowUnit::Ratio(TileRatio(ratio)) => ratio,
                WindowUnit::Px(px) => px as f64 * total_ratio / size_component,
            };

            let window = &mut self.arena[child_id];
            window.ratio.0 += ratio_offset;
            window.ratio.0 = window.ratio.0.max(0.);
            let other = &mut self.arena[other_id];
            other.ratio.0 -= ratio_offset;
            other.ratio.0 = other.ratio.0.max(0.);
        } else {
            #[allow(clippy::collapsible_else_if)]
            if let Some((ancestor_id, refer_id)) = self.find_ancestor_with_split(parent_id, *split)
            {
                self.adjust_adjacent_ratios(ancestor_id, refer_id, edge, unit, false);
            } else if !in_loop {
                self.adjust_adjacent_ratios(parent_id, child_id, edge.opposite(), -unit, true);
            }
        }
    }

    fn find_ancestor_with_split(
        &self,
        start_id: TileId,
        current_split: TileSplit,
    ) -> Option<(TileId, TileId)> {
        let mut refer = start_id;
        let mut some_parent = self.arena[start_id].parent;

        while let Some(parent) = some_parent {
            let TileKind::Layout { split, .. } = &self.arena[parent].kind else {
                return None;
            };
            if *split == current_split {
                return Some((parent, refer));
            }
            refer = parent;
            some_parent = self.arena[parent].parent;
        }
        None
    }

    pub fn resize_tile<'a>(
        &mut self,
        key: impl SearchKey<'a>,
        edge: WindowDirection,
        unit: WindowUnit,
    ) {
        let Some(window_id) = self.find_tile_id(key.into()) else {
            return;
        };
        let Some(parent_id) = self.arena[window_id].parent else {
            return;
        };
        let TileKind::Layout { split, .. } = &self.arena[parent_id].kind else {
            return;
        };

        match (split, edge) {
            (TileSplit::Vertical, WindowDirection::Left | WindowDirection::Right) => {
                self.adjust_adjacent_ratios(parent_id, window_id, edge, unit, false);
            }
            (TileSplit::Horizontal, WindowDirection::Up | WindowDirection::Down) => {
                self.adjust_adjacent_ratios(parent_id, window_id, edge, unit, false);
            }
            (TileSplit::Vertical, WindowDirection::Up | WindowDirection::Down) => {
                if let Some((ancestor_id, refer_id)) =
                    self.find_ancestor_with_split(window_id, TileSplit::Horizontal)
                {
                    self.adjust_adjacent_ratios(ancestor_id, refer_id, edge, unit, false);
                }
            }
            (TileSplit::Horizontal, WindowDirection::Left | WindowDirection::Right) => {
                if let Some((ancestor_id, refer_id)) =
                    self.find_ancestor_with_split(window_id, TileSplit::Vertical)
                {
                    self.adjust_adjacent_ratios(ancestor_id, refer_id, edge, unit, false);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, sync::LazyLock};

    use super::*;
    use test_case::test_case;

    #[derive(Debug, Default, Clone, PartialEq)]
    pub struct TestWindow {
        inner: Rc<RefCell<TestWindowInner>>,
    }

    #[derive(Debug, Default, PartialEq)]
    struct TestWindowInner {
        pub id: Option<u32>,
        pub location: Point<i32, Logical>,
        pub size: Size<i32, Logical>,
    }

    impl TestWindow {
        pub fn new() -> Self {
            Self::default()
        }
    }

    impl TileTreeWindow for TestWindow {
        fn match_id(&self, key: TileTreeSearchKey) -> bool {
            match key {
                TileTreeSearchKey::Id(x) => self.inner.borrow().id == Some(x),
                _ => false,
            }
        }

        fn get_location(&self) -> Point<i32, Logical> {
            self.inner.borrow().location
        }

        fn get_size(&self) -> Size<i32, Logical> {
            self.inner.borrow().size
        }

        fn set_location(&mut self, location: Point<i32, Logical>) {
            self.inner.borrow_mut().location = location;
        }

        fn set_size(&mut self, size: Size<i32, Logical>) {
            self.inner.borrow_mut().size = size;
        }

        fn swap(&mut self, other: &mut Self) {
            let temp = self.inner.borrow().id;
            self.inner.borrow_mut().id = other.inner.borrow().id;
            other.inner.borrow_mut().id = temp;
        }
    }

    impl PartialEq for TileTree<TestWindow> {
        fn eq(&self, other: &Self) -> bool {
            fn compare_tiles(
                tree_a: &TileTree<TestWindow>,
                id_a: TileId,
                tree_b: &TileTree<TestWindow>,
                id_b: TileId,
            ) -> bool {
                let tile_a = match tree_a.arena.get(id_a) {
                    Some(t) => t,
                    None => return false,
                };
                let tile_b = match tree_b.arena.get(id_b) {
                    Some(t) => t,
                    None => return false,
                };

                if tile_a.ratio != tile_b.ratio {
                    return false;
                }

                match (&tile_a.kind, &tile_b.kind) {
                    (TileKind::Window(w1), TileKind::Window(w2)) => w1 == w2,
                    (
                        TileKind::Layout {
                            split: split1,
                            orientation: orientation1,
                            tiles: tiles1,
                            location: location1,
                            size: size1,
                        },
                        TileKind::Layout {
                            split: split2,
                            orientation: orientation2,
                            tiles: tiles2,
                            location: location2,
                            size: size2,
                        },
                    ) => {
                        if split1 != split2
                            || orientation1 != orientation2
                            || tiles1.len() != tiles2.len()
                            || location1 != location2
                            || size1 != size2
                        {
                            return false;
                        }

                        tiles1
                            .iter()
                            .zip(tiles2.iter())
                            .all(|(&child_a, &child_b)| {
                                compare_tiles(tree_a, child_a, tree_b, child_b)
                            })
                    }
                    _ => false,
                }
            }

            compare_tiles(self, self.root, other, other.root)
        }
    }

    impl TileTree<TestWindow> {
        fn visualize(&self) -> String {
            let mut output = String::new();
            self.format_node(self.root, "", true, true, &mut output);
            output
        }

        fn format_node(
            &self,
            id: TileId,
            prefix: &str,
            is_first: bool,
            is_last: bool,
            f: &mut String,
        ) {
            let marker = if is_first {
                ""
            } else if is_last {
                "└── "
            } else {
                "├── "
            };
            let tile = match self.arena.get(id) {
                Some(t) => t,
                None => {
                    f.push_str(&format!("{}{} [INVALID ID]\n", prefix, marker));
                    return;
                }
            };

            f.push_str(prefix);
            f.push_str(marker);

            match &tile.kind {
                TileKind::Window(window) => {
                    f.push_str(&format!(
                        "Window {}[loc: ({}, {}), size: ({}, {}), ratio: {}]\n",
                        if let Some(id) = window.inner.borrow().id {
                            id.to_string() + " "
                        } else {
                            "".to_string()
                        },
                        window.inner.borrow().location.x,
                        window.inner.borrow().location.y,
                        window.inner.borrow().size.w,
                        window.inner.borrow().size.h,
                        tile.ratio.0
                    ));
                }
                TileKind::Layout {
                    split,
                    orientation,
                    tiles,
                    location,
                    size,
                } => {
                    f.push_str(&format!(
                        "Layout [{:?}, {:?}, loc: ({}, {}), size: ({}, {}), ratio: {}]\n",
                        split, orientation, location.x, location.y, size.w, size.h, tile.ratio.0
                    ));

                    let new_prefix = format!(
                        "{}{}",
                        prefix,
                        if is_first {
                            ""
                        } else if is_last {
                            "    "
                        } else {
                            "│   "
                        }
                    );

                    for (i, child_id) in tiles.iter().enumerate() {
                        let last_child = i == tiles.len() - 1;
                        self.format_node(*child_id, &new_prefix, false, last_child, f);
                    }
                }
            }
        }
    }

    macro_rules! tile_tree {
        (@window_opt $window:ident $size:ident) => {};

        (@window_opt $window:ident $size:ident id: $id:expr $(, $($rest:tt)*)?) => {
            $window.inner.borrow_mut().id = Some($id);
            $(tile_tree!(@window_opt $window $size $($rest)*);)?
        };

        (@window_opt $window:ident $size:ident loc: $l:expr $(, $($rest:tt)*)?) => {
            $window.inner.borrow_mut().location = $l.into();
            $(tile_tree!(@window_opt $window $size $($rest)*);)?
        };

        (@window_opt $window:ident $size:ident size: $s:expr $(, $($rest:tt)*)?) => {
            $window.inner.borrow_mut().size = $s.into();
            $(tile_tree!(@window_opt $window $size $($rest)*);)?
        };

        (@window_opt $window:ident $ratio:ident ratio: $r:expr $(, $($rest:tt)*)?) => {
            $ratio = $r as f64;
            $(tile_tree!(@window_opt $window $ratio $($rest)*);)?
        };

        (@node $arena:ident, $parent:expr, window($($opts:tt)*)) => {{
            #[allow(unused_mut, unused_assignments)]
            let mut window = TestWindow::new();
            #[allow(unused_mut, unused_assignments)]
            let mut ratio = 1.0;
            tile_tree!(@window_opt window ratio $($opts)*);
            $arena.insert(Tile {
                kind: TileKind::Window(window),
                ratio: TileRatio(ratio),
                parent: $parent,
            })
        }};

        (@layout_opt $split:ident $orient:ident $loc:ident $size:ident $ratio:ident) => {};

        (@layout_opt $split:ident $orient:ident $loc:ident $size:ident $ratio:ident split: $s:ident $(, $($rest:tt)*)?) => {
            $split = TileSplit::$s;
            $(tile_tree!(@layout_opt $split $orient $loc $size $ratio $($rest)*);)?
        };

        (@layout_opt $split:ident $orient:ident $loc:ident $size:ident $ratio:ident orient: $o:ident $(, $($rest:tt)*)?) => {
            $orient = TileOrientation::$o;
            $(tile_tree!(@layout_opt $split $orient $loc $size $ratio $($rest)*);)?
        };

        (@layout_opt $split:ident $orient:ident $loc:ident $size:ident $ratio:ident loc: $l:expr $(, $($rest:tt)*)?) => {
            $loc = $l.into();
            $(tile_tree!(@layout_opt $split $orient $loc $size $ratio $($rest)*);)?
        };

        (@layout_opt $split:ident $orient:ident $loc:ident $size:ident $ratio:ident size: $s:expr $(, $($rest:tt)*)?) => {
            $size = $s.into();
            $(tile_tree!(@layout_opt $split $orient $loc $size $ratio $($rest)*);)?
        };

        (@layout_opt $split:ident $orient:ident $loc:ident $size:ident $ratio:ident ratio: $r:expr $(, $($rest:tt)*)?) => {
            $ratio = $r as f64;
            $(tile_tree!(@layout_opt $split $orient $loc $size $ratio $($rest)*);)?
        };

        (@node $arena:ident, $parent:expr, layout($($opts:tt)*) [ $($child_kind:ident ( $($child_args:tt)* ) $( [ $($child_inner:tt)* ] )? ),* $(,)? ]) => {{
            #[allow(unused_mut, unused_assignments)]
            let mut split = TileSplit::default();
            #[allow(unused_mut, unused_assignments)]
            let mut orient = TileOrientation::default();
            #[allow(unused_mut, unused_assignments)]
            let mut ratio = 1.0;
            #[allow(unused_mut, unused_assignments)]
            let mut location = (0, 0).into();
            #[allow(unused_mut, unused_assignments)]
            let mut size = (0, 0).into();
            tile_tree!(@layout_opt split orient location size ratio $($opts)*);

            let layout_id = $arena.insert(Tile {
                kind: TileKind::Layout {
                    split,
                    orientation: orient,
                    tiles: Vec::new(),
                    location,
                    size,
                },
                ratio: TileRatio(ratio),
                parent: $parent,
            });

            let children = vec![
                $(
                    tile_tree!(@node $arena, Some(layout_id), $child_kind ( $($child_args)* ) $( [ $($child_inner)* ] )?)
                ),*
            ];

            if let TileKind::Layout { ref mut tiles, .. } = $arena[layout_id].kind {
                *tiles = children;
            }
            layout_id
        }};

        ($kind:ident ( $($args:tt)* ) $( [ $($inner:tt)* ] )?) => {{
            let mut arena = slotmap::SlotMap::with_key();
            let root = tile_tree!(@node arena, None, $kind ( $($args)* ) $( [ $($inner)* ] )?);
            TileTree {
                arena,
                root,
                current_tile: root,
                layouts: Rc::new(LayoutSet(HashMap::new())),
                layout_name: "".to_string(),
                layout_trace: Vec::new(),
            }
        }};
    }

    macro_rules! node {
        (@opt $layout:ident $repeat:ident $ratio:ident) => {};
        (@opt $layout:ident $repeat:ident $ratio:ident repeat: $r:expr $(, $($rest:tt)*)?) => {
            $repeat = TileRepeat($r);
            $(node!(@opt $layout $repeat $ratio $($rest)*);)?
        };
        (@opt $layout:ident $repeat:ident $ratio:ident ratio: $s:expr $(, $($rest:tt)*)?) => {
            $ratio = $s;
            $(node!(@opt $layout $repeat $ratio $($rest)*);)?
        };

        (win $(, $($opts:tt)*)?) => {{
            #[allow(unused_mut, unused_assignments)]
            let mut layout = None;
            #[allow(unused_mut, unused_assignments)]
            let mut repeat = TileRepeat(1);
            #[allow(unused_mut, unused_assignments)]
            let mut ratio = 1;
            $(node!(@opt layout repeat ratio $($opts)*);)?
            LayoutNode { layout, repeat, ratio: TileRatio(ratio as f64) }
        }};

        (ref: $name:expr $(, $($opts:tt)*)?) => {{
            #[allow(unused_mut, unused_assignments)]
            let mut layout = Some($name.to_string());
            #[allow(unused_mut, unused_assignments)]
            let mut repeat = TileRepeat(1);
            #[allow(unused_mut, unused_assignments)]
            let mut ratio = 1;
            $(node!(@opt layout repeat ratio $($opts)*);)?
            LayoutNode { layout, repeat, ratio: TileRatio(ratio as f64) }
        }};
    }

    macro_rules! schema {
        (@opt $split:ident $orient:ident $nodes:ident) => {};
        (@opt $split:ident $orient:ident $nodes:ident split: $s:ident $(, $($rest:tt)*)?) => {
            $split = TileSplit::$s;
            $(schema!(@opt $split $orient $nodes $($rest)*);)?
        };
        (@opt $split:ident $orient:ident $nodes:ident orient: $o:ident $(, $($rest:tt)*)?) => {
            $orient = TileOrientation::$o;
            $(schema!(@opt $split $orient $nodes $($rest)*);)?
        };
        (@opt $split:ident $orient:ident $nodes:ident nodes: [ $($n:expr),* $(,)? ] $(, $($rest:tt)*)?) => {
            $nodes = vec![ $($n),* ];
            $(schema!(@opt $split $orient $nodes $($rest)*);)?
        };

        ($($opts:tt)*) => {{
            #[allow(unused_mut, unused_assignments)]
            let mut split = TileSplit::default();
            #[allow(unused_mut, unused_assignments)]
            let mut orient = TileOrientation::default();
            #[allow(unused_mut, unused_assignments)]
            let mut nodes = Vec::new();
            schema!(@opt split orient nodes $($opts)*);
            LayoutSchema { split, orientation: orient, nodes }
        }};
    }

    static LAYOUT_SET: LazyLock<HashMap<String, LayoutSchema>> = LazyLock::new(|| {
        HashMap::from_iter([
            ("empty".into(), schema!()),
            (
                "windows".into(),
                schema!(
                    nodes: [ node!(win, repeat: 3) ]
                ),
            ),
            (
                "layout".into(),
                schema!(
                    nodes: [ node!(ref: "windows") ]
                ),
            ),
            (
                "layouts".into(),
                schema!(
                    nodes: [
                        node!(ref: "windows", repeat: 2),
                    ]
                ),
            ),
            (
                "mix windows layouts".into(),
                schema!(
                    nodes: [
                        node!(win, repeat: 2),
                        node!(ref: "windows", repeat: 2),
                        node!(win, repeat: 2),
                        node!(ref: "windows", repeat: 2),
                    ]
                ),
            ),
            (
                "nested layout".into(),
                schema!(
                    nodes: [ node!(ref: "layout") ]
                ),
            ),
            (
                "double nested layout".into(),
                schema!(
                    nodes: [ node!(ref: "nested layout") ]
                ),
            ),
        ])
    });

    enum LayoutType {
        Ref(&'static str),
        New(Vec<(&'static str, LayoutSchema)>),
    }

    macro_rules! assert_tree_eq {
        ($tree:ident, $expected:ident) => {
            assert!(
                $tree == $expected,
                "\nExpected:\n{}\nGet:\n{}\n{:?}\n",
                $expected.visualize(),
                $tree.visualize(),
                $tree.layout_trace
            )
        };
    }

    #[test_case(
        LayoutType::Ref("empty"),
        1,
        tile_tree!(layout() []);
        "empty layout schema"
    )]
    #[test_case(
        LayoutType::Ref("windows"),
        1,
        tile_tree!(layout() [
            window()
        ]);
        "single window"
    )]
    #[test_case(
        LayoutType::Ref("windows"),
        3,
        tile_tree!(layout() [
            window(),
            window(),
            window(),
        ]);
        "multiple windows with repeat"
    )]
    #[test_case(
        LayoutType::New(vec![
            ("root", schema!(nodes: [
                node!(win, ratio: 1),
                node!(win, ratio: 2, repeat: 2),
                node!(win, ratio: 3),
            ]))
        ]),
        4,
        tile_tree!(layout() [
            window(ratio: 1),
            window(ratio: 2),
            window(ratio: 2),
            window(ratio: 3),
        ]);
        "windows with different ratios"
    )]
    #[test_case(
        LayoutType::New(vec![
            ("root", schema!(nodes: [
                node!(ref: "windows", ratio: 2),
            ]))
        ]),
        1,
        tile_tree!(layout() [
            layout(ratio: 2) [
                window()
            ]
        ]);
        "nested layout with ratio"
    )]
    #[test_case(
        LayoutType::Ref("layout"),
        1,
        tile_tree!(layout() [
            layout() [
                window()
            ]
        ]);
        "single nested layout"
    )]
    #[test_case(
        LayoutType::Ref("layouts"),
        5,
        tile_tree!(layout() [
            layout() [
                window(),
                window(),
                window(),
            ],
            layout() [
                window(),
                window(),
            ],
        ]);
        "multiple nested layouts with repeat"
    )]
    #[test_case(
        LayoutType::Ref("mix windows layouts"),
        14,
        tile_tree!(layout() [
            window(),
            window(),
            layout() [
                window(),
                window(),
                window(),
            ],
            layout() [
                window(),
                window(),
                window(),
            ],
            window(),
            window(),
            layout() [
                window(),
                window(),
                window(),
            ],
            layout() [
                window(),
            ],
        ]);
        "mixed windows and layouts with repeat"
    )]
    #[test_case(
        LayoutType::Ref("nested layout"),
        2,
        tile_tree!(layout() [
            layout() [
                layout() [
                    window(),
                    window(),
                ]
            ]
        ]);
        "deeply nested layouts"
    )]
    #[test_case(
        LayoutType::New(vec![
            ("root", schema!(nodes: [
                node!(ref: "sub", ratio: 3),
                node!(win, ratio: 1),
            ])),
            ("sub", schema!(nodes: [
                node!(win, repeat: 2),
            ]))
        ]),
        3,
        tile_tree!(layout() [
            layout(ratio: 3) [
                window(),
                window(),
            ],
            window(ratio: 1),
        ]);
        "nested layout with ratios and repeat"
    )]
    fn test_insert(layout_type: LayoutType, tiles: u32, expected: TileTree<TestWindow>) {
        let (layouts, layout_name) = match layout_type {
            LayoutType::Ref(name) => (LayoutSet((*LAYOUT_SET).clone()), name),
            LayoutType::New(iter) => (
                {
                    let mut set = (*LAYOUT_SET).clone();
                    set.extend(
                        iter.into_iter()
                            .map(|(name, schema)| (name.to_string(), schema)),
                    );
                    LayoutSet(set)
                },
                "root",
            ),
        };
        let mut tree = TileTree::<TestWindow>::new(Rc::new(layouts), layout_name);
        for _ in 0..tiles {
            tree.insert(TestWindow::new(), None);
        }
        assert_tree_eq!(tree, expected);
    }

    #[test]
    fn test_insert_override_ratio() {
        let mut tree = TileTree::<TestWindow>::new(
            Rc::new(LayoutSet(HashMap::from_iter([(
                "root".to_string(),
                schema!(nodes: [
                    node!(win, ratio: 3),
                    node!(win, ratio: 2),
                    node!(win, ratio: 1),
                ]),
            )]))),
            "root",
        );
        for i in 1..=3 {
            tree.insert(TestWindow::new(), Some(TileRatio(i as f64)));
        }
        let expected = tile_tree!(layout() [
            window(ratio: 1),
            window(ratio: 2),
            window(ratio: 3)
        ]);
        assert_tree_eq!(tree, expected);
    }

    #[test_case("empty", 1 => false; "empty layout rejects first window")]
    #[test_case("windows", 1 => true; "single window accepted")]
    #[test_case("windows", 3 => true; "windows within capacity accepted")]
    #[test_case("windows", 4 => false; "windows beyond capacity rejected")]
    #[test_case("layouts", 7 => false; "layouts beyond capacity rejected")]
    #[test_case("mix windows layouts", 17 => false; "mixed layout beyond capacity rejected")]
    #[test_case("nested layout", 4 => false; "nested layout beyond capacity rejected")]
    fn test_reject_insertion(layout_name: &str, tiles: u32) -> bool {
        let layouts = Rc::new(LayoutSet((*LAYOUT_SET).clone()));
        let mut tree = TileTree::new(layouts, layout_name);
        for i in 0..tiles - 1 {
            assert!(
                tree.insert(TestWindow::new(), None).is_none(),
                "Number {:?} insert failed",
                i
            );
        }
        tree.insert(TestWindow::new(), None).is_none()
    }

    #[test_case(
        "windows",
        1,
        0,
        tile_tree!(layout() []);
        "remove only window - empty tree"
    )]
    #[test_case(
        "windows",
        3,
        0,
        tile_tree!(layout() [
            window(id: 1),
            window(id: 2),
        ]);
        "remove first window from multiple"
    )]
    #[test_case(
        "windows",
        3,
        1,
        tile_tree!(layout() [
            window(id: 0),
            window(id: 2),
        ]);
        "remove middle window"
    )]
    #[test_case(
        "windows",
        3,
        2,
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]);
        "remove last window"
    )]
    #[test_case(
        "layout",
        3,
        2,
        tile_tree!(layout() [
            layout() [
                window(id: 0),
                window(id: 1),
            ]
        ]);
        "remove window from nested layout"
    )]
    #[test_case(
        "layouts",
        4,
        3,
        tile_tree!(layout() [
            layout() [
                window(id: 0),
                window(id: 1),
                window(id: 2),
            ],
        ]);
        "remove last window - layout pruned"
    )]
    #[test_case(
        "layouts",
        4,
        0,
        tile_tree!(layout() [
            layout() [
                window(id: 1),
                window(id: 2),
                window(id: 3),
            ],
        ]);
        "remove first window from first layout"
    )]
    #[test_case(
        "layouts",
        5,
        3,
        tile_tree!(layout() [
            layout() [
                window(id: 0),
                window(id: 1),
                window(id: 2),
            ],
            layout() [
                window(id: 4),
            ],
        ]);
        "remove window from second layout"
    )]
    #[test_case(
        "nested layout",
        3,
        2,
        tile_tree!(layout() [
            layout() [
                layout() [
                    window(id: 0),
                    window(id: 1),
                ]
            ]
        ]);
        "remove from deeply nested layout"
    )]
    #[test_case(
        "nested layout",
        1,
        0,
        tile_tree!(layout() []);
        "remove from nested layout - empty tree"
    )]
    fn test_remove(layout_name: &str, tiles: u32, remove: u32, expected: TileTree<TestWindow>) {
        let layouts = Rc::new(LayoutSet((*LAYOUT_SET).clone()));
        let mut tree = TileTree::<TestWindow>::new(layouts, layout_name);
        for i in 0..tiles {
            tree.insert(
                TestWindow {
                    inner: Rc::new(RefCell::new(TestWindowInner {
                        id: Some(i),
                        ..Default::default()
                    })),
                },
                None,
            );
        }
        assert_eq!(
            tree.remove(remove).map(|t| t.inner.borrow().id),
            Some(Some(remove))
        );
        assert_tree_eq!(tree, expected);
    }

    #[test_case(
        tile_tree!(layout() [
            window(),
            window(),
        ]),
        tile_tree!(layout(loc: (0, 0), size: (100, 100)) [
            window(loc: (0, 0), size: (50, 100)),
            window(loc: (50, 0), size: (50, 100)),
        ]);
        "two windows split evenly"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(ratio: 1),
            window(ratio: 3),
            window(ratio: 2),
            window(ratio: 5),
            window(ratio: 4),
        ]),
        tile_tree!(layout(loc: (0, 0), size: (100, 100)) [
            window(ratio: 1, loc: (0, 0), size: (7, 100)),
            window(ratio: 3, loc: (7, 0), size: (20, 100)),
            window(ratio: 2, loc: (27, 0), size: (13, 100)),
            window(ratio: 5, loc: (40, 0), size: (33, 100)),
            window(ratio: 4, loc: (73, 0), size: (27, 100)),
        ]);
        "windows with different ratios"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(),
            window(),
        ]),
        tile_tree!(layout(split: Horizontal, loc: (0, 0), size: (100, 100)) [
            window(loc: (0, 0), size: (100, 50)),
            window(loc: (0, 50), size: (100, 50)),
        ]);
        "horizontal split layout"
    )]
    #[test_case(
        tile_tree!(layout(orient: TopLeft) [
            window(),
            window(),
        ]),
        tile_tree!(layout(orient: TopLeft, loc: (0, 0), size: (100, 100)) [
            window(loc: (50, 0), size: (50, 100)),
            window(loc: (0, 0), size: (50, 100)),
        ]);
        "layout with top-left orientation"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(),
            layout(split: Horizontal, orient: TopLeft) [
                window(ratio: 2),
                layout(split: Horizontal) [
                    window(),
                    window(),
                ]
            ]
        ]),
        tile_tree!(layout(loc: (0, 0), size: (100, 100)) [
            window(loc: (0, 0), size: (50, 100)),
            layout(split: Horizontal, orient: TopLeft, loc: (50, 0), size: (50, 100)) [
                window(loc: (50, 33), size: (50, 67), ratio: 2),
                layout(split: Horizontal, loc: (50, 0), size: (50, 33)) [
                    window(loc: (50, 0), size: (50, 17)),
                    window(loc: (50, 17), size: (50, 16)),
                ]
            ]
        ]);
        "nested layouts with mixed split and orientation"
    )]
    fn test_update_window_size(mut tree: TileTree<TestWindow>, expected: TileTree<TestWindow>) {
        tree.update_window_size((0, 0).into(), (100, 100).into());
        assert_tree_eq!(tree, expected);
    }

    #[test_case(
        tile_tree!(layout() [window(id: 0)]),
        0,
        WindowDirection::Up,
        vec![];
        "single window - no neighbors"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        WindowDirection::Up,
        vec![0];
        "horizontal split - navigate up"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        WindowDirection::Down,
        vec![1];
        "horizontal split - navigate down"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        WindowDirection::Left,
        vec![0];
        "vertical split - navigate left"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        WindowDirection::Right,
        vec![1];
        "vertical split - navigate right"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            layout() [
                window(id: 0),
                window(id: 1),
                window(id: 2),
            ],
            layout() [
                window(id: 3),
                window(id: 4),
            ]
        ]),
        3,
        WindowDirection::Up,
        vec![0, 1];
        "nested rows - navigate up from bottom row"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            layout() [
                window(id: 0),
                window(id: 1),
                window(id: 2),
            ],
            layout() [
                window(id: 3),
                window(id: 4),
            ]
        ]),
        1,
        WindowDirection::Down,
        vec![3, 4];
        "nested rows - navigate down from top row"
    )]
    #[test_case(
        tile_tree!(layout() [
            layout(split: Horizontal) [
                window(id: 0),
                window(id: 1),
                window(id: 2),
            ],
            layout(split: Horizontal) [
                window(id: 3),
                window(id: 4),
            ]
        ]),
        4,
        WindowDirection::Left,
        vec![1, 2];
        "nested columns - navigate left from right column"
    )]
    #[test_case(
        tile_tree!(layout() [
            layout(split: Horizontal) [
                window(id: 0),
                window(id: 1),
                window(id: 2),
            ],
            layout(split: Horizontal) [
                window(id: 3),
                window(id: 4),
            ]
        ]),
        2,
        WindowDirection::Right,
        vec![4];
        "nested columns - navigate right from left column"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
            window(id: 2),
            window(id: 3),
        ]),
        1,
        WindowDirection::Right,
        vec![2, 3];
        "multiple windows in direction"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        WindowDirection::Up,
        vec![];
        "horizontal split - top window has no up neighbor"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        WindowDirection::Left,
        vec![];
        "horizontal split - left has no meaning"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
            window(id: 2),
            window(id: 3),
        ]),
        1,
        WindowDirection::Down,
        vec![2, 3];
        "navigate down skips immediate neighbor when multiple exist"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            layout(split: Horizontal) [
                window(id: 1),
                window(id: 2),
            ],
        ]),
        0,
        WindowDirection::Right,
        vec![1, 2];
        "navigate right into nested vertical stack"
    )]
    #[test_case(
        tile_tree!(layout() [
            layout(split: Horizontal) [
                layout() [
                    window(id: 0),
                    window(id: 1),
                ]
            ],
            window(id: 2),
        ]),
        0,
        WindowDirection::Right,
        vec![1, 2];
        "navigate right through multiple layout levels"
    )]
    #[test_case(
        tile_tree!(layout(orient: TopLeft) [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        WindowDirection::Left,
        vec![1];
        "orientation TopLeft - left neighbor"
    )]
    fn test_find_windows_in_direction(
        mut tree: TileTree<TestWindow>,
        id: u32,
        direction: WindowDirection,
        expected: Vec<u32>,
    ) {
        tree.update_window_size((0, 0).into(), (100, 100).into());
        println!("{}", tree.visualize());
        assert_eq!(
            tree.find_windows_in_direction(id, direction)
                .iter()
                .map(|w| w.inner.borrow().id.unwrap())
                .collect::<Vec<_>>(),
            expected
        );
    }

    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        1,
        tile_tree!(layout() [
            window(id: 1),
            window(id: 0),
        ]);
        "swap adjacent windows"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
            window(id: 2),
            layout() [
                layout() [
                    window(id: 3)
                ]
            ]
        ]),
        0,
        3,
        tile_tree!(layout() [
            window(id: 3),
            window(id: 1),
            window(id: 2),
            layout() [
                layout() [
                    window(id: 0)
                ]
            ]
        ]);
        "swap across nested layouts"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
            window(id: 2),
        ]),
        0,
        2,
        tile_tree!(layout() [
            window(id: 2),
            window(id: 1),
            window(id: 0),
        ]);
        "swap first and last window"
    )]
    #[test_case(
        tile_tree!(layout() [
            layout() [
                window(id: 0),
                window(id: 1),
            ],
            layout() [
                window(id: 2),
                window(id: 3),
            ],
        ]),
        1,
        2,
        tile_tree!(layout() [
            layout() [
                window(id: 0),
                window(id: 2),
            ],
            layout() [
                window(id: 1),
                window(id: 3),
            ],
        ]);
        "swap windows between sibling layouts"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            layout() [
                window(id: 1),
                layout() [
                    window(id: 2),
                ]
            ]
        ]),
        0,
        2,
        tile_tree!(layout() [
            window(id: 2),
            layout() [
                window(id: 1),
                layout() [
                    window(id: 0),
                ]
            ]
        ]);
        "swap windows at different nesting levels"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        0,
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]);
        "swap window with itself - no change"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0, ratio: 2),
            window(id: 1, ratio: 3),
        ]),
        0,
        1,
        tile_tree!(layout() [
            window(id: 1, ratio: 2),
            window(id: 0, ratio: 3),
        ]);
        "swap windows preserving ratios"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        1,
        tile_tree!(layout(split: Horizontal) [
            window(id: 1),
            window(id: 0),
        ]);
        "swap in horizontal split layout"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        99,
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]);
        "swap with invalid id - no change"
    )]
    fn test_swap_window(
        mut tree: TileTree<TestWindow>,
        lhs: u32,
        rhs: u32,
        expected: TileTree<TestWindow>,
    ) {
        tree.swap_window(lhs, rhs);
        assert_tree_eq!(tree, expected)
    }

    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
        ]),
        0,
        WindowDirection::Right,
        10,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (200, 100)),
        ]);
        "resize single window - no change"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        WindowDirection::Left,
        10,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (90, 100), ratio: 0.9),
            window(id: 1, loc: (90, 0), size: (110, 100), ratio: 1.1),
        ]);
        "resize window left - shrinks left neighbor"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        WindowDirection::Right,
        10,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (110, 100), ratio: 1.1),
            window(id: 1, loc: (110, 0), size: (90, 100), ratio: 0.9),
        ]);
        "resize window right - shrinks right neighbor"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        WindowDirection::Up,
        10,
        tile_tree!(layout(split: Horizontal, loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (200, 40), ratio: 0.8),
            window(id: 1, loc: (0, 40), size: (200, 60), ratio: 1.2),
        ]);
        "resize window up - shrinks top neighbor"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        WindowDirection::Left,
        -10,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (110, 100), ratio: 1.1),
            window(id: 1, loc: (110, 0), size: (90, 100), ratio: 0.9),
    ]);
    "resize window left with negative offset - grows left neighbor"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        WindowDirection::Left,
        10,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (90, 100), ratio: 0.9),
            window(id: 1, loc: (90, 0), size: (110, 100), ratio: 1.1),
        ]);
        "resize window left at edge - grows right neighbor"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        WindowDirection::Right,
        10,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (110, 100), ratio: 1.1),
            window(id: 1, loc: (110, 0), size: (90, 100), ratio: 0.9),
        ]);
        "resize window right at edge - grows left neighbor"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        WindowDirection::Up,
        10,
        tile_tree!(layout(split: Horizontal, loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (200, 40), ratio: 0.8),
            window(id: 1, loc: (0, 40), size: (200, 60), ratio: 1.2),
        ]);
        "resize window up at edge - grows bottom neighbor"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            layout() [
                window(id: 1),
                window(id: 2),
            ]
        ]),
        2,
        WindowDirection::Down,
        10,
        tile_tree!(layout(split: Horizontal, loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (200, 60), ratio: 1.2),
            layout(loc: (0, 60), size: (200, 40), ratio: 0.8) [
                window(id: 1, loc: (0, 60), size: (100, 40)),
                window(id: 2, loc: (100, 60), size: (100, 40)),
            ]
        ]);
        "resize window down at edge - grows top neighbor"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            layout() [
                window(id: 1),
                window(id: 2),
            ]
        ]),
        1,
        WindowDirection::Up,
        10,
        tile_tree!(layout(split: Horizontal, loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (200, 40), ratio: 0.8),
            layout(loc: (0, 40), size: (200, 60), ratio: 1.2) [
                window(id: 1, loc: (0, 40), size: (100, 60)),
                window(id: 2, loc: (100, 40), size: (100, 60)),
            ]
        ]);
        "resize window up - expands parent layout"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
            window(id: 2),
        ]),
        1,
        WindowDirection::Left,
        10,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (57, 100), ratio: 0.85),
            window(id: 1, loc: (57, 0), size: (76, 100), ratio: 1.15),
            window(id: 2, loc: (133, 0), size: (67, 100), ratio: 1),
        ]);
        "resize middle window left - only affects left neighbor"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            layout(split: Horizontal) [
                window(id: 1),
                window(id: 2),
            ],
            layout(split: Horizontal) [
                window(id: 3),
                window(id: 4),
                window(id: 5),
            ]
        ]),
        3,
        WindowDirection::Left,
        10,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (67, 100)),
            layout(split: Horizontal, loc: (67, 0), size: (57, 100), ratio: 0.85) [
                window(id: 1, loc: (67, 0), size: (57, 50), ratio: 1),
                window(id: 2, loc: (67, 50), size: (57, 50), ratio: 1),
            ],
            layout(split: Horizontal, loc: (124, 0), size: (76, 100), ratio: 1.15) [
                window(id: 3, loc: (124, 0), size: (76, 34), ratio: 1),
                window(id: 4, loc: (124, 34), size: (76, 33), ratio: 1),
                window(id: 5, loc: (124, 67), size: (76, 33), ratio: 1),
            ]
        ]);
        "resize window left - affects multiple parent layouts"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            layout(split: Horizontal) [
                layout(split: Vertical) [
                    layout(split: Horizontal) [
                        window(id: 1),
                        window(id: 2),
                    ]
                ]
            ],
        ]),
        2,
        WindowDirection::Left,
        10,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (90, 100), ratio: 0.9),
            layout(split: Horizontal, loc: (90, 0), size: (110, 100), ratio: 1.1) [
                layout(loc: (90, 0), size: (110, 100)) [
                    layout(split: Horizontal, loc: (90, 0), size: (110, 100)) [
                        window(id: 1, loc: (90, 0), size: (110, 50), ratio: 1),
                        window(id: 2, loc: (90, 50), size: (110, 50), ratio: 1),
                    ]
                ]
            ],
        ]);
        "resize deeply nested window - propagates through multiple layout levels"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal, orient: TopLeft) [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        WindowDirection::Down,
        10,
        tile_tree!(layout(split: Horizontal, orient: TopLeft, loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 60), size: (200, 40), ratio: 0.8),
            window(id: 1, loc: (0, 0), size: (200, 60), ratio: 1.2),
        ]);
        "resize window down with reverse orientation"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        WindowDirection::Left,
        150,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (0, 100), ratio: 0),
            window(id: 1, loc: (0, 0), size: (200, 100), ratio: 2.5),
        ]);
        "resize window beyond neighbor minimum - clamped"
    )]
    fn test_resize_tile(
        mut tree: TileTree<TestWindow>,
        id: u32,
        edge: WindowDirection,
        px: i32,
        expected: TileTree<TestWindow>,
    ) {
        tree.update_window_size((0, 0).into(), (200, 100).into());
        tree.resize_tile(id, edge, WindowUnit::Px(px));
        tree.update_window_size((0, 0).into(), (200, 100).into());
        assert_tree_eq!(tree, expected)
    }
}
