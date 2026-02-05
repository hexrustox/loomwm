use crate::{utils::partition, window::MappedWindow};
use slotmap::{SlotMap, new_key_type};
use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point, Size},
};
use std::{borrow::Cow, collections::HashMap, rc::Rc};

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
    size: TileSize,
    parent: Option<TileId>,
}

impl<T> Tile<T> {
    fn into_window(self) -> T {
        match self.kind {
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
struct TileSize(u32);

impl Default for TileSize {
    fn default() -> Self {
        Self(1)
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

pub trait TileTreeWindow {
    fn match_id(&self, id: TileTreeWindowId) -> bool;
    fn update_location(&mut self, location: Point<i32, Logical>);
    fn update_size(&mut self, size: Size<i32, Logical>);
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
    size: TileSize,
}

#[derive(Debug, Clone)]
struct TileRepeat(usize);

impl TileRepeat {
    const MAX: Self = Self(usize::MAX);
}

impl Default for TileRepeat {
    fn default() -> Self {
        TileRepeat(1)
    }
}

#[derive(Debug)]
struct TileLayoutTrace {
    layout: String,
    index: usize,
    tile_count: usize,
}

impl TileLayoutTrace {
    fn new(layout_name: &str) -> Self {
        Self {
            layout: layout_name.to_string(),
            index: 0,
            tile_count: 0,
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
            size: TileSize(1),
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

    pub fn insert(&mut self, window: T) -> Option<T> {
        let Some(trace) = self.layout_trace.last_mut() else {
            return Some(window);
        };

        let layout = self.layouts.get(&trace.layout);
        for i in trace.index..layout.nodes.len() {
            trace.index = i;
            let node = &layout.nodes[i];
            if trace.tile_count < node.repeat.0 {
                if let Some(layout_name) = &node.layout {
                    let layout = self.layouts.get(layout_name);
                    let new_tile = Tile {
                        kind: TileKind::Layout {
                            split: layout.split,
                            orientation: layout.orientation,
                            tiles: Vec::new(),
                        },
                        size: node.size,
                        parent: Some(self.current_tile),
                    };
                    let tile_id = self.arena.insert(new_tile);

                    self.arena[self.current_tile]
                        .as_layout_tiles_mut()
                        .push(tile_id);

                    self.current_tile = tile_id;
                    self.layout_trace.push(TileLayoutTrace::new(layout_name));
                    self.insert(window);
                } else {
                    let new_tile = Tile {
                        kind: TileKind::Window(window),
                        size: node.size,
                        parent: Some(self.current_tile),
                    };
                    let tile_id = self.arena.insert(new_tile);

                    self.arena[self.current_tile]
                        .as_layout_tiles_mut()
                        .push(tile_id);

                    trace.tile_count += 1;
                }
                return None;
            }
            trace.tile_count = 0;
        }

        let Some(parent) = self.arena[self.current_tile].parent else {
            return Some(window);
        };
        self.current_tile = parent;
        self.layout_trace.pop();
        self.layout_trace.last_mut().unwrap().index += 1;
        self.insert(window)
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
                        ..
                    } => {
                        extracted_windows.push(self.arena.remove(id).unwrap().into_window());
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
            for window in extracted_windows {
                self.insert(window);
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
                .fold(0, |acc, tile_id| acc + arena[*tile_id].size.0);

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
                        .drain(lengths.len().saturating_sub(tile.size.0 as usize)..)
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
            window.update_location(update.location);
            window.update_size(update.size);
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

        fn update_location(&mut self, location: Point<i32, Logical>) {
            self.location = location;
        }

        fn update_size(&mut self, size: Size<i32, Logical>) {
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

                if tile_a.size != tile_b.size {
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
                        "Window {}[point:({}, {}) area:({}, {}) size:{}]\n",
                        if let Some(id) = window.id {
                            id.to_string() + " "
                        } else {
                            "".to_string()
                        },
                        window.location.x,
                        window.location.y,
                        window.size.w,
                        window.size.h,
                        tile.size.0
                    ));
                }
                TileKind::Layout {
                    split,
                    orientation,
                    tiles,
                } => {
                    f.push_str(&format!(
                        "Layout [{:?} {:?} size:{}]\n",
                        split, orientation, tile.size.0
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
        (@node $arena:ident, $parent:expr, window($($ratio:expr)?$(; $id:expr)?$(, $point:expr, $size:expr)?)) => {
            $arena.insert(Tile {
                kind: TileKind::Window(
                    {
                        #[allow(unused_mut)]
                        #[allow(unused_assignments)]
                        let mut window = TestWindow::new();
                        $(window.id = Some($id);)?
                        $(
                            window.location = $point.into();
                            window.size = $size.into();
                        )?
                        window
                    }
                ),
                size: TileSize({
                    #[allow(unused_mut)]
                    #[allow(unused_assignments)]
                    let mut size = 1;
                    $(size = $ratio;)?
                    size
                }),
                parent: $parent,
            })
        };

        (@node $arena:ident, $parent:expr, layout($($size:expr)?$(; $split:ident)?$(, $orient:ident)?) [ $($child_kind:ident ( $($child_args:tt)* ) $( [ $($child_inner:tt)* ] )? ),* $(,)? ]) => {{
            let layout_id = $arena.insert_with_key(|_| Tile {
                kind: TileKind::Layout {
                    split: {
                        #[allow(unused_mut)]
                        #[allow(unused_assignments)]
                        let mut split = TileSplit::default();
                        $(split = TileSplit::$split;)?
                        split

                    },
                    orientation: {
                        #[allow(unused_mut)]
                        #[allow(unused_assignments)]
                        let mut orient = TileOrientation::default();
                        $(orient = TileOrientation::$orient;)?
                        orient

                    },
                    tiles: Vec::new(),
                },
                size: TileSize({
                    #[allow(unused_mut)]
                    #[allow(unused_assignments)]
                    let mut size = 1;
                    $(size = $size;)?
                    size
                }),
                parent: $parent,
            });

            let children = vec![
                $(
                    tile_tree!(@node $arena, Some(layout_id), $child_kind ( $($child_args)* ) $( [ $($child_inner)* ] )? )
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

    static LAYOUT_SET: LazyLock<HashMap<String, LayoutSchema>> = LazyLock::new(|| {
        HashMap::from_iter([
            (
                "empty".to_string(),
                LayoutSchema {
                    split: Default::default(),
                    orientation: Default::default(),
                    nodes: vec![],
                },
            ),
            (
                "1 window repeat 3".to_string(),
                LayoutSchema {
                    split: Default::default(),
                    orientation: Default::default(),
                    nodes: vec![LayoutNode {
                        layout: None,
                        repeat: TileRepeat(3),
                        size: TileSize(1),
                    }],
                },
            ),
            (
                "3 windows".to_string(),
                LayoutSchema {
                    split: Default::default(),
                    orientation: Default::default(),
                    nodes: vec![
                        LayoutNode {
                            layout: None,
                            repeat: TileRepeat(1),
                            size: TileSize(1),
                        },
                        LayoutNode {
                            layout: None,
                            repeat: TileRepeat(2),
                            size: TileSize(2),
                        },
                        LayoutNode {
                            layout: None,
                            repeat: TileRepeat(1),
                            size: TileSize(3),
                        },
                    ],
                },
            ),
            (
                "1 layout".to_string(),
                LayoutSchema {
                    split: Default::default(),
                    orientation: Default::default(),
                    nodes: vec![LayoutNode {
                        layout: Some("1 window repeat 3".to_string()),
                        repeat: TileRepeat(1),
                        size: TileSize(1),
                    }],
                },
            ),
            (
                "2 windows 2 layouts".to_string(),
                LayoutSchema {
                    split: Default::default(),
                    orientation: Default::default(),
                    nodes: vec![
                        LayoutNode {
                            layout: None,
                            repeat: TileRepeat(1),
                            size: TileSize(1),
                        },
                        LayoutNode {
                            layout: Some("1 window repeat 3".to_string()),
                            repeat: TileRepeat(1),
                            size: TileSize(1),
                        },
                        LayoutNode {
                            layout: None,
                            repeat: TileRepeat(1),
                            size: TileSize(1),
                        },
                        LayoutNode {
                            layout: Some("1 window repeat 3".to_string()),
                            repeat: TileRepeat(1),
                            size: TileSize(1),
                        },
                    ],
                },
            ),
            (
                "nested layout".to_string(),
                LayoutSchema {
                    split: Default::default(),
                    orientation: Default::default(),
                    nodes: vec![LayoutNode {
                        layout: Some("1 layout".to_string()),
                        repeat: TileRepeat(1),
                        size: TileSize(1),
                    }],
                },
            ),
            (
                "1 empty 1 window".to_string(),
                LayoutSchema {
                    split: Default::default(),
                    orientation: Default::default(),
                    nodes: vec![
                        LayoutNode {
                            layout: Some("empty".to_string()),
                            repeat: TileRepeat(1),
                            size: TileSize(1),
                        },
                        LayoutNode {
                            layout: Some("empty".to_string()),
                            repeat: TileRepeat(1),
                            size: TileSize(1),
                        },
                        LayoutNode {
                            layout: None,
                            repeat: TileRepeat(1),
                            size: TileSize(1),
                        },
                    ],
                },
            ),
        ])
    });

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

    #[test_case("1 window repeat 3", 1,
    tile_tree!(layout() [
        window()
    ]); "1 simple insert")]
    #[test_case("1 window repeat 3", 3,
    tile_tree!(layout() [
        window(),
        window(),
        window()
    ]); "multiple simple insert")]
    #[test_case("3 windows", 4,
    tile_tree!(layout() [
        window(1),
        window(2),
        window(2),
        window(3)
    ]); "insert across nodes")]
    #[test_case("1 layout", 1,
    tile_tree!(layout() [
        layout() [
            window()
        ]
    ]); "insert into layout")]
    #[test_case("2 windows 2 layouts", 7,
    tile_tree!(layout() [
        window(),
        layout() [
            window(),
            window(),
            window()
        ],
        window(),
        layout() [
            window(),
            window(),
        ]
    ]); "insert across nodes and layouts")]
    #[test_case("nested layout", 1,
    tile_tree!(layout() [
        layout() [
            layout() [
                window()
            ]
        ]
    ]); "insert into nested layout")]
    #[test_case("1 empty 1 window", 1,
    tile_tree!(layout() [
        layout() [],
        layout() [],
        window()
    ]); "skip empty layout")]
    fn test_tile_tree_insertion_matches_expected(
        layout_name: &str,
        tiles: u32,
        expected: TileTree<TestWindow>,
    ) {
        let layouts = Rc::new(LayoutSet((*LAYOUT_SET).clone()));
        let mut tree = TileTree::<TestWindow>::new(layouts, layout_name);
        for _ in 0..tiles {
            tree.insert(TestWindow::new());
        }
        assert_tree_eq!(tree, expected);
    }

    #[test_case("1 window repeat 3", 1 => true; "simple")]
    #[test_case("1 window repeat 3", 4 => false; "full")]
    #[test_case("empty", 1 => false; "empty")]
    fn test_tile_tree_insertion(layout_name: &str, tiles: u32) -> bool {
        let layouts = Rc::new(LayoutSet((*LAYOUT_SET).clone()));
        let mut tree = TileTree::new(layouts, layout_name);
        for _ in 0..tiles - 1 {
            tree.insert(TestWindow::new());
        }
        tree.insert(TestWindow::new()).is_none()
    }

    #[test_case("1 window repeat 3", 3, 0,
    tile_tree!(layout() [
        window(; 1),
        window(; 2),
    ]); "remove first window")]
    #[test_case("1 window repeat 3", 3, 1,
    tile_tree!(layout() [
        window(; 0),
        window(; 2),
    ]); "remove middle window")]
    #[test_case("1 window repeat 3", 3, 2,
    tile_tree!(layout() [
        window(; 0),
        window(; 1),
    ]); "remove last window")]
    #[test_case("2 windows 2 layouts", 7, 3,
    tile_tree!(layout() [
        window(; 0),
        layout() [
            window(; 1),
            window(; 2),
            window(; 4)
        ],
        window(; 5),
        layout() [
            window(; 6),
        ]
    ]); "remove across window and layouts")]
    #[test_case("nested layout", 2, 0,
    tile_tree!(layout() [
        layout() [
            layout() [
                window(; 1)
            ]
        ]
    ]); "remove from nested layout")]
    fn test_tile_tree_removal_matches_expected(
        layout_name: &str,
        tiles: u32,
        remove: u32,
        expected: TileTree<TestWindow>,
    ) {
        let layouts = Rc::new(LayoutSet((*LAYOUT_SET).clone()));
        let mut tree = TileTree::<TestWindow>::new(layouts, layout_name);
        for i in 0..tiles {
            tree.insert(TestWindow {
                id: Some(i),
                ..Default::default()
            });
        }
        assert_eq!(tree.remove(remove).map(|w| w.id), Some(Some(remove)));
        assert_tree_eq!(tree, expected);
    }

    #[test_case(
        tile_tree!(layout() [window(), window()]),
        tile_tree!(layout() [window(, (0, 0), (50, 100)), window(, (50, 0), (50, 100))])
    ; "simple")]
    #[test_case(
        tile_tree!(layout() [window(1), window(3), window(2)]),
        tile_tree!(layout() [
            window(1, (0, 0), (16, 100)),
            window(3, (16, 0), (50, 100)),
            window(2, (66, 0), (34, 100))
        ])
    ; "tile size")]
    #[test_case(
        tile_tree!(layout(; Horizontal) [window(), window()]),
        tile_tree!(layout(; Horizontal) [window(, (0, 0), (100, 50)), window(, (0, 50), (100, 50))])
    ; "layout split")]
    #[test_case(
        tile_tree!(layout(, TopLeft) [window(), window()]),
        tile_tree!(layout(, TopLeft) [window(, (50, 0), (50, 100)), window(, (0, 0), (50, 100))])
    ; "layout orientation")]
    #[test_case(
        tile_tree!(layout() [
            window(),
            layout(; Horizontal, TopLeft) [
                window(),
                layout(; Horizontal) [
                    window(),
                    window(),
                ]
            ]
        ]),
        tile_tree!(layout() [
            window(, (0, 0), (50, 100)),
            layout(; Horizontal, TopLeft) [
                window(, (50, 50), (50, 50)),
                layout(; Horizontal) [
                    window(, (50, 0), (50, 25)),
                    window(, (50, 25), (50, 25)),
                ]
            ]
        ])
    ; "all")]
    fn test_tile_tree_update_toplevel_state(
        mut tree: TileTree<TestWindow>,
        expected: TileTree<TestWindow>,
    ) {
        tree.update_toplevel_state((0, 0).into(), (100, 100).into());
        assert_tree_eq!(tree, expected);
    }
}
