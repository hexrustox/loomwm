use slotmap::{SlotMap, new_key_type};
use std::{collections::HashMap, rc::Rc};

#[cfg(not(test))]
use crate::window::MappedWindow;
#[cfg(test)]
use tests::MappedWindow;

new_key_type! { struct TileId; }

#[derive(Debug)]
struct TileTree {
    arena: SlotMap<TileId, Tile>,
    root: TileId,
    current_tile: TileId,
    layouts: Rc<TileLayouts>,
    layout_trace: Vec<TileLayoutTrace>,
}

#[derive(Debug)]
struct Tile {
    kind: TileKind,
    size: TileSize,
    parent: Option<TileId>,
}

impl Tile {
    fn as_group_mut(&mut self) -> &mut TileGroup {
        match &mut self.kind {
            TileKind::Group(group) => group,
            TileKind::Window(_) => panic!("Expected Tile to be a Group, but found a Window"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(PartialEq))]
struct TileSize(f64);

#[derive(Debug)]
enum TileKind {
    Window(TileWindow),
    Group(TileGroup),
}

#[derive(Debug)]
struct TileWindow {
    window: MappedWindow,
}

#[derive(Debug)]
struct TileGroup {
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
struct TileLayouts(HashMap<String, TileLayout>);

impl TileLayouts {
    fn get(&self, layout_name: &str) -> &TileLayout {
        // TODO
        self.0.get(layout_name).unwrap()
    }
}

#[derive(Debug)]
struct TileLayout {
    split: TileSplit,
    orientation: TileOrientation,
    specs: Vec<TileSpec>,
}

#[derive(Debug)]
struct TileSpec {
    group: Option<String>,
    repeat: TileRepeat,
    size: TileSize,
}

#[derive(Debug)]
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
    fn new(layouts: Rc<TileLayouts>, layout_name: &str) -> Self {
        let mut arena = SlotMap::with_key();
        let layout = layouts.get(layout_name);

        let new_tile = Tile {
            kind: TileKind::Group(TileGroup {
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
            layout_trace: vec![TileLayoutTrace::new(layout_name)],
        }
    }

    fn add(&mut self, mapped: MappedWindow) {
        let Some(trace) = self.layout_trace.last_mut() else {
            // TODO
            panic!();
        };

        let layout = self.layouts.get(&trace.layout);
        for i in trace.index..layout.specs.len() {
            trace.index = i;
            let spec = &layout.specs[i];
            if trace.tile_count < spec.repeat.0 {
                if let Some(group) = &spec.group {
                } else {
                    let new_tile = Tile {
                        kind: TileKind::Window(TileWindow { window: mapped }),
                        size: spec.size,
                        parent: Some(self.current_tile),
                    };
                    let id = self.arena.insert(new_tile);

                    let current_group = self.arena.get_mut(self.current_tile).unwrap();
                    current_group.as_group_mut().tiles.push(id);

                    trace.tile_count += 1;
                }
                break;
            }
            trace.tile_count = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    pub struct MappedWindow;

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
                    (TileKind::Group(grp_a), TileKind::Group(grp_b)) => {
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
                TileKind::Window(_) => {
                    f.push_str(&format!("Window (size: {:?})\n", tile.size.0));
                }
                TileKind::Group(group) => {
                    f.push_str(&format!(
                        "Group {:?} {:?} (size: {:?})\n",
                        group.split, group.orientation, tile.size.0
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

                    for (i, child_id) in group.tiles.iter().enumerate() {
                        let last_child = i == group.tiles.len() - 1;
                        self.format_node(*child_id, &new_prefix, false, last_child, f);
                    }
                }
            }
        }
    }

    macro_rules! tile_tree {
        (@node $arena:ident, $parent:expr, window($size:expr)) => {
            $arena.insert(Tile {
                kind: TileKind::Window(TileWindow { window: MappedWindow }),
                size: TileSize($size),
                parent: $parent,
            })
        };

        (@node $arena:ident, $parent:expr, group($size:expr, $split:ident, $orient:ident) [ $($child_kind:ident ( $($child_args:tt)* ) $( [ $($child_inner:tt)* ] )? ),* $(,)? ]) => {{
            let group_id = $arena.insert_with_key(|_| Tile {
                kind: TileKind::Group(TileGroup {
                    split: TileSplit::$split,
                    orientation: TileOrientation::$orient,
                    tiles: Vec::new(),
                }),
                size: TileSize($size),
                parent: $parent,
            });

            let children = vec![
                $(
                    tile_tree!(@node $arena, Some(group_id), $child_kind ( $($child_args)* ) $( [ $($child_inner)* ] )? )
                ),*
            ];

            if let TileKind::Group(ref mut g) = $arena[group_id].kind {
                g.tiles = children;
            }
            group_id
        }};

        ($kind:ident ( $($args:tt)* ) $( [ $($inner:tt)* ] )?) => {{
            let mut arena = slotmap::SlotMap::with_key();
            let root = tile_tree!(@node arena, None, $kind ( $($args)* ) $( [ $($inner)* ] )?);
            TileTree { arena, root, layouts: Rc::new(TileLayouts(HashMap::new())), current_tile: root, layout_trace: Vec::new() }
        }};
    }

    macro_rules! layouts {
        () => {
            TileLayouts(HashMap::from_iter([
                (
                    "a".to_string(),
                    TileLayout {
                        split: Default::default(),
                        orientation: Default::default(),
                        specs: vec![TileSpec {
                            group: None,
                            repeat: TileRepeat(10),
                            size: TileSize(1.0),
                        }],
                    },
                ),
                (
                    "b".to_string(),
                    TileLayout {
                        split: Default::default(),
                        orientation: Default::default(),
                        specs: vec![
                            TileSpec {
                                group: None,
                                repeat: TileRepeat(1),
                                size: TileSize(0.1),
                            },
                            TileSpec {
                                group: None,
                                repeat: TileRepeat(2),
                                size: TileSize(0.2),
                            },
                            TileSpec {
                                group: None,
                                repeat: TileRepeat(1),
                                size: TileSize(0.3),
                            },
                        ],
                    },
                ),
            ]))
        };
    }

    use test_case::test_case;
    #[test_case("a",
    |tree| {
        tree.add(MappedWindow);
    },
    tile_tree!(group(1.0, Vertical, BottomRight) [
        window(1.0)
    ]); "1 simple insert")]
    #[test_case("a",
    |tree| {
        tree.add(MappedWindow);
        tree.add(MappedWindow);
        tree.add(MappedWindow);
    },
    tile_tree!(group(1.0, Vertical, BottomRight) [
        window(1.0),
        window(1.0),
        window(1.0)
    ]); "multiple simple insert")]
    #[test_case("b",
    |tree| {
        tree.add(MappedWindow);
        tree.add(MappedWindow);
        tree.add(MappedWindow);
        tree.add(MappedWindow);
    },
    tile_tree!(group(1.0, Vertical, BottomRight) [
        window(0.1),
        window(0.2),
        window(0.2),
        window(0.3)
    ]); "insert across specs")]
    fn test_tile_tree<T: FnOnce(&mut TileTree)>(
        layout_name: &str,
        callback: T,
        expected: TileTree,
    ) {
        let layouts = Rc::new(layouts!());
        let mut tree = TileTree::new(layouts, layout_name);
        callback(&mut tree);
        if tree != expected {
            println!("Expected:");
            println!("{}", expected.visualize());
            println!("Get:");
            println!("{}", tree.visualize());
            println!("{:?}", tree.layout_trace);
            panic!("Expected tree differ from tree got");
        }
    }
}
