use std::collections::HashMap;

use smithay::utils::{Rectangle, Size};

use crate::utils::evenly_div;

#[derive(Default)]
pub struct LayoutGroups(HashMap<String, LayoutGroupKind>);

impl LayoutGroups {
    fn get(&self, key: &str) -> &LayoutGroupKind {
        self.0.get(key).unwrap_or(&LayoutGroupKind::None)
    }

    fn new_tree(&self, mut window_count: usize) -> WindowTree {
        let group = self.get("main");
        if let Some(split) = group.kind() {
            group.build_tree(self, split, &mut window_count)
        } else {
            WindowTree::default()
        }
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
                kind: WindowTreeNodeKind::Branch(group.build_tree(self, split, window_count)),
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

#[derive(Default)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct WindowTree {
    inner: Vec<WindowTreeNode>,
    count: usize,
    split: SplitDirection,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
struct WindowTreeNode {
    kind: WindowTreeNodeKind,
    size_ratio: f32,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
enum WindowTreeNodeKind {
    Node(usize),
    Branch(WindowTree),
}

#[derive(Clone, Copy, Default)]
#[cfg_attr(test, derive(Debug, PartialEq))]
enum SplitDirection {
    Horizontal,
    #[default]
    Vertical,
}

impl WindowTree {
    fn new(split: SplitDirection) -> Self {
        Self {
            inner: Vec::new(),
            count: 0,
            split,
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

    fn build_tree(
        &self,
        layout_splits: &LayoutGroups,
        split: SplitDirection,
        window_count: &mut usize,
    ) -> WindowTree {
        let mut iter = self.iter();
        let mut tree = WindowTree::new(split);
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

impl WindowTree {
    fn split_windows<T>(self, container: Rectangle<i32, T>) -> Vec<Rectangle<i32, T>> {
        let mut vec = Vec::new();
        let mut x = container.loc.x;
        let mut y = container.loc.y;
        let size_iter: Vec<_> = match self.split {
            SplitDirection::Horizontal => evenly_div(container.size.h, self.count as i32)
                .into_iter()
                .map(|h| Size::new(container.size.w, h))
                .collect(),
            SplitDirection::Vertical => evenly_div(container.size.w, self.count as i32)
                .into_iter()
                .map(|w| Size::new(w, container.size.h))
                .collect(),
        };
        let mut size_iter = size_iter.into_iter();

        for node in self.inner {
            match node.kind {
                WindowTreeNodeKind::Node(n) => {
                    for _ in 0..n {
                        let size = size_iter.next().unwrap();
                        vec.push(Rectangle::new((x, y).into(), size));
                        match self.split {
                            SplitDirection::Horizontal => {
                                y += size.h;
                            }
                            SplitDirection::Vertical => {
                                x += size.w;
                            }
                        }
                    }
                }
                WindowTreeNodeKind::Branch(tree) => {
                    let size = size_iter.next().unwrap();
                    vec.extend(tree.split_windows(Rectangle::new((x, y).into(), size)));
                    match self.split {
                        SplitDirection::Horizontal => {
                            y += size.h;
                        }
                        SplitDirection::Vertical => {
                            x += size.w;
                        }
                    }
                }
            }
        }
        vec
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_mut)]
    #![allow(unused_assignments)]

    use super::*;
    use smithay::utils::Logical;
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
            WindowTree::default()
        };
        ($split:ident; $($kind:ident ($($arg:expr),*)),*; $count:expr) => {
            WindowTree {
                inner: vec![$(window_tree_node!($kind ($($arg),*))),*],
                count: $count,
                split: SplitDirection::$split,
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
        (branch($tree:expr $(,$ratio:expr)?)) => {
            WindowTreeNode {
                kind: WindowTreeNodeKind::Branch($tree),
                size_ratio: { let mut r = 1.0; $( r = $ratio; )? r },
            }
        };
    }

    #[test_case(layout_groups!(), 1 => window_tree!(); "empty")]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1)),
        1 => window_tree!(Horizontal; node(1); 1);
        "single_window"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(2)),
        1 => window_tree!(Horizontal; node(1); 1);
        "more_slots_than_windows"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1)),
        2 => window_tree!(Horizontal; node(1); 1);
        "more_windows_than_slots"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1)),
        0 => window_tree!(Horizontal; ; 0);
        "windows_zero"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1), window(1)),
        3 => window_tree!(Horizontal; node(1), node(1); 2);
        "horizontal_two_windows"
    )]
    #[test_case(
        layout_groups!("main" => Vertical => window(1), window(1)),
        2 => window_tree!(Vertical; node(1), node(1); 2);
        "vertical_two_windows"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1), window(1), window(1)),
        4 => window_tree!(Horizontal; node(1), node(1), node(1); 3);
        "three_windows"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1, 0.5), window(2, 1.5)),
        3 => window_tree!(Horizontal; node(1,0.5), node(2,1.5); 3);
        "with_size_ratios"
    )]
    #[test_case(
        layout_groups!("main" => Vertical => window(1, 0.3), window(2, 0.7)),
        3 => window_tree!(Vertical; node(1,0.3), node(2,0.7); 3);
        "vertical_with_ratios"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1), group("group"); "group" => Vertical => window(2)),
        3 => window_tree!(Horizontal; node(1), branch(window_tree!(Vertical; node(2); 2)); 2);
        "simple_group"
    )]
    #[test_case(
        layout_groups!(
            "main" => Horizontal => window(1, 0.5), group("sub", 1.5);
            "sub" => Vertical => window(1, 0.3), window(2, 0.7)
        ),
        4 =>
        window_tree!(
            Horizontal;
            node(1,0.5),
            branch(window_tree!(Vertical; node(1,0.3), node(2,0.7); 3), 1.5);
            2
        );
        "nested_with_ratios"
    )]
    #[test_case(
        layout_groups!(
            "main" => Horizontal => group("a"), group("b");
            "a" => Vertical => window(1);
            "b" => Horizontal => window(2)
        ),
        3 =>
        window_tree!(
            Horizontal;
            branch(window_tree!(Vertical; node(1); 1)),
            branch(window_tree!(Horizontal; node(2); 2));
            2
        );
        "multiple_groups_horizontal"
    )]
    #[test_case(
        layout_groups!(
            "main" => Vertical => group("a"), group("b");
            "a" => Horizontal => window(1);
            "b" => Vertical => window(2)
        ),
        3 =>
        (window_tree!(
            Vertical;
            branch(window_tree!(Horizontal; node(1); 1)),
            branch(window_tree!(Vertical; node(2); 2));
            2
        ));
        "vertical_with_groups"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1), group("missing")),
        2 => window_tree!(Horizontal; node(1); 1);
        "missing_group"
    )]
    #[test_case(
        layout_groups!(
            "main" => Horizontal => group("level1");
            "level1" => Vertical => group("level2");
            "level2" => Horizontal => window(3)
        ),
        3 =>
        window_tree!(
            Horizontal; 
            branch(window_tree!(
                Vertical;
                branch(window_tree!(Horizontal; node(3); 3))
            ; 1)
        ); 1);
        "deep_nesting"
    )]
    #[test_case(
        layout_groups!("main" => Horizontal => window(1), group("main")),
        3 =>
        window_tree!(
            Horizontal; 
            node(1),
            branch(window_tree!(
                Horizontal;
                node(1),
                branch(window_tree!(Horizontal; node(1); 1))
            ; 2)
        ); 2);
        "circular_group"
    )]
    fn test_build_tree(layout_splits: LayoutGroups, windows: usize) -> WindowTree {
        layout_splits.new_tree(windows)
    }

    macro_rules! rectangle {
        ($x:expr, $y:expr, $w:expr, $h:expr) => {
            Rectangle::<_, Logical>::new(($x, $y).into(), ($w, $h).into())
        };
    }
}
