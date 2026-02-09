use crate::{
    input::{WindowDirection, WindowUnit},
    window::MappedWindow,
};
use serde::Deserialize;
use slotmap::{SlotMap, new_key_type};
use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Size},
};
use std::{borrow::Cow, collections::HashMap, fmt::Debug, rc::Rc};

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

#[derive(Debug, Clone, Copy, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(transparent)]
pub struct TileRatio(pub f64);

impl Default for TileRatio {
    fn default() -> Self {
        Self(1.0)
    }
}

#[derive(Debug)]
enum TileKind<T> {
    Window(T),
    Layout {
        split: TileSplit,
        orientation: TileOrientation,
        tiles: Vec<TileId>,
        size: Size<i32, Logical>,
    },
}

pub trait TileTreeWindow: Debug {
    type Inner;

    fn match_id(&self, id: TileTreeWindowId) -> bool;
    fn get_location(&self) -> Point<i32, Logical>;
    fn get_size(&self) -> Size<i32, Logical>;
    fn set_location(&mut self, location: Point<i32, Logical>);
    fn set_size(&mut self, size: Size<i32, Logical>);
    fn get_inner(&self) -> Self::Inner;
    fn set_inner(&mut self, inner: Self::Inner);
}

#[derive(Clone, Copy)]
pub enum TileTreeWindowId<'a> {
    #[allow(unused)]
    Id(u32),
    WlSurface(&'a WlSurface),
}

impl From<u32> for TileTreeWindowId<'_> {
    fn from(value: u32) -> Self {
        Self::Id(value)
    }
}

impl<'a> From<&'a WlSurface> for TileTreeWindowId<'a> {
    fn from(value: &'a WlSurface) -> Self {
        Self::WlSurface(value)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
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
struct LayoutSchema {
    #[serde(default)]
    split: TileSplit,
    #[serde(default)]
    orientation: TileOrientation,
    nodes: Vec<LayoutNode>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
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

impl<T: TileTreeWindow> TileTree<T> {
    pub fn new(layouts: Rc<LayoutSet>, layout_name: &str) -> Self {
        let mut arena = SlotMap::with_key();
        let layout = layouts.get(layout_name);

        let new_tile = Tile {
            kind: TileKind::Layout {
                split: layout.split,
                orientation: layout.orientation,
                tiles: Vec::new(),
                size: Size::default(),
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
                            size: Size::default(),
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

    // TODO
    pub fn remove<'a, I: Into<TileTreeWindowId<'a>> + Copy>(&mut self, id: I) -> Option<T> {
        struct RemoveTile {
            parent: TileId,
            index: usize,
        }

        fn traverse<T: TileTreeWindow>(
            arena: &TileArena<T>,
            layout_id: TileId,
            id: TileTreeWindowId<'_>,
        ) -> Option<RemoveTile> {
            for (index, tile_id) in arena[layout_id].as_layout_tiles().iter().enumerate() {
                match &arena[*tile_id] {
                    Tile {
                        kind: TileKind::Window(window),
                        ..
                    } => {
                        if window.match_id(id) {
                            return Some(RemoveTile {
                                parent: layout_id,
                                index,
                            });
                        }
                    }
                    _ => {
                        if let res @ Some(_) = traverse(arena, *tile_id, id) {
                            return res;
                        }
                    }
                }
            }

            None
        }

        if let Some(RemoveTile { parent, index }) = traverse(&self.arena, self.root, id.into()) {
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
        enum UpdateTile {
            Window {
                id: TileId,
                location: Point<i32, Logical>,
                size: Size<i32, Logical>,
            },
            Layout {
                id: TileId,
                size: Size<i32, Logical>,
            },
        }

        fn traverse<T>(
            arena: &TileArena<T>,
            layout_id: TileId,
            mut origin: Point<i32, Logical>,
            area: Size<i32, Logical>,
            updates: &mut Vec<UpdateTile>,
        ) {
            updates.push(UpdateTile::Layout {
                id: layout_id,
                size: area,
            });

            let TileKind::Layout {
                split,
                orientation,
                tiles,
                ..
            } = &arena[layout_id].kind
            else {
                return;
            };

            let total_ratio = tiles
                .iter()
                .fold(0.0, |acc, tile_id| acc + arena[*tile_id].ratio.0);

            let total_length = match split {
                TileSplit::Vertical => area.w,
                TileSplit::Horizontal => area.h,
            };
            let mut remaining_length = total_length;

            let len = tiles.len();
            let is_reversed = matches!(orientation, TileOrientation::TopLeft);

            let mut round_up = true;
            for i in 0..len {
                let idx = if is_reversed { len - 1 - i } else { i };
                let tile_id = tiles[idx];
                let tile = &arena[tile_id];

                let new_length = if i == len - 1 {
                    remaining_length
                } else {
                    let mut length = total_length as f64 * tile.ratio.0 / total_ratio;
                    if round_up {
                        length = length.ceil();
                    } else {
                        length = length.floor();
                    }
                    round_up = !round_up;
                    let length = length as i32;
                    remaining_length -= length;
                    length
                };

                let new_area = match split {
                    TileSplit::Vertical => Size::new(new_length, area.h),
                    TileSplit::Horizontal => Size::new(area.w, new_length),
                };

                match &tile.kind {
                    TileKind::Window(_) => {
                        updates.push(UpdateTile::Window {
                            id: tile_id,
                            location: origin,
                            size: new_area,
                        });
                    }
                    TileKind::Layout { .. } => {
                        traverse(arena, tile_id, origin, new_area, updates);
                    }
                }

                match split {
                    TileSplit::Vertical => origin.x += new_area.w,
                    TileSplit::Horizontal => origin.y += new_area.h,
                }
            }
        }

        let mut updates = Vec::new();
        traverse(&self.arena, self.root, location, size, &mut updates);

        for update in updates {
            match update {
                UpdateTile::Window { id, location, size } => {
                    let window = self.arena[id].as_window_mut();
                    window.set_location(location);
                    window.set_size(size);
                }
                UpdateTile::Layout { id, size: new_size } => {
                    let TileKind::Layout { size, .. } = &mut self.arena[id].kind else {
                        continue;
                    };
                    *size = new_size;
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

    fn find_tile_id(&self, id: TileTreeWindowId) -> Option<TileId> {
        fn traverse<T: TileTreeWindow>(
            arena: &TileArena<T>,
            current_id: TileId,
            target_id: TileTreeWindowId,
        ) -> Option<TileId> {
            match &arena[current_id].kind {
                TileKind::Window(window) => {
                    if window.match_id(target_id) {
                        return Some(current_id);
                    }
                }
                TileKind::Layout { tiles, .. } => {
                    for &child_id in tiles.iter() {
                        if let res @ Some(_) = traverse(arena, child_id, target_id) {
                            return res;
                        }
                    }
                }
            }
            None
        }

        traverse(&self.arena, self.root, id)
    }

    pub fn find_window_mut<'a, I: Into<TileTreeWindowId<'a>> + Copy>(
        &mut self,
        id: I,
    ) -> Option<&mut T> {
        let id = id.into();
        let tile_id = self.find_tile_id(id)?;

        self.arena.get_mut(tile_id).map(|t| t.as_window_mut())
    }

    pub fn for_each_window_mut(&mut self, mut f: impl FnMut(&mut T)) {
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

        for tile_id in window_ids {
            if let Tile {
                kind: TileKind::Window(window),
                ..
            } = &mut self.arena[tile_id]
            {
                f(window);
            }
        }
    }

    pub fn find_windows_in_direction<'a, I: Into<TileTreeWindowId<'a>> + Copy>(
        &self,
        id: I,
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

        let Some(target_id) = self.find_tile_id(id.into()) else {
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

    pub fn swap_window<'a, I: Into<TileTreeWindowId<'a>> + Copy>(&mut self, lhs: I, rhs: I) {
        let lhs_id = match self.find_tile_id(lhs.into()) {
            Some(id) => id,
            None => return,
        };
        let rhs_id = match self.find_tile_id(rhs.into()) {
            Some(id) => id,
            None => return,
        };

        let lhs_inner = self.arena[lhs_id].as_window().get_inner();
        let rhs_inner = self.arena[rhs_id].as_window().get_inner();

        self.arena[lhs_id].as_window_mut().set_inner(rhs_inner);
        self.arena[rhs_id].as_window_mut().set_inner(lhs_inner);
    }

    fn adjust_adjacent_ratios(
        &mut self,
        parent_id: TileId,
        child_id: TileId,
        index_offset: i32,
        unit: WindowUnit,
        size_component: f64,
    ) {
        let parent = &self.arena[parent_id];
        let TileKind::Layout { tiles, .. } = &parent.kind else {
            panic!();
        };

        let index = tiles.iter().position(|t| *t == child_id).unwrap() as i32 + index_offset;
        if (0..(tiles.len() as i32)).contains(&index) {
            let total_ratio = tiles
                .iter()
                .fold(0., |acc, id| acc + self.arena[*id].ratio.0);
            let other_id = tiles[index as usize];
            let ratio_offset = match unit {
                WindowUnit::Ratio(TileRatio(ratio)) => ratio,
                WindowUnit::Px(px) => px as f64 * total_ratio / size_component,
            };

            let window = &mut self.arena[child_id];
            window.ratio.0 += ratio_offset;
            let other = &mut self.arena[other_id];
            other.ratio.0 -= ratio_offset;
        }
    }

    fn find_ancestor_with_opposite_split(
        &self,
        start_id: TileId,
        current_split: TileSplit,
    ) -> Option<(TileId, TileId)> {
        let mut refer = start_id;
        let mut some_parent = self.arena[start_id].parent;

        while let Some(parent) = some_parent {
            let TileKind::Layout { split, .. } = &self.arena[parent].kind else {
                panic!();
            };
            if *split != current_split {
                return Some((parent, refer));
            }
            refer = parent;
            some_parent = self.arena[parent].parent;
        }
        None
    }

    pub fn resize_tile<'a, I: Into<TileTreeWindowId<'a>> + Copy>(
        &mut self,
        id: I,
        edge: WindowDirection,
        unit: WindowUnit,
    ) {
        let window_id = self.find_tile_id(id.into()).unwrap();
        let parent = self.arena[window_id].parent.unwrap();
        let TileKind::Layout {
            split,
            orientation,
            size,
            ..
        } = &self.arena[parent].kind
        else {
            panic!()
        };

        let compute_index_offset = |edge: WindowDirection, orientation: TileOrientation| -> i32 {
            let mut offset = match edge {
                WindowDirection::Left | WindowDirection::Up => -1,
                WindowDirection::Right | WindowDirection::Down => 1,
            };
            offset *= match orientation {
                TileOrientation::BottomRight => 1,
                TileOrientation::TopLeft => -1,
            };
            offset
        };

        match (split, edge) {
            (TileSplit::Vertical, WindowDirection::Left | WindowDirection::Right) => {
                let index_offset = compute_index_offset(edge, *orientation);
                self.adjust_adjacent_ratios(parent, window_id, index_offset, unit, size.w as f64);
            }
            (TileSplit::Horizontal, WindowDirection::Up | WindowDirection::Down) => {
                let index_offset = compute_index_offset(edge, *orientation);
                self.adjust_adjacent_ratios(parent, window_id, index_offset, unit, size.h as f64);
            }
            (TileSplit::Vertical, WindowDirection::Up | WindowDirection::Down) => {
                if let Some((ancestor_id, refer_id)) =
                    self.find_ancestor_with_opposite_split(window_id, TileSplit::Vertical)
                {
                    let TileKind::Layout {
                        orientation: ancestor_orientation,
                        size: ancestor_size,
                        ..
                    } = &self.arena[ancestor_id].kind
                    else {
                        panic!()
                    };
                    let index_offset = compute_index_offset(edge, *ancestor_orientation);
                    self.adjust_adjacent_ratios(
                        ancestor_id,
                        refer_id,
                        index_offset,
                        unit,
                        ancestor_size.h as f64,
                    );
                }
            }
            (TileSplit::Horizontal, WindowDirection::Left | WindowDirection::Right) => {
                if let Some((ancestor_id, refer_id)) =
                    self.find_ancestor_with_opposite_split(window_id, TileSplit::Horizontal)
                {
                    let TileKind::Layout {
                        orientation: ancestor_orientation,
                        size: ancestor_size,
                        ..
                    } = &self.arena[ancestor_id].kind
                    else {
                        panic!()
                    };
                    let index_offset = compute_index_offset(edge, *ancestor_orientation);
                    self.adjust_adjacent_ratios(
                        ancestor_id,
                        refer_id,
                        index_offset,
                        unit,
                        ancestor_size.w as f64,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use super::*;
    use test_case::test_case;

    #[derive(Debug, Default, PartialEq)]
    pub struct TestWindow {
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
        type Inner = Option<u32>;

        fn match_id(&self, id: TileTreeWindowId) -> bool {
            match id {
                TileTreeWindowId::Id(x) => self.id == Some(x),
                _ => false,
            }
        }

        fn get_location(&self) -> Point<i32, Logical> {
            self.location
        }

        fn get_size(&self) -> Size<i32, Logical> {
            self.size
        }

        fn set_location(&mut self, location: Point<i32, Logical>) {
            self.location = location;
        }

        fn set_size(&mut self, size: Size<i32, Logical>) {
            self.size = size;
        }

        fn get_inner(&self) -> Self::Inner {
            self.id
        }

        fn set_inner(&mut self, inner: Self::Inner) {
            self.id = inner;
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
                            size: _,
                        },
                        TileKind::Layout {
                            split: split2,
                            orientation: orientation2,
                            tiles: tiles2,
                            size: _,
                        },
                    ) => {
                        if split1 != split2 || orientation1 != orientation2 {
                            return false;
                        }
                        if tiles1.len() != tiles2.len() {
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
                        "Window {}[point: ({}, {}), area: ({}, {}), ratio: {}]\n",
                        if let Some(id) = window.id {
                            id.to_string() + " "
                        } else {
                            "".to_string()
                        },
                        window.location.x,
                        window.location.y,
                        window.size.w,
                        window.size.h,
                        tile.ratio.0
                    ));
                }
                TileKind::Layout {
                    split,
                    orientation,
                    tiles,
                    size,
                } => {
                    f.push_str(&format!(
                        "Layout [{:?}, {:?}, area: ({}, {}), ratio: {}]\n",
                        split, orientation, size.w, size.h, tile.ratio.0
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
        // ==================== Window Option Parsing ====================
        (@window_opt $window:ident $size:ident) => {};

        (@window_opt $window:ident $ratio:ident ratio: $r:expr $(, $($rest:tt)*)?) => {
            $ratio = $r as f64;
            $(tile_tree!(@window_opt $window $ratio $($rest)*);)?
        };

        (@window_opt $window:ident $size:ident id: $id:expr $(, $($rest:tt)*)?) => {
            $window.id = Some($id);
            $(tile_tree!(@window_opt $window $size $($rest)*);)?
        };

        (@window_opt $window:ident $size:ident pos: $point:expr, size: $sz:expr $(, $($rest:tt)*)?) => {
            $window.location = $point.into();
            $window.size = $sz.into();
            $(tile_tree!(@window_opt $window $size $($rest)*);)?
        };

        // ==================== Layout Option Parsing ====================
        (@layout_opt $split:ident $orient:ident $size:ident) => {};

        (@layout_opt $split:ident $orient:ident $ratio:ident ratio: $r:expr $(, $($rest:tt)*)?) => {
            $ratio = $r;
            $(tile_tree!(@layout_opt $split $orient $size $($rest)*);)?
        };

        (@layout_opt $split:ident $orient:ident $size:ident split: $s:ident $(, $($rest:tt)*)?) => {
            $split = TileSplit::$s;
            $(tile_tree!(@layout_opt $split $orient $size $($rest)*);)?
        };

        (@layout_opt $split:ident $orient:ident $size:ident orient: $o:ident $(, $($rest:tt)*)?) => {
            $orient = TileOrientation::$o;
            $(tile_tree!(@layout_opt $split $orient $size $($rest)*);)?
        };

        // ==================== Node: Window ====================
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

        // ==================== Node: Layout ====================
        (@node $arena:ident, $parent:expr, layout($($opts:tt)*) [ $($child_kind:ident ( $($child_args:tt)* ) $( [ $($child_inner:tt)* ] )? ),* $(,)? ]) => {{
            #[allow(unused_mut, unused_assignments)]
            let mut split = TileSplit::default();
            #[allow(unused_mut, unused_assignments)]
            let mut orient = TileOrientation::default();
            #[allow(unused_mut, unused_assignments)]
            let mut ratio = 1.0;
            tile_tree!(@layout_opt split orient ratio $($opts)*);

            let layout_id = $arena.insert(Tile {
                kind: TileKind::Layout {
                    split,
                    orientation: orient,
                    tiles: Vec::new(),
                    size: Size::default(),
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

        // ==================== Entry Point ====================
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
        // Parser for node options
        (@opt $layout:ident $repeat:ident $ratio:ident) => {};
        (@opt $layout:ident $repeat:ident $ratio:ident repeat: $r:expr $(, $($rest:tt)*)?) => {
            $repeat = TileRepeat($r);
            $(node!(@opt $layout $repeat $ratio $($rest)*);)?
        };
        (@opt $layout:ident $repeat:ident $ratio:ident ratio: $s:expr $(, $($rest:tt)*)?) => {
            $ratio = $s;
            $(node!(@opt $layout $repeat $ratio $($rest)*);)?
        };

        // Entry point for Window Node
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

        // Entry point for Layout Reference Node
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
        // Parser for schema options
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

        // Main Entry
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
        LayoutType::Ref("windows"),
        1,
        tile_tree!(layout() [
            window()
        ]);
        "simple"
    )]
    #[test_case(
        LayoutType::Ref("windows"),
        3,
        tile_tree!(layout() [
            window(),
            window(),
            window(),
        ]);
        "repeat windows"
    )]
    #[test_case(
        LayoutType::New(vec![
            ("default", schema!(nodes: [
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
        "ratio"
    )]
    #[test_case(
        LayoutType::Ref("layout"),
        1,
        tile_tree!(layout() [
            layout() [
                window()
            ]
        ]);
        "layout"
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
        "repeat layouts"
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
        "repeat windows and layouts"
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
        "nested layout"
    )]
    fn test_insert(layout_type: LayoutType, tiles: u32, expected: TileTree<TestWindow>) {
        let (layouts, layout_name) = match layout_type {
            LayoutType::Ref(name) => (LayoutSet((*LAYOUT_SET).clone()), name),
            LayoutType::New(iter) => (
                LayoutSet(HashMap::from_iter(
                    iter.into_iter()
                        .map(|(name, schema)| (name.to_string(), schema)),
                )),
                "default",
            ),
        };
        let mut tree = TileTree::<TestWindow>::new(Rc::new(layouts), layout_name);
        for _ in 0..tiles {
            tree.insert(TestWindow::new(), None);
        }
        assert_tree_eq!(tree, expected);
    }

    #[test_case("windows", 1 => true; "pass")]
    #[test_case("empty", 1 => false; "empty")]
    #[test_case("windows", 4 => false; "windows")]
    #[test_case("layouts", 7 => false; "layouts")]
    #[test_case("mix windows layouts", 17 => false; "mix windows layouts")]
    #[test_case("nested layout", 4 => false; "nested layout")]
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
        3,
        0,
        tile_tree!(layout() [
            window(id: 1),
            window(id: 2),
        ]);
        "first window"
    )]
    #[test_case(
        "windows",
        3,
        1,
        tile_tree!(layout() [
            window(id: 0),
            window(id: 2),
        ]);
        "middle window"
    )]
    #[test_case(
        "windows",
        3,
        2,
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]);
        "last window"
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
        "window in layout"
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
        "layout"
    )]
    #[test_case(
        "nested layout",
        1,
        0,
        tile_tree!(layout() []);
        "nested layout"
    )]
    fn test_remove(layout_name: &str, tiles: u32, remove: u32, expected: TileTree<TestWindow>) {
        let layouts = Rc::new(LayoutSet((*LAYOUT_SET).clone()));
        let mut tree = TileTree::<TestWindow>::new(layouts, layout_name);
        for i in 0..tiles {
            tree.insert(
                TestWindow {
                    id: Some(i),
                    ..Default::default()
                },
                None,
            );
        }
        assert_eq!(tree.remove(remove).map(|t| t.id), Some(Some(remove)));
        assert_tree_eq!(tree, expected);
    }

    #[test_case(
        tile_tree!(layout() [
            window(),
            window(),
        ]),
        tile_tree!(layout() [
            window(pos: (0, 0), size: (50, 100)),
            window(pos: (50, 0), size: (50, 100)),
        ]);
        "simple"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(ratio: 1),
            window(ratio: 3),
            window(ratio: 2),
            window(ratio: 5),
            window(ratio: 4),
        ]),
        tile_tree!(layout() [
            window(ratio: 1, pos: (0, 0), size: (7, 100)),
            window(ratio: 3, pos: (7, 0), size: (20, 100)),
            window(ratio: 2, pos: (27, 0), size: (14, 100)),
            window(ratio: 5, pos: (41, 0), size: (33, 100)),
            window(ratio: 4, pos: (74, 0), size: (26, 100)),
        ]);
        "tile size"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(),
            window(),
        ]),
        tile_tree!(layout(split: Horizontal) [
            window(pos: (0, 0), size: (100, 50)),
            window(pos: (0, 50), size: (100, 50)),
        ]);
        "layout split"
    )]
    #[test_case(
        tile_tree!(layout(orient: TopLeft) [
            window(),
            window(),
        ]),
        tile_tree!(layout(orient: TopLeft) [
            window(pos: (50, 0), size: (50, 100)),
            window(pos: (0, 0), size: (50, 100)),
        ]);
        "layout orientation"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(),
            layout(split: Horizontal, orient: TopLeft) [
                window(),
                layout(split: Horizontal) [
                    window(),
                    window(),
                ]
            ]
        ]),
        tile_tree!(layout() [
            window(pos: (0, 0), size: (50, 100)),
            layout(split: Horizontal, orient: TopLeft) [
                window(pos: (50, 50), size: (50, 50)),
                layout(split: Horizontal) [
                    window(pos: (50, 0), size: (50, 25)),
                    window(pos: (50, 25), size: (50, 25)),
                ]
            ]
        ]);
        "all"
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
        "empty"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        WindowDirection::Up,
        vec![0];
        "2 stacked top"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        WindowDirection::Down,
        vec![1];
        "2 stacked bottom"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        WindowDirection::Left,
        vec![0];
        "2 parallel left"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        WindowDirection::Right,
        vec![1];
        "2 parallel right"
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
        "3|2 row split top"
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
        "3|2 row split bottom"
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
        "3|2 col split left"
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
        "3|2 col split right"
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
        "multiple"
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
                .map(|w| w.id.unwrap())
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
        "simple"
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
        "across tiles"
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
            window(id: 1),
        ]),
        1,
        WindowDirection::Left,
        10,
        tile_tree!(layout() [
            window(id: 0, pos: (0, 0), size: (90, 100), ratio: 0.9),
            window(id: 1, pos: (90, 0), size: (110, 100), ratio: 1.1),
        ]);
        "resize parallel with axis"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        WindowDirection::Left,
        10,
        tile_tree!(layout() [
            window(id: 0, pos: (0, 0), size: (100, 100), ratio: 1),
            window(id: 1, pos: (100, 0), size: (100, 100), ratio: 1),
        ]);
        "resize parallel with axis at edge"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal, orient: TopLeft) [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        WindowDirection::Down,
        10,
        tile_tree!(layout(split: Horizontal, orient: TopLeft) [
            window(id: 0, pos: (0, 60), size: (200, 40), ratio: 0.8),
            window(id: 1, pos: (0, 0), size: (200, 60), ratio: 1.2),
        ]);
        "resize parallel with axis in reverse orientation"
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
        tile_tree!(layout(split: Horizontal) [
            window(id: 0, pos: (0, 0), size: (200, 40), ratio: 0.8),
            layout(ratio: 1.2) [
                window(id: 1, pos: (0, 40), size: (100, 60)),
                window(id: 2, pos: (100, 40), size: (100, 60)),
            ]
        ]);
        "resize perpendicular with axis"
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
        tile_tree!(layout(split: Horizontal) [
            window(id: 0, pos: (0, 0), size: (200, 50)),
            layout() [
                window(id: 1, pos: (0, 50), size: (100, 50)),
                window(id: 2, pos: (100, 50), size: (100, 50)),
            ]
        ]);
        "resize perpendicular with axis at edge"
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
        tile_tree!(layout() [
            window(id: 0, pos: (0, 0), size: (67, 100)),
            layout(split: Horizontal, ratio: 0.85) [
                window(id: 1, pos: (67, 0), size: (56, 50), ratio: 1),
                window(id: 2, pos: (67, 50), size: (56, 50), ratio: 1),
            ],
            layout(split: Horizontal, ratio: 1.15) [
                window(id: 3, pos: (123, 0), size: (77, 34), ratio: 1),
                window(id: 4, pos: (123, 34), size: (77, 33), ratio: 1),
                window(id: 5, pos: (123, 67), size: (77, 33), ratio: 1),
            ]
        ]);
        "resize multiple perpendicular with axis"
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
