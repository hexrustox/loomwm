use crate::{
    input::WindowUnit,
    utils::{Direction, floats_to_ints},
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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
    fn set_location(&mut self, location: Point<i32, Logical>);
    fn get_size(&self) -> Size<i32, Logical>;
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

#[derive(Debug, Default, Clone, Copy, Deserialize, PartialEq)]
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

#[derive(Debug, Clone)]
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

#[derive(Clone, Copy)]
pub enum TileResizeUnit {
    Exact(i32),
    Px(i32),
    Ratio(f64),
}

impl From<i32> for TileResizeUnit {
    fn from(value: i32) -> Self {
        Self::Exact(value)
    }
}

impl From<WindowUnit> for TileResizeUnit {
    fn from(value: WindowUnit) -> Self {
        match value {
            WindowUnit::Px(x) => Self::Px(x),
            WindowUnit::Ratio(x) => Self::Ratio(x.0),
        }
    }
}

impl Neg for TileResizeUnit {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Self::Exact(_) => {
                panic!("Cannot resize to negative size")
            }
            Self::Px(px) => Self::Px(-px),
            Self::Ratio(r) => Self::Ratio(-r),
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

    // TODO improve efficiency
    pub fn update_tile_size(&mut self, location: Point<i32, Logical>, size: Size<i32, Logical>) {
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
                    TileSplit::Vertical => location.x += t_len,
                    TileSplit::Horizontal => location.y += t_len,
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

    // TODO improve efficiency
    pub fn find_windows_in_direction<'a>(
        &self,
        key: impl SearchKey<'a>,
        direction: Direction,
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

            fn is_in_direction(&self, direction: Direction, target: &Rect) -> bool {
                direction.contains(Direction::TOP)
                    && self.bottom() <= target.top()
                    && self.overlaps_horizontally(target)
                    || direction.contains(Direction::BOTTOM)
                        && self.top() >= target.bottom()
                        && self.overlaps_horizontally(target)
                    || direction.contains(Direction::LEFT)
                        && self.right() <= target.left()
                        && self.overlaps_vertically(target)
                    || direction.contains(Direction::RIGHT)
                        && self.left() >= target.right()
                        && self.overlaps_vertically(target)
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
        direction: Direction,
        unit: TileResizeUnit,
        in_loop: bool,
    ) {
        #[cfg(test)]
        assert!(direction.bits().count_ones() == 1);

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

        let offset = if direction.intersects(Direction::BOTTOM_RIGHT) {
            1
        } else {
            -1
        } * if matches!(orientation, TileOrientation::BottomRight) {
            1
        } else {
            -1
        };

        let child_index = tiles
            .iter()
            .position(|t| *t == child_id)
            .expect("Cannot find tile in its parent") as i32;

        let other_index = child_index + offset;

        let in_bounds = (0..tiles.len() as i32).contains(&other_index);
        if !in_bounds {
            if let Some((ancestor_id, refer_id)) = self.find_ancestor_with_split(parent_id, *split)
            {
                self.adjust_adjacent_ratios(ancestor_id, refer_id, direction, unit, false);
            } else if !in_loop && !matches!(unit, TileResizeUnit::Exact(..)) {
                self.adjust_adjacent_ratios(parent_id, child_id, direction.opposite(), -unit, true);
            }
            return;
        }

        let other_id = tiles[other_index as usize];

        let total_ratio = tiles
            .iter()
            .fold(0.0, |acc, id| acc + self.arena[*id].ratio.0);

        let size_component = match split {
            TileSplit::Vertical => size.w,
            TileSplit::Horizontal => size.h,
        } as f64;

        let length_from_direction = |direction: Direction, s: Size<i32, Logical>| {
            if direction.intersects(Direction::LEFT | Direction::RIGHT) {
                s.w
            } else {
                s.h
            }
        };

        let kind_size = |k: &TileKind<T>| match k {
            TileKind::Window(w) => w.get_size(),
            TileKind::Layout { size, .. } => *size,
        };

        match unit {
            TileResizeUnit::Exact(new_tile_length) => {
                let tile_size = kind_size(&self.arena[child_id].kind);
                let other_size = kind_size(&self.arena[other_id].kind);

                let tile_length = length_from_direction(direction, tile_size);
                let other_length = length_from_direction(direction, other_size);

                let tile_ratio = self.arena[child_id].ratio.0;
                let other_ratio = self.arena[other_id].ratio.0;

                let total_length = tile_length + other_length;
                let pair_total_ratio = tile_ratio + other_ratio;

                let new_tile_length = new_tile_length.clamp(0, total_length);
                let new_other_length = total_length - new_tile_length;

                let new_tile_ratio =
                    new_tile_length as f64 / total_length as f64 * pair_total_ratio;
                let new_other_ratio =
                    new_other_length as f64 / total_length as f64 * pair_total_ratio;

                self.arena[child_id].ratio.0 = new_tile_ratio;
                self.arena[other_id].ratio.0 = new_other_ratio;
            }
            TileResizeUnit::Px(px) => {
                let ratio_offset = px as f64 * total_ratio / size_component;
                let ratio_offset =
                    ratio_offset.clamp(-self.arena[child_id].ratio.0, self.arena[other_id].ratio.0);

                self.arena[child_id].ratio.0 += ratio_offset;
                self.arena[other_id].ratio.0 -= ratio_offset;
            }
            TileResizeUnit::Ratio(r) => {
                let ratio_offset =
                    r.clamp(-self.arena[child_id].ratio.0, self.arena[other_id].ratio.0);

                self.arena[child_id].ratio.0 += ratio_offset;
                self.arena[other_id].ratio.0 -= ratio_offset;
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
        direction: Direction,
        unit: impl Into<TileResizeUnit>,
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

        let split = *split;
        let unit = unit.into();
        if direction.intersects(Direction::TOP | Direction::BOTTOM) {
            match split {
                TileSplit::Vertical => {
                    if let Some((ancestor_id, refer_id)) =
                        self.find_ancestor_with_split(window_id, TileSplit::Horizontal)
                    {
                        self.adjust_adjacent_ratios(ancestor_id, refer_id, direction, unit, false);
                    }
                }
                TileSplit::Horizontal => {
                    self.adjust_adjacent_ratios(parent_id, window_id, direction, unit, false);
                }
            }
        }
        if direction.intersects(Direction::LEFT | Direction::RIGHT) {
            match split {
                TileSplit::Vertical => {
                    self.adjust_adjacent_ratios(parent_id, window_id, direction, unit, false);
                }
                TileSplit::Horizontal => {
                    if let Some((ancestor_id, refer_id)) =
                        self.find_ancestor_with_split(window_id, TileSplit::Vertical)
                    {
                        self.adjust_adjacent_ratios(ancestor_id, refer_id, direction, unit, false);
                    }
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

        fn set_location(&mut self, location: Point<i32, Logical>) {
            self.inner.borrow_mut().location = location;
        }

        fn get_size(&self) -> Size<i32, Logical> {
            self.inner.borrow().size
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

                const PRECISION: f64 = 100.;
                if (tile_a.ratio.0 * PRECISION).round() / PRECISION
                    != (tile_b.ratio.0 * PRECISION).round() / PRECISION
                {
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
        "single window becomes empty"
    )]
    #[test_case(
        "windows",
        3,
        0,
        tile_tree!(layout() [
            window(id: 1),
            window(id: 2),
        ]);
        "three windows remove first"
    )]
    #[test_case(
        "windows",
        3,
        1,
        tile_tree!(layout() [
            window(id: 0),
            window(id: 2),
        ]);
        "three windows remove middle"
    )]
    #[test_case(
        "windows",
        3,
        2,
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]);
        "three windows remove last"
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
        "nested layout single remove one"
    )]
    #[test_case(
        "nested layout",
        1,
        0,
        tile_tree!(layout() []);
        "nested layout single becomes empty"
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
        "multi layout remove from first"
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
        "multi layout remove from second"
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
        "multi layout prune empty layout"
    )]
    #[test_case(
        "layouts",
        6,
        3,
        tile_tree!(layout() [
            layout() [
                window(id: 0),
                window(id: 1),
                window(id: 2),
            ],
            layout() [
                window(id: 4),
                window(id: 5),
            ],
        ]);
        "multi layout remove first from second"
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
        "deeply nested three levels"
    )]
    #[test_case(
        "mix windows layouts",
        8,
        3,
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
            layout() [
                window(id: 2),
                window(id: 4),
                window(id: 5),
            ],
            layout() [
                window(id: 6),
                window(id: 7),
            ],
        ]);
        "mixed windows and layouts"
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

    #[test]
    fn test_remove_nonexistent_window() {
        let layouts = Rc::new(LayoutSet((*LAYOUT_SET).clone()));
        let mut tree = TileTree::<TestWindow>::new(layouts, "windows");

        for i in 0..2 {
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

        assert_eq!(tree.remove(99), None);

        let expected = tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]);
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
        "two windows vertical split"
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
        "two windows horizontal split"
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
        "two windows top left orientation"
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
        "five windows different ratios"
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
        "nested layouts mixed split and orientation"
    )]
    fn test_update_tile_size(mut tree: TileTree<TestWindow>, expected: TileTree<TestWindow>) {
        tree.update_tile_size((0, 0).into(), (100, 100).into());
        assert_tree_eq!(tree, expected);
    }

    #[test_case(
        tile_tree!(layout() [window(id: 0)]),
        0,
        Direction::TOP,
        vec![];
        "single window no neighbors"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        Direction::TOP,
        vec![];
        "horizontal split top has no up neighbor"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        Direction::LEFT,
        vec![];
        "horizontal split left no meaning"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        Direction::BOTTOM,
        vec![1];
        "horizontal split two windows down"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        Direction::TOP,
        vec![0];
        "horizontal split two windows up"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        Direction::RIGHT,
        vec![1];
        "vertical split two windows right"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        Direction::LEFT,
        vec![0];
        "vertical split two windows left"
    )]
    #[test_case(
        tile_tree!(layout(orient: TopLeft) [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        Direction::LEFT,
        vec![1];
        "two windows TopLeft orientation left"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
            window(id: 2),
            window(id: 3),
        ]),
        1,
        Direction::RIGHT,
        vec![2, 3];
        "four windows multiple in direction"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
            window(id: 2),
            window(id: 3),
        ]),
        1,
        Direction::BOTTOM,
        vec![2, 3];
        "four horizontal skip neighbor multiple below"
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
        Direction::RIGHT,
        vec![4];
        "nested columns right from left column"
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
        Direction::LEFT,
        vec![1, 2];
        "nested columns left from right column"
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
        Direction::BOTTOM,
        vec![3, 4];
        "nested rows down from top row"
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
        Direction::TOP,
        vec![0, 1];
        "nested rows up from bottom row"
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
        Direction::RIGHT,
        vec![1, 2];
        "navigate into nested layout"
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
        Direction::RIGHT,
        vec![1, 2];
        "navigate through multiple layout levels"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            layout(orient: TopLeft, split: Horizontal) [
                window(id: 1),
                window(id: 2),
            ],
        ]),
        0,
        Direction::RIGHT,
        vec![1, 2];
        "navigate into TopLeft nested layout"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        Direction::TOP_LEFT,
        vec![];
        "composite direction no neighbor"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        Direction::BOTTOM_LEFT,
        vec![0];
        "composite direction finds window"
    )]
    #[test_case(
        tile_tree!(layout() [
            layout(split: Horizontal) [
                layout() [
                    layout(split: Horizontal) [
                        window(id: 0),
                        window(id: 1),
                    ]
                ],
                window(id: 2),
            ],
            window(id: 3),
        ]),
        0,
        Direction::BOTTOM,
        vec![1, 2];
        "deep nesting three levels down"
    )]
    fn test_find_windows_in_direction(
        mut tree: TileTree<TestWindow>,
        id: u32,
        direction: Direction,
        expected: Vec<u32>,
    ) {
        tree.update_tile_size((0, 0).into(), (100, 100).into());
        println!("{}", tree.visualize());
        assert_eq!(
            tree.find_windows_in_direction(id, direction)
                .iter()
                .map(|w| w.inner.borrow().id.unwrap())
                .collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn test_find_windows_in_direction_nonexistent_id() {
        let mut tree = tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]);
        tree.update_tile_size((0, 0).into(), (100, 100).into());

        let result: Vec<u32> = tree
            .find_windows_in_direction(99, Direction::RIGHT)
            .iter()
            .map(|w| w.inner.borrow().id.unwrap())
            .collect();

        assert_eq!(result, Vec::<u32>::new());
    }

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
        "swap same window no change"
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
        "swap invalid id no change"
    )]
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
        "swap adjacent two windows"
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
        "swap two windows horizontal split"
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
        "swap first and last three windows"
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
        "swap windows preserve ratios"
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
        "swap different nesting levels"
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
        "swap between sibling layouts"
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
        "swap across deeply nested layout"
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
        Direction::RIGHT,
        10,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (200, 100)),
        ]);
        "resize single window no change"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        Direction::RIGHT,
        10,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (110, 100), ratio: 1.1),
            window(id: 1, loc: (110, 0), size: (90, 100), ratio: 0.9),
        ]);
        "resize two windows right shrinks right neighbor"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        Direction::LEFT,
        10,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (90, 100), ratio: 0.9),
            window(id: 1, loc: (90, 0), size: (110, 100), ratio: 1.1),
        ]);
        "resize two windows left shrinks left neighbor"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        Direction::LEFT,
        -10,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (110, 100), ratio: 1.1),
            window(id: 1, loc: (110, 0), size: (90, 100), ratio: 0.9),
        ]);
        "resize two windows negative offset grows left neighbor"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        Direction::LEFT,
        10,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (90, 100), ratio: 0.9),
            window(id: 1, loc: (90, 0), size: (110, 100), ratio: 1.1),
        ]);
        "resize two windows left direction grows other neighbor"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        Direction::RIGHT,
        10,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (110, 100), ratio: 1.1),
            window(id: 1, loc: (110, 0), size: (90, 100), ratio: 0.9),
        ]);
        "resize two windows right direction grows other neighbor"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        Direction::TOP,
        10,
        tile_tree!(layout(split: Horizontal, loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (200, 40), ratio: 0.8),
            window(id: 1, loc: (0, 40), size: (200, 60), ratio: 1.2),
        ]);
        "resize horizontal split up shrinks top neighbor"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        Direction::TOP,
        10,
        tile_tree!(layout(split: Horizontal, loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (200, 40), ratio: 0.8),
            window(id: 1, loc: (0, 40), size: (200, 60), ratio: 1.2),
        ]);
        "resize horizontal split top direction grows bottom neighbor"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
            window(id: 2),
        ]),
        1,
        Direction::LEFT,
        10,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (57, 100), ratio: 0.85),
            window(id: 1, loc: (57, 0), size: (76, 100), ratio: 1.15),
            window(id: 2, loc: (133, 0), size: (67, 100), ratio: 1),
        ]);
        "resize three windows middle affects one neighbor"
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
        Direction::TOP,
        10,
        tile_tree!(layout(split: Horizontal, loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (200, 40), ratio: 0.8),
            layout(loc: (0, 40), size: (200, 60), ratio: 1.2) [
                window(id: 1, loc: (0, 40), size: (100, 60)),
                window(id: 2, loc: (100, 40), size: (100, 60)),
            ]
        ]);
        "resize nested layout up expands parent"
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
        Direction::BOTTOM,
        10,
        tile_tree!(layout(split: Horizontal, loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (200, 60), ratio: 1.2),
            layout(loc: (0, 60), size: (200, 40), ratio: 0.8) [
                window(id: 1, loc: (0, 60), size: (100, 40)),
                window(id: 2, loc: (100, 60), size: (100, 40)),
            ]
        ]);
        "resize nested layout down grows top neighbor"
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
        Direction::LEFT,
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
        "resize affects multiple parent layouts"
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
        Direction::LEFT,
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
        "resize deep nesting propagates levels"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal, orient: TopLeft) [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        Direction::BOTTOM,
        10,
        tile_tree!(layout(split: Horizontal, orient: TopLeft, loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 60), size: (200, 40), ratio: 0.8),
            window(id: 1, loc: (0, 0), size: (200, 60), ratio: 1.2),
        ]);
        "resize reverse orientation"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        Direction::LEFT,
        150,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (0, 100), ratio: 0),
            window(id: 1, loc: (0, 0), size: (200, 100), ratio: 2),
        ]);
        "resize beyond minimum clamped"
    )]
    fn test_resize_tile(
        mut tree: TileTree<TestWindow>,
        id: u32,
        direction: Direction,
        px: i32,
        expected: TileTree<TestWindow>,
    ) {
        tree.update_tile_size((0, 0).into(), (200, 100).into());
        tree.resize_tile(id, direction, WindowUnit::Px(px));
        tree.update_tile_size((0, 0).into(), (200, 100).into());
        assert_tree_eq!(tree, expected)
    }

    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        Direction::RIGHT,
        0.1,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (110, 100), ratio: 1.1),
            window(id: 1, loc: (110, 0), size: (90, 100), ratio: 0.9),
        ]);
        "resize ratio increases window size"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        Direction::RIGHT,
        -0.1,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (90, 100), ratio: 0.9),
            window(id: 1, loc: (90, 0), size: (110, 100), ratio: 1.1),
        ]);
        "resize ratio decreases window size"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        Direction::BOTTOM,
        0.2,
        tile_tree!(layout(split: Horizontal, loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (200, 60), ratio: 1.2),
            window(id: 1, loc: (0, 60), size: (200, 40), ratio: 0.8),
        ]);
        "resize ratio horizontal split down"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0, ratio: 1),
            window(id: 1, ratio: 1),
            window(id: 2, ratio: 1),
        ]),
        1,
        Direction::LEFT,
        0.1,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (60, 100), ratio: 0.9),
            window(id: 1, loc: (60, 0), size: (73, 100), ratio: 1.1),
            window(id: 2, loc: (133, 0), size: (67, 100), ratio: 1),
        ]);
        "resize ratio three windows middle left"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            layout(split: Horizontal) [
                window(id: 1),
                window(id: 2),
            ],
        ]),
        1,
        Direction::TOP,
        0.15,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (100, 100), ratio: 1),
            layout(split: Horizontal, loc: (100, 0), size: (100, 100), ratio: 1) [
                window(id: 1, loc: (100, 0), size: (100, 43), ratio: 0.85),
                window(id: 2, loc: (100, 43), size: (100, 57), ratio: 1.15),
            ],
        ]);
        "resize ratio nested layout up"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        Direction::RIGHT,
        1.5,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (200, 100), ratio: 2),
            window(id: 1, loc: (200, 0), size: (0, 100), ratio: 0),
        ]);
        "resize ratio full clamp"
    )]
    fn test_resize_tile_ratio(
        mut tree: TileTree<TestWindow>,
        id: u32,
        direction: Direction,
        ratio: f64,
        expected: TileTree<TestWindow>,
    ) {
        tree.update_tile_size((0, 0).into(), (200, 100).into());
        tree.resize_tile(id, direction, TileResizeUnit::Ratio(ratio));
        tree.update_tile_size((0, 0).into(), (200, 100).into());
        assert_tree_eq!(tree, expected)
    }

    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
        ]),
        0,
        Direction::LEFT,
        10,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (200, 100)),
        ]);
        "resize single window no change"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        Direction::LEFT,
        120,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (80, 100), ratio: 0.8),
            window(id: 1, loc: (80, 0), size: (120, 100), ratio: 1.2),
        ]);
        "resize two windows left shrinks left neighbor"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        Direction::RIGHT,
        120,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (120, 100), ratio: 1.2),
            window(id: 1, loc: (120, 0), size: (80, 100), ratio: 0.8),
        ]);
        "resize two windows right shrinks right neighbor"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        Direction::TOP,
        70,
        tile_tree!(layout(split: Horizontal, loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (200, 30), ratio: 0.6),
            window(id: 1, loc: (0, 30), size: (200, 70), ratio: 1.4),
        ]);
        "resize horizontal split up shrinks top neighbor"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        Direction::BOTTOM,
        30,
        tile_tree!(layout(split: Horizontal, loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (200, 30), ratio: 0.6),
            window(id: 1, loc: (0, 30), size: (200, 70), ratio: 1.4),
        ]);
        "resize horizontal split down shrinks bottom neighbor"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
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
        Direction::BOTTOM,
        30,
        tile_tree!(layout(split: Horizontal, loc: (0, 0), size: (200, 100)) [
            layout(loc: (0, 0), size: (200, 30), ratio: 0.6) [
                window(id: 0, loc: (0, 0), size: (100, 30), ratio: 1),
                window(id: 1, loc: (100, 0), size: (100, 30), ratio: 1),
            ],
            layout(loc: (0, 30), size: (200, 70), ratio: 1.4) [
                window(id: 2, loc: (0, 30), size: (100, 70), ratio: 1),
                window(id: 3, loc: (100, 30), size: (100, 70), ratio: 1),
            ],
        ]);
        "resize exact nested layout internal"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
            window(id: 2),
        ]),
        1,
        Direction::LEFT,
        60,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (73, 100), ratio: 1.1),
            window(id: 1, loc: (73, 0), size: (60, 100), ratio: 0.9),
            window(id: 2, loc: (133, 0), size: (67, 100), ratio: 1),
        ]);
        "resize exact three windows middle"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        Direction::RIGHT,
        300,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (200, 100), ratio: 2),
            window(id: 1, loc: (200, 0), size: (0, 100), ratio: 0),
        ]);
        "resize exact beyond maximum clamped"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        Direction::RIGHT,
        -50,
        tile_tree!(layout(loc: (0, 0), size: (200, 100)) [
            window(id: 0, loc: (0, 0), size: (0, 100), ratio: 0),
            window(id: 1, loc: (0, 0), size: (200, 100), ratio: 2),
        ]);
        "resize exact zero minimum clamped"
    )]
    fn test_resize_tile_exact(
        mut tree: TileTree<TestWindow>,
        id: u32,
        direction: Direction,
        px: i32,
        expected: TileTree<TestWindow>,
    ) {
        tree.update_tile_size((0, 0).into(), (200, 100).into());
        tree.resize_tile(id, direction, px);
        tree.update_tile_size((0, 0).into(), (200, 100).into());
        assert_tree_eq!(tree, expected)
    }
}
