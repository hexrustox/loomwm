use slotmap::{SlotMap, new_key_type};
use std::{borrow::Cow, collections::HashMap, rc::Rc};

#[cfg(test)]
use tests::MappedWindow;

#[cfg(not(test))]
use crate::window::MappedWindow;
#[cfg(not(test))]
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

new_key_type! { struct TileId; }

type TileArena = SlotMap<TileId, Tile>;

#[derive(Debug)]
struct TileTree {
    arena: TileArena,
    root: TileId,
    current_tile: TileId,
    layouts: Rc<LayoutSet>,
    current_layout: String,
    layout_trace: Vec<TileLayoutTrace>,
}

#[derive(Debug)]
struct Tile {
    kind: TileKind,
    size: TileSize,
    parent: Option<TileId>,
}

impl Tile {
    fn as_window(self) -> TileWindow {
        match self.kind {
            TileKind::Window(x) => x,
            TileKind::Layout(_) => panic!("Expected `Tile` to be a `Window`, but found a `Layout`"),
        }
    }

    fn as_layout(self) -> TileLayout {
        match self.kind {
            TileKind::Layout(x) => x,
            TileKind::Window(_) => panic!("Expected `Tile` to be a `Layout`, but found a `Window`"),
        }
    }

    fn get_layout(&self) -> &TileLayout {
        match &self.kind {
            TileKind::Layout(x) => x,
            TileKind::Window(_) => panic!("Expected `Tile` to be a `Layout`, but found a `Window`"),
        }
    }

    fn get_layout_mut(&mut self) -> &mut TileLayout {
        match &mut self.kind {
            TileKind::Layout(x) => x,
            TileKind::Window(_) => panic!("Expected `Tile` to be a `Layout`, but found a `Window`"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(PartialEq))]
struct TileSize(f64);

#[derive(Debug)]
enum TileKind {
    Window(TileWindow),
    Layout(TileLayout),
}

#[derive(Debug)]
struct TileWindow {
    window: MappedWindow,
}

#[derive(Debug)]
struct TileLayout {
    split: TileSplit,
    orientation: TileOrientation,
    tiles: Vec<TileId>,
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
struct LayoutSet(HashMap<String, LayoutSchema>);

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

#[derive(Debug, Clone)]
struct LayoutNode {
    layout: Option<String>,
    repeat: TileRepeat,
    size: TileSize,
}

#[derive(Debug, Clone)]
struct TileRepeat(usize);

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

impl TileTree {
    fn new(layouts: Rc<LayoutSet>, layout_name: &str) -> Self {
        let mut arena = SlotMap::with_key();
        let layout = layouts.get(layout_name);

        let new_tile = Tile {
            kind: TileKind::Layout(TileLayout {
                split: layout.split,
                orientation: layout.orientation,
                tiles: Vec::new(),
            }),
            size: TileSize(1.0),
            parent: None,
        };
        let root = arena.insert(new_tile);

        Self {
            arena,
            root,
            current_tile: root,
            layouts,
            current_layout: layout_name.to_string(),
            layout_trace: vec![TileLayoutTrace::new(layout_name)],
        }
    }

    fn insert(&mut self, window: MappedWindow) -> bool {
        let Some(trace) = self.layout_trace.last_mut() else {
            return false;
        };

        let layout = self.layouts.get(&trace.layout);
        for i in trace.index..layout.nodes.len() {
            trace.index = i;
            let node = &layout.nodes[i];
            if trace.tile_count < node.repeat.0 {
                if let Some(layout_name) = &node.layout {
                    let layout = self.layouts.get(layout_name);
                    let new_tile = Tile {
                        kind: TileKind::Layout(TileLayout {
                            split: layout.split,
                            orientation: layout.orientation,
                            tiles: Vec::new(),
                        }),
                        size: node.size,
                        parent: Some(self.current_tile),
                    };
                    let tile_id = self.arena.insert(new_tile);

                    self.arena[self.current_tile]
                        .get_layout_mut()
                        .tiles
                        .push(tile_id);

                    self.current_tile = tile_id;
                    self.layout_trace.push(TileLayoutTrace::new(layout_name));
                    self.insert(window);
                } else {
                    let new_tile = Tile {
                        kind: TileKind::Window(TileWindow { window }),
                        size: node.size,
                        parent: Some(self.current_tile),
                    };
                    let tile_id = self.arena.insert(new_tile);

                    self.arena[self.current_tile]
                        .get_layout_mut()
                        .tiles
                        .push(tile_id);

                    trace.tile_count += 1;
                }
                return true;
            }
            trace.tile_count = 0;
        }

        let Some(parent) = self.arena[self.current_tile].parent else {
            return false;
        };
        self.current_tile = parent;
        self.layout_trace.pop();
        self.layout_trace.last_mut().unwrap().index += 1;
        self.insert(window)
    }

    fn remove(
        &mut self,
        #[cfg(test)] window_id: u32,
        #[cfg(not(test))] wl_surface: &WlSurface,
    ) -> Option<MappedWindow> {
        let predicate = {
            #[cfg(test)]
            {
                |mapped: &MappedWindow| mapped.0 == Some(window_id)
            }
            #[cfg(not(test))]
            {
                |mapped: &MappedWindow| {
                    mapped.inner.toplevel().map(|t| t.wl_surface()) == Some(wl_surface)
                }
            }
        };

        struct RemoveTile {
            parent: TileId,
            index: usize,
        }

        fn traverse<F>(arena: &TileArena, layout_id: TileId, predicate: &F) -> Option<RemoveTile>
        where
            F: Fn(&MappedWindow) -> bool,
        {
            let mut res = None;
            for (index, tile_id) in arena[layout_id].get_layout().tiles.iter().enumerate() {
                match &arena[*tile_id] {
                    Tile {
                        kind: TileKind::Window(TileWindow { window }),
                        ..
                    } => {
                        if predicate(window) {
                            res = Some(RemoveTile {
                                parent: layout_id,
                                index,
                            });
                        }
                    }
                    _ => {
                        if let sub_res @ Some(_) = traverse(arena, *tile_id, predicate) {
                            res = sub_res;
                        }
                    }
                }
            }

            res
        }

        if let Some(RemoveTile { parent, index }) = traverse(&self.arena, self.root, &predicate) {
            let remove_id = self.arena[parent].get_layout_mut().tiles.remove(index);
            let window = self.arena.remove(remove_id).unwrap().as_window().window;

            let mut handle_ids = Vec::new();
            for id in &self.arena[self.root].get_layout().tiles {
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
                        extracted_windows.push(self.arena.remove(id).unwrap().as_window().window);
                    }
                    Tile {
                        kind: TileKind::Layout(..),
                        ..
                    } => {
                        handle_ids.splice(
                            i + 1..i + 1,
                            self.arena.remove(id).unwrap().as_layout().tiles,
                        );
                    }
                }

                i += 1;
            }

            *self = Self::new(self.layouts.clone(), &self.current_layout);
            for window in extracted_windows {
                self.insert(window);
            }

            return Some(window);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use super::*;

    #[derive(Debug, PartialEq)]
    pub struct MappedWindow(pub Option<u32>);

    impl PartialEq for TileTree {
        fn eq(&self, other: &Self) -> bool {
            fn compare_tiles(
                tree_a: &TileTree,
                id_a: TileId,
                tree_b: &TileTree,
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
                    (TileKind::Window(win_a), TileKind::Window(win_b)) => {
                        win_a.window == win_b.window
                    }
                    (TileKind::Layout(grp_a), TileKind::Layout(grp_b)) => {
                        if grp_a.split != grp_b.split || grp_a.orientation != grp_b.orientation {
                            return false;
                        }
                        if grp_a.tiles.len() != grp_b.tiles.len() {
                            return false;
                        }

                        grp_a
                            .tiles
                            .iter()
                            .zip(grp_a.tiles.iter())
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

    impl TileTree {
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
                TileKind::Window(TileWindow { window }) => {
                    f.push_str(&format!(
                        "Window {:?} (size: {:?})\n",
                        window.0, tile.size.0
                    ));
                }
                TileKind::Layout(layout) => {
                    f.push_str(&format!(
                        "Layout {:?} {:?} (size: {:?})\n",
                        layout.split, layout.orientation, tile.size.0
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

                    for (i, child_id) in layout.tiles.iter().enumerate() {
                        let last_child = i == layout.tiles.len() - 1;
                        self.format_node(*child_id, &new_prefix, false, last_child, f);
                    }
                }
            }
        }
    }

    macro_rules! tile_tree {
        (@node $arena:ident, $parent:expr, window($size:expr$(, $id:expr)?)) => {
            $arena.insert(Tile {
                kind: TileKind::Window(TileWindow {
                    window: {
                        #[allow(unused_mut)]
                        #[allow(unused_assignments)]
                        let mut window = MappedWindow(None);
                        $(window = MappedWindow(Some($id));)?
                        window
                    }
                }),
                size: TileSize($size),
                parent: $parent,
            })
        };

        (@node $arena:ident, $parent:expr, layout($size:expr, $split:ident$(, $orient:ident)?) [ $($child_kind:ident ( $($child_args:tt)* ) $( [ $($child_inner:tt)* ] )? ),* $(,)? ]) => {{
            let layout_id = $arena.insert_with_key(|_| Tile {
                kind: TileKind::Layout(TileLayout {
                    split: TileSplit::$split,
                    orientation: {
                        #[allow(unused_mut)]
                        #[allow(unused_assignments)]
                        let mut orient = TileOrientation::default();
                        $(orient = TileOrientation::$orient;)?
                        orient

                    },
                    tiles: Vec::new(),
                }),
                size: TileSize($size),
                parent: $parent,
            });

            let children = vec![
                $(
                    tile_tree!(@node $arena, Some(layout_id), $child_kind ( $($child_args)* ) $( [ $($child_inner)* ] )? )
                ),*
            ];

            if let TileKind::Layout(ref mut g) = $arena[layout_id].kind {
                g.tiles = children;
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
                current_layout: "".to_string(),
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
                        size: TileSize(1.0),
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
                            size: TileSize(0.1),
                        },
                        LayoutNode {
                            layout: None,
                            repeat: TileRepeat(2),
                            size: TileSize(0.2),
                        },
                        LayoutNode {
                            layout: None,
                            repeat: TileRepeat(1),
                            size: TileSize(0.3),
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
                        size: TileSize(1.0),
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
                            size: TileSize(1.0),
                        },
                        LayoutNode {
                            layout: Some("1 window repeat 3".to_string()),
                            repeat: TileRepeat(1),
                            size: TileSize(1.0),
                        },
                        LayoutNode {
                            layout: None,
                            repeat: TileRepeat(1),
                            size: TileSize(1.0),
                        },
                        LayoutNode {
                            layout: Some("1 window repeat 3".to_string()),
                            repeat: TileRepeat(1),
                            size: TileSize(1.0),
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
                        size: TileSize(1.0),
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
                            size: TileSize(1.0),
                        },
                        LayoutNode {
                            layout: Some("empty".to_string()),
                            repeat: TileRepeat(1),
                            size: TileSize(1.0),
                        },
                        LayoutNode {
                            layout: None,
                            repeat: TileRepeat(1),
                            size: TileSize(1.0),
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

    use test_case::test_case;
    #[test_case("1 window repeat 3", 1,
    tile_tree!(layout(1.0, Vertical) [
        window(1.0)
    ]); "1 simple insert")]
    #[test_case("1 window repeat 3", 3,
    tile_tree!(layout(1.0, Vertical) [
        window(1.0),
        window(1.0),
        window(1.0)
    ]); "multiple simple insert")]
    #[test_case("3 windows", 4,
    tile_tree!(layout(1.0, Vertical) [
        window(0.1),
        window(0.2),
        window(0.2),
        window(0.3)
    ]); "insert across nodes")]
    #[test_case("1 layout", 1,
    tile_tree!(layout(1.0, Vertical) [
        layout(1.0, Vertical) [
            window(1.0)
        ]
    ]); "insert into layout")]
    #[test_case("2 windows 2 layouts", 7,
    tile_tree!(layout(1.0, Vertical) [
        window(1.0),
        layout(1.0, Vertical) [
            window(1.0),
            window(1.0),
            window(1.0)
        ],
        window(1.0),
        layout(1.0, Vertical) [
            window(1.0),
            window(1.0),
        ]
    ]); "insert across nodes and layouts")]
    #[test_case("nested layout", 1,
    tile_tree!(layout(1.0, Vertical) [
        layout(1.0, Vertical) [
            layout(1.0, Vertical) [
                window(1.0)
            ]
        ]
    ]); "insert into nested layout")]
    #[test_case("1 empty 1 window", 1,
    tile_tree!(layout(1.0, Vertical) [
        layout(1.0, Vertical) [],
        layout(1.0, Vertical) [],
        window(1.0)
    ]); "skip empty layout")]
    fn test_tile_tree_insertion_matches_expected(
        layout_name: &str,
        tiles: u32,
        expected: TileTree,
    ) {
        let layouts = Rc::new(LayoutSet((*LAYOUT_SET).clone()));
        let mut tree = TileTree::new(layouts, layout_name);
        for _ in 0..tiles {
            tree.insert(MappedWindow(None));
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
            tree.insert(MappedWindow(None));
        }
        tree.insert(MappedWindow(None))
    }

    #[test_case("1 window repeat 3", 3, 0,
    tile_tree!(layout(1.0, Vertical) [
        window(1.0, 1),
        window(1.0, 2),
    ]); "remove first window")]
    #[test_case("1 window repeat 3", 3, 1,
    tile_tree!(layout(1.0, Vertical) [
        window(1.0, 0),
        window(1.0, 2),
    ]); "remove middle window")]
    #[test_case("1 window repeat 3", 3, 2,
    tile_tree!(layout(1.0, Vertical) [
        window(1.0, 0),
        window(1.0, 1),
    ]); "remove last window")]
    #[test_case("2 windows 2 layouts", 7, 3,
    tile_tree!(layout(1.0, Vertical) [
        window(1.0, 0),
        layout(1.0, Vertical) [
            window(1.0, 1),
            window(1.0, 2),
            window(1.0, 4)
        ],
        window(1.0, 5),
        layout(1.0, Vertical) [
            window(1.0, 6),
        ]
    ]); "remove across window and layouts")]
    #[test_case("nested layout", 2, 0,
    tile_tree!(layout(1.0, Vertical) [
        layout(1.0, Vertical) [
            layout(1.0, Vertical) [
                window(1.0, 1)
            ]
        ]
    ]); "remove from nested layout")]
    fn test_tile_tree_removal_matches_expected(
        layout_name: &str,
        tiles: u32,
        remove: u32,
        expected: TileTree,
    ) {
        let layouts = Rc::new(LayoutSet((*LAYOUT_SET).clone()));
        let mut tree = TileTree::new(layouts, layout_name);
        for i in 0..tiles {
            tree.insert(MappedWindow(Some(i)));
        }
        assert_eq!(tree.remove(remove), Some(MappedWindow(Some(remove))));
        assert_tree_eq!(tree, expected);
    }
}
