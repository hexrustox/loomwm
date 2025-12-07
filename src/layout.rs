use std::collections::HashMap;

use smithay::utils::{Rectangle, Size};

use crate::utils::distribute_evenly;

const DEFAULT_SIZE: f32 = 1.0;

#[derive(Default)]
pub struct LayoutTemplates(HashMap<String, LayoutTemplate>);

impl LayoutTemplates {
    fn get(&self, key: &str) -> &LayoutTemplate {
        self.0.get(key).unwrap_or(&LayoutTemplate::None)
    }

    fn build_tree_from_template(&self, mut window_count: usize) -> WindowTree {
        let template = self.get("main");
        if let Some(direction) = template.kind() {
            template.build_tree(self, direction, &mut window_count)
        } else {
            WindowTree::default()
        }
    }

    fn add_subtree(
        &self,
        window_count: &mut usize,
        name: &str,
        tree: &mut WindowTree,
        size_ratio: f32,
    ) {
        let template = self.get(name);
        if let Some(direction) = template.kind() {
            tree.push(WindowTreeNode {
                kind: WindowTreeNodeKind::Subtree(template.build_tree(
                    self,
                    direction,
                    window_count,
                )),
                size: size_ratio,
            });
        }
    }
}

pub enum LayoutTemplate {
    Horizontal(Vec<LayoutTemplateNode>),
    Vertical(Vec<LayoutTemplateNode>),
    None,
}

pub struct LayoutTemplateNode {
    kind: LayoutTemplateNodeKind,
    size: Option<f32>,
}

pub enum LayoutTemplateNodeKind {
    Window(usize),
    Group(String),
}

#[derive(Default)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct WindowTree {
    nodes: Vec<WindowTreeNode>,
    count: usize,
    split: SplitDirection,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
struct WindowTreeNode {
    kind: WindowTreeNodeKind,
    size: f32,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
enum WindowTreeNodeKind {
    Window(usize),
    Subtree(WindowTree),
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
            nodes: Vec::new(),
            count: 0,
            split,
        }
    }

    fn push(&mut self, value: WindowTreeNode) {
        match value.kind {
            WindowTreeNodeKind::Window(n) => {
                self.count += n;
            }
            WindowTreeNodeKind::Subtree(..) => {
                self.count += 1;
            }
        }
        self.nodes.push(value);
    }
}

impl LayoutTemplate {
    fn iter(&self) -> Box<dyn Iterator<Item = &LayoutTemplateNode> + '_> {
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
        templates: &LayoutTemplates,
        direction: SplitDirection,
        window_count: &mut usize,
    ) -> WindowTree {
        let mut iter = self.iter();
        let mut tree = WindowTree::new(direction);
        while *window_count > 0
            && let Some(LayoutTemplateNode {
                kind,
                size: size_ratio,
            }) = iter.next()
        {
            let ratio = size_ratio.unwrap_or(DEFAULT_SIZE);
            match kind {
                LayoutTemplateNodeKind::Window(n) => {
                    let left = window_count.saturating_sub(*n);
                    let used = *window_count - left;
                    tree.push(WindowTreeNode {
                        kind: WindowTreeNodeKind::Window(used),
                        size: ratio,
                    });

                    *window_count = left;
                }
                LayoutTemplateNodeKind::Group(group) => {
                    templates.add_subtree(window_count, group, &mut tree, ratio);
                }
            }
        }

        tree
    }
}

impl WindowTree {
    fn calculate_rectangles<T>(self, container: Rectangle<i32, T>) -> Vec<Rectangle<i32, T>> {
        let mut rectangles = Vec::with_capacity(self.count);
        let mut x = container.loc.x;
        let mut y = container.loc.y;
        let split = self.split;

        let sizes: Vec<_> = match split {
            SplitDirection::Horizontal => distribute_evenly(container.size.h, self.count as i32)
                .into_iter()
                .map(|h| Size::new(container.size.w, h))
                .collect(),
            SplitDirection::Vertical => distribute_evenly(container.size.w, self.count as i32)
                .into_iter()
                .map(|w| Size::new(w, container.size.h))
                .collect(),
        };
        let mut sizes = sizes.into_iter();

        for node in self.nodes {
            match node.kind {
                WindowTreeNodeKind::Window(n) => {
                    for _ in 0..n {
                        let size = sizes.next().unwrap();
                        rectangles.push(Rectangle::new((x, y).into(), size));
                        match split {
                            SplitDirection::Horizontal => y += size.h,
                            SplitDirection::Vertical => x += size.w,
                        }
                    }
                }
                WindowTreeNodeKind::Subtree(tree) => {
                    let size = sizes.next().unwrap();
                    rectangles
                        .extend(tree.calculate_rectangles(Rectangle::new((x, y).into(), size)));
                    match split {
                        SplitDirection::Horizontal => y += size.h,
                        SplitDirection::Vertical => x += size.w,
                    }
                }
            }
        }
        rectangles
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
                map.insert($key.to_string(), LayoutTemplate::$split(items));
            })*
            LayoutTemplates(map)
        }};
    }

    macro_rules! layout_group_item {
        (window($count:expr $(,$size:expr)?)) => {
            LayoutTemplateNode {
                kind: LayoutTemplateNodeKind::Window($count),
                size: { let mut r = None; $( r = Some($size); )? r },
            }
        };
        (group($group:literal $(,$size:expr)?)) => {
            LayoutTemplateNode {
                kind: LayoutTemplateNodeKind::Group($group.to_string()),
                size: { let mut r = None; $( r = Some($size); )? r },
            }
        };
    }

    macro_rules! window_tree {
        () => {
            WindowTree::default()
        };
        ($split:ident; $($kind:ident ($($arg:expr),*)),*; $count:expr) => {
            WindowTree {
                nodes: vec![$(window_tree_node!($kind ($($arg),*))),*],
                count: $count,
                split: SplitDirection::$split,
            }
        }
    }

    macro_rules! window_tree_node {
        (node($count:expr $(,$size:expr)?)) => {
            WindowTreeNode {
                kind: WindowTreeNodeKind::Window($count),
                size: { let mut r = 1.0; $( r = $size; )? r },
            }
        };
        (branch($tree:expr $(,$size:expr)?)) => {
            WindowTreeNode {
                kind: WindowTreeNodeKind::Subtree($tree),
                size: { let mut r = 1.0; $( r = $size; )? r },
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
    fn test_build_tree(templates: LayoutTemplates, windows: usize) -> WindowTree {
        templates.build_tree_from_template(windows)
    }

    macro_rules! rectangle {
        ($x:expr, $y:expr, $w:expr, $h:expr) => {
            Rectangle::<_, Logical>::new(($x, $y).into(), ($w, $h).into())
        };
    }

    #[test_case(
        window_tree!(Vertical; ; 0) => Vec::<Rectangle<i32, Logical>>::new();
        "empty_tree"
    )]
    #[test_case(
        window_tree!(Vertical; node(1); 1) => vec![rectangle!(0, 0, 100, 100)];
        "single_window_vertical"
    )]
    #[test_case(
        window_tree!(Vertical; node(1), branch(window_tree!(Horizontal; node(2); 2)); 2) =>
        vec![
            rectangle!(0, 0, 50, 100),
            rectangle!(50, 0, 50, 50),
            rectangle!(50, 50, 50, 50)
        ];
        "vertical_with_horizontal_branch"
    )]
    fn test_calculate_window_rectangles<T>(window_tree: WindowTree) -> Vec<Rectangle<i32, T>> {
        window_tree.calculate_rectangles(Rectangle::new((0, 0).into(), (100, 100).into()))
    }
}
