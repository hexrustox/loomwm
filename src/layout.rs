use std::collections::HashMap;

#[derive(Default)]
pub struct LayoutGroups(HashMap<String, LayoutGroupKind>);

impl LayoutGroups {
    fn get(&self, key: &str) -> &LayoutGroupKind {
        self.0.get(key).unwrap_or(&LayoutGroupKind::None)
    }

    fn new_tree(&self, mut window_count: usize) -> WindowTree {
        let mut tree = WindowTree::new();
        self.extend_tree(&mut window_count, "main", &mut tree, 1.0);
        tree
    }

    fn extend_tree(
        &self,
        window_count: &mut usize,
        name: &str,
        tree: &mut WindowTree,
        size_ratio: f32,
    ) {
        let group = self.get(name);
        if let Some(split) = group.kind() {
            tree.push(WindowTreeNode {
                kind: WindowTreeNodeKind::Branch(split, group.build_tree(self, window_count)),
                size_ratio,
            });
        }
    }
}

pub enum LayoutGroupKind {
    Horizontal(Vec<LayoutGroupItem>),
    Vertical(Vec<LayoutGroupItem>),
    None,
}

pub struct LayoutGroupItem {
    kind: LayoutGroupItemKind,
    size_ratio: Option<f32>,
}

pub enum LayoutGroupItemKind {
    Window(usize),
    Group(String),
}

#[cfg_attr(test, derive(Debug, PartialEq))]
struct WindowTree {
    inner: Vec<WindowTreeNode>,
    count: usize,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
struct WindowTreeNode {
    kind: WindowTreeNodeKind,
    size_ratio: f32,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
enum WindowTreeNodeKind {
    Node(usize),
    Branch(SplitDirection, WindowTree),
}

#[cfg_attr(test, derive(Debug, PartialEq))]
enum SplitDirection {
    Horizontal,
    Vertical,
}

impl WindowTree {
    fn new() -> Self {
        Self {
            inner: Vec::new(),
            count: 0,
        }
    }
    fn push(&mut self, value: WindowTreeNode) {
        match value.kind {
            WindowTreeNodeKind::Node(n) => {
                self.count += n;
            }
            WindowTreeNodeKind::Branch(..) => {
                self.count += 1;
            }
        }
        self.inner.push(value);
    }
}

impl LayoutGroupKind {
    fn iter(&self) -> Box<dyn Iterator<Item = &LayoutGroupItem> + '_> {
        match self {
            Self::Horizontal(x) => Box::new(x.iter()),
            Self::Vertical(x) => Box::new(x.iter()),
            Self::None => Box::new(std::iter::empty()),
        }
    }

    fn kind(&self) -> Option<SplitDirection> {
        match self {
            Self::Horizontal(_) => Some(SplitDirection::Horizontal),
            Self::Vertical(_) => Some(SplitDirection::Vertical),
            Self::None => None,
        }
    }

    fn build_tree(&self, layout_splits: &LayoutGroups, window_count: &mut usize) -> WindowTree {
        let mut iter = self.iter();
        let mut tree = WindowTree::new();
        while *window_count > 0
            && let Some(LayoutGroupItem { kind, size_ratio }) = iter.next()
        {
            match kind {
                LayoutGroupItemKind::Window(n) => {
                    let left = window_count.saturating_sub(*n);
                    let used = *window_count - left;
                    tree.push(WindowTreeNode {
                        kind: WindowTreeNodeKind::Node(used),
                        size_ratio: size_ratio.unwrap_or(1.0),
                    });

                    *window_count = left;
                }
                LayoutGroupItemKind::Group(group) => {
                    layout_splits.extend_tree(
                        window_count,
                        group,
                        &mut tree,
                        size_ratio.unwrap_or(1.0),
                    );
                }
            }
        }

        tree
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_mut)]
    #![allow(unused_assignments)]

    use super::*;
    use test_case::test_case;

    macro_rules! layout_groups {
        ($($key:expr => $split:ident => $($kind:ident ($($arg:expr),*)),*);*) => {{
            #[allow(unused_mut)]
            let mut map = HashMap::new();
            $({
                let items = vec![$(layout_group_item!($kind ($($arg),*))),*];
                map.insert($key.to_string(), LayoutGroupKind::$split(items));
            })*
            LayoutGroups(map)
        }};
    }

    macro_rules! layout_group_item {
        (window($count:expr $(,$ratio:expr)?)) => {
            LayoutGroupItem {
                kind: LayoutGroupItemKind::Window($count),
                size_ratio: { let mut r = None; $( r = Some($ratio); )? r },
            }
        };
        (group($group:literal $(,$ratio:expr)?)) => {
            LayoutGroupItem {
                kind: LayoutGroupItemKind::Group($group.to_string()),
                size_ratio: { let mut r = None; $( r = Some($ratio); )? r },
            }
        };
    }

    macro_rules! window_tree {
        () => {
            WindowTree::new()
        };
        ($($kind:ident ($($arg:expr),*)),*; $count:expr) => {
            WindowTree {
                inner: vec![$(window_tree_node!($kind ($($arg),*))),*],
                count: $count,
            }
        }
    }

    macro_rules! window_tree_node {
        (node($count:expr $(,$ratio:expr)?)) => {
            WindowTreeNode {
                kind: WindowTreeNodeKind::Node($count),
                size_ratio: { let mut r = 1.0; $( r = $ratio; )? r },
            }
        };
        (horizontal($tree:expr $(,$ratio:expr)?)) => {
            WindowTreeNode {
                kind: WindowTreeNodeKind::Branch(SplitDirection::Horizontal, $tree),
                size_ratio: { let mut r = 1.0; $( r = $ratio; )? r },
            }
        };
        (vertical($tree:expr $(,$ratio:expr)?)) => {
            WindowTreeNode {
                kind: WindowTreeNodeKind::Branch(SplitDirection::Vertical, $tree),
                size_ratio: { let mut r = 1.0; $( r = $ratio; )? r },
            }
        };
    }

    #[test_case(layout_groups!(), 1 => window_tree!(); "empty")]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1)),
        1 => window_tree!(horizontal(window_tree!(node(1); 1)); 1);
        "single_window"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(2)),
        1 => window_tree!(horizontal(window_tree!(node(1); 1)); 1);
        "more_slots_than_windows"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1)),
        2 => window_tree!(horizontal(window_tree!(node(1); 1)); 1);
        "more_windows_than_slots"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1)),
        0 => window_tree!(horizontal(window_tree!()); 1);
        "windows_zero"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1), window(1)),
        3 => window_tree!(horizontal(window_tree!(node(1), node(1); 2)); 1);
        "horizontal_two_windows"
    )]
    #[test_case(
        layout_groups!("main" => Vertical => window(1), window(1)),
        2 => window_tree!(vertical(window_tree!(node(1), node(1); 2)); 1);
        "vertical_two_windows"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1), window(1), window(1)),
        4 => window_tree!(horizontal(window_tree!(node(1), node(1), node(1); 3)); 1);
        "three_windows"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1, 0.5), window(2, 1.5)),
        3 => window_tree!(horizontal(window_tree!(node(1,0.5), node(2,1.5); 3)); 1);
        "with_size_ratios"
    )]
    #[test_case(
        layout_groups!("main" => Vertical => window(1, 0.3), window(2, 0.7)),
        3 => window_tree!(vertical(window_tree!(node(1,0.3), node(2,0.7); 3)); 1);
        "vertical_with_ratios"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1), group("group"); "group" => Vertical => window(2)),
        3 => window_tree!(horizontal(window_tree!(node(1), vertical(window_tree!(node(2); 2)); 2)); 1);
        "simple_group"
    )]
    #[test_case(
        layout_groups!(
            "main" => Horizontal => window(1, 0.5), group("sub", 1.5);
            "sub" => Vertical => window(1, 0.3), window(2, 0.7)
        ),
        4 =>
        window_tree!(horizontal(window_tree!(
            node(1,0.5),
            vertical(window_tree!(node(1,0.3), node(2,0.7); 3), 1.5);
            2
        )); 1);
        "nested_with_ratios"
    )]
    #[test_case(
        layout_groups!(
            "main" => Horizontal => group("a"), group("b");
            "a" => Vertical => window(1);
            "b" => Horizontal => window(2)
        ),
        3 =>
        window_tree!(horizontal(window_tree!(
            vertical(window_tree!(node(1); 1)),
            horizontal(window_tree!(node(2); 2));
            2
        )); 1);
        "multiple_groups_horizontal"
    )]
    #[test_case(
        layout_groups!(
            "main" => Vertical => group("a"), group("b");
            "a" => Horizontal => window(1);
            "b" => Vertical => window(2)
        ),
        3 =>
        window_tree!(vertical(window_tree!(
            horizontal(window_tree!(node(1); 1)),
            vertical(window_tree!(node(2); 2));
            2
        )); 1);
        "vertical_with_groups"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1), group("missing")),
        2 => window_tree!(horizontal(window_tree!(node(1); 1)); 1);
        "missing_group"
    )]
    #[test_case(
        layout_groups!(
            "main" => Horizontal => group("level1");
            "level1" => Vertical => group("level2");
            "level2" => Horizontal => window(3)
        ),
        3 =>
        window_tree!(horizontal(window_tree!(
            vertical(window_tree!(
                horizontal(window_tree!(node(3); 3))
            ; 1)
        ); 1)); 1);
        "deep_nesting"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1), group("main")),
        3 =>
        window_tree!(horizontal(window_tree!(
            node(1),
            horizontal(window_tree!(
                node(1),
                horizontal(window_tree!(node(1); 1))
            ; 2)
        ); 2)); 1);
        "circular_group"
    )]
    fn test_build_tree(layout_splits: LayoutGroups, windows: usize) -> WindowTree {
        layout_splits.new_tree(windows)
    }
}
