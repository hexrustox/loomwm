use crate::{input::FocusDirection, utils::partition, window::MappedWindow};
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

#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(PartialEq))]
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
    },
}

pub trait TileTreeWindow: Debug {
    fn match_id(&self, id: TileTreeWindowId) -> bool;
    fn get_location(&self) -> Point<i32, Logical>;
    fn get_size(&self) -> Size<i32, Logical>;
    fn set_location(&mut self, location: Point<i32, Logical>);
    fn set_size(&mut self, size: Size<i32, Logical>);
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

#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(test, derive(PartialEq))]
enum TileSplit {
    #[default]
    Vertical,
    Horizontal,
}

#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(test, derive(PartialEq))]
// TEMP
#[allow(dead_code)]
enum TileOrientation {
    #[default]
    BottomRight,
    TopLeft,
}

#[derive(Debug)]
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

#[derive(Debug, Default, Clone)]
struct LayoutSchema {
    split: TileSplit,
    orientation: TileOrientation,
    nodes: Vec<LayoutNode>,
}

#[derive(Debug, Default, Clone)]
struct LayoutNode {
    layout: Option<String>,
    repeat: TileRepeat,
    ratio: TileRatio,
}

#[derive(Debug, Clone)]
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

    pub fn update_toplevel_state(
        &mut self,
        location: Point<i32, Logical>,
        size: Size<i32, Logical>,
    ) {
        struct UpdateWindow {
            id: TileId,
            location: Point<i32, Logical>,
            size: Size<i32, Logical>,
        }

        fn traverse<T>(
            arena: &TileArena<T>,
            layout_id: TileId,
            mut origin: Point<i32, Logical>,
            area: Size<i32, Logical>,
        ) -> Vec<UpdateWindow> {
            let mut updates = Vec::new();

            let TileKind::Layout {
                split,
                orientation,
                tiles,
            } = &arena[layout_id].kind
            else {
                unreachable!()
            };

            let total_weight = tiles
                .iter()
                .fold(0.0, |acc, tile_id| acc + arena[*tile_id].ratio.0);

            let mut lengths = partition(
                match split {
                    TileSplit::Vertical => area.w,
                    TileSplit::Horizontal => area.h,
                },
                total_weight as usize,
            );

            let tile_iter: Box<dyn Iterator<Item = &_>> =
                if matches!(orientation, TileOrientation::BottomRight) {
                    Box::new(tiles.iter())
                } else {
                    Box::new(tiles.iter().rev())
                };
            for tile_id in tile_iter {
                let tile = &arena[*tile_id];
                let new_area = {
                    let len = lengths
                        .drain(lengths.len().saturating_sub(tile.ratio.0 as usize)..)
                        .sum();
                    match split {
                        TileSplit::Vertical => Size::new(len, area.h),
                        TileSplit::Horizontal => Size::new(area.w, len),
                    }
                };
                match &tile.kind {
                    TileKind::Window(_) => {
                        updates.push(UpdateWindow {
                            id: *tile_id,
                            location: origin,
                            size: new_area,
                        });
                    }
                    TileKind::Layout { .. } => {
                        updates.extend(traverse(arena, *tile_id, origin, new_area));
                    }
                }

                match split {
                    TileSplit::Vertical => origin.x += new_area.w,
                    TileSplit::Horizontal => origin.y += new_area.h,
                }
            }

            updates
        }

        let updates = traverse(&self.arena, self.root, location, size);
        for update in updates {
            let window = self.arena[update.id].as_window_mut();
            window.set_location(update.location);
            window.set_size(update.size);
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

    pub fn find_window_mut<'a, I: Into<TileTreeWindowId<'a>> + Copy>(
        &mut self,
        id: I,
    ) -> Option<&mut T> {
        fn traverse<T: TileTreeWindow>(
            arena: &TileArena<T>,
            layout_id: TileId,
            id: TileTreeWindowId,
        ) -> Option<TileId> {
            for tile_id in arena[layout_id].as_layout_tiles().iter() {
                match &arena[*tile_id] {
                    Tile {
                        kind: TileKind::Window(window),
                        ..
                    } => {
                        if window.match_id(id) {
                            return Some(*tile_id);
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
        let tile_id = traverse(&self.arena, self.root, id.into());

        tile_id.map(|id| self.arena[id].as_window_mut())
    }

    pub fn find_windows_in_direction<'a, I: Into<TileTreeWindowId<'a>> + Copy>(
        &self,
        id: I,
        direction: FocusDirection,
    ) -> Vec<&T> {
        fn traverse_find<T: TileTreeWindow>(
            arena: &TileArena<T>,
            layout_id: TileId,
            id: TileTreeWindowId,
        ) -> Option<TileId> {
            for tile_id in arena[layout_id].as_layout_tiles().iter() {
                match &arena[*tile_id] {
                    Tile {
                        kind: TileKind::Window(window),
                        ..
                    } => {
                        if window.match_id(id) {
                            return Some(*tile_id);
                        }
                    }
                    _ => {
                        if let res @ Some(_) = traverse_find(arena, *tile_id, id) {
                            return res;
                        }
                    }
                }
            }

            None
        }
        fn traverse_calc<'a, T: TileTreeWindow, F: Fn(&T) -> bool>(
            arena: &'a TileArena<T>,
            layout_id: TileId,
            possible_ids: &mut Vec<&'a T>,
            cmp: &F,
        ) {
            for tile_id in arena[layout_id].as_layout_tiles().iter() {
                match &arena[*tile_id] {
                    Tile {
                        kind: TileKind::Window(window),
                        ..
                    } => {
                        if cmp(window) {
                            possible_ids.push(window);
                        }
                    }
                    _ => {
                        traverse_calc(arena, *tile_id, possible_ids, cmp);
                    }
                }
            }
        }

        let Some(window_id) = traverse_find(&self.arena, self.root, id.into()) else {
            return Vec::new();
        };
        let target = &self.arena[window_id].as_window();

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

            fn is_in_direction(&self, direction: FocusDirection, target: &Rect) -> bool {
                use FocusDirection::*;
                match direction {
                    Top => self.bottom() <= target.top() && self.overlaps_horizontally(target),
                    Bottom => self.top() >= target.bottom() && self.overlaps_horizontally(target),
                    Left => self.right() <= target.left() && self.overlaps_vertically(target),
                    Right => self.left() >= target.right() && self.overlaps_vertically(target),
                }
            }
        }
        let target_rect = Rect::new(target.get_location(), target.get_size());

        let mut possible_ids = Vec::new();
        traverse_calc(&self.arena, self.root, &mut possible_ids, &|window| {
            let window_rect = Rect::new(window.get_location(), window.get_size());
            window_rect.is_in_direction(direction, &target_rect)
        });

        possible_ids
    }
}

// TEMP
pub fn test_layout_set() -> LayoutSet {
    LayoutSet(HashMap::from_iter([
        (
            "master".to_string(),
            LayoutSchema {
                nodes: vec![
                    LayoutNode::default(),
                    LayoutNode {
                        layout: Some("slaves".to_string()),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
        ),
        (
            "slaves".to_string(),
            LayoutSchema {
                split: TileSplit::Horizontal,
                nodes: vec![LayoutNode {
                    repeat: TileRepeat(2),
                    ..Default::default()
                }],
                ..Default::default()
            },
        ),
    ]))
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
                        },
                        TileKind::Layout {
                            split: split2,
                            orientation: orientation2,
                            tiles: tiles2,
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
                        "Window {}[point: ({}, {}), area: ({}, {}), size: {}]\n",
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
                } => {
                    f.push_str(&format!(
                        "Layout [{:?}, {:?}, size: {}]\n",
                        split, orientation, tile.ratio.0
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
            $ratio = $r;
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
            let mut ratio = 1;
            tile_tree!(@window_opt window ratio $($opts)*);
            $arena.insert(Tile {
                kind: TileKind::Window(window),
                ratio: TileRatio(ratio as f64),
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
            let mut ratio = 1;
            tile_tree!(@layout_opt split orient ratio $($opts)*);

            let layout_id = $arena.insert(Tile {
                kind: TileKind::Layout {
                    split,
                    orientation: orient,
                    tiles: Vec::new(),
                },
                ratio: TileRatio(ratio as f64),
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
        ]),
        tile_tree!(layout() [
            window(ratio: 1, pos: (0, 0), size: (16, 100)),
            window(ratio: 3, pos: (16, 0), size: (50, 100)),
            window(ratio: 2, pos: (66, 0), size: (34, 100)),
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
    fn test_update_toplevel_state(mut tree: TileTree<TestWindow>, expected: TileTree<TestWindow>) {
        tree.update_toplevel_state((0, 0).into(), (100, 100).into());
        assert_tree_eq!(tree, expected);
    }

    #[test_case(
        tile_tree!(layout() [window(id: 0)]),
        0,
        FocusDirection::Top,
        vec![];
        "empty"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        FocusDirection::Top,
        vec![0];
        "2 stacked top"
    )]
    #[test_case(
        tile_tree!(layout(split: Horizontal) [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        FocusDirection::Bottom,
        vec![1];
        "2 stacked bottom"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        1,
        FocusDirection::Left,
        vec![0];
        "2 parallel left"
    )]
    #[test_case(
        tile_tree!(layout() [
            window(id: 0),
            window(id: 1),
        ]),
        0,
        FocusDirection::Right,
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
        FocusDirection::Top,
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
        FocusDirection::Bottom,
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
        FocusDirection::Left,
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
        FocusDirection::Right,
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
        FocusDirection::Right,
        vec![2, 3];
        "multiple"
    )]
    fn test_find_windows_in_direction(
        mut tree: TileTree<TestWindow>,
        id: u32,
        direction: FocusDirection,
        expected: Vec<u32>,
    ) {
        tree.update_toplevel_state((0, 0).into(), (100, 100).into());
        println!("{}", tree.visualize());
        assert_eq!(
            tree.find_windows_in_direction(id, direction)
                .iter()
                .map(|w| w.id.unwrap())
                .collect::<Vec<_>>(),
            expected
        );
    }
}
