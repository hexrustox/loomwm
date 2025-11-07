use std::cell::Cell;

use smithay::utils::Rectangle;

pub type BBox = Rectangle<i32, i32>;

pub enum LayoutTree {
    Horizontal(Vec<LayoutNode>),
    Vertical(Vec<LayoutNode>),
}

impl LayoutTree {
    fn unwrap(&self) -> &[LayoutNode] {
        match self {
            Self::Horizontal(v) => v,
            Self::Vertical(v) => v,
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::Horizontal(v) => v.is_empty(),
            Self::Vertical(v) => v.is_empty(),
        }
    }
}

pub enum LayoutNode {
    Window,
    SubTree(LayoutTree),
}

pub fn get_layout(output: BBox, window_remain: usize, layout_tree: &LayoutTree) -> Vec<BBox> {
    if window_remain == 0 || layout_tree.is_empty() {
        return vec![];
    }

    let mut vec = Vec::new();

    let tree = layout_tree.unwrap();
    let (window_count, remain) = consume_window(window_remain, tree);

    let x = Cell::new(output.loc.x);
    let y = Cell::new(output.loc.y);
    let w;
    let h;

    let mut increment: Box<dyn FnMut()> = match layout_tree {
        LayoutTree::Horizontal(..) => {
            w = output.size.w / window_count as i32;
            h = output.size.h;
            Box::new(|| {
                x.set(x.get() + w);
            })
        }
        LayoutTree::Vertical(..) => {
            w = output.size.w;
            h = output.size.h / window_count as i32;
            Box::new(|| {
                y.set(y.get() + h);
            })
        }
    };

    let mut iter = tree.iter();
    let mut i = 0;
    while let Some(n) = iter.next()
        && i < window_count
    {
        let rect = Rectangle::new((x.get(), y.get()).into(), (w, h).into());
        if let LayoutNode::SubTree(tree) = n {
            let count = window_remain.saturating_sub(remain + 1);
            if count == 0 {
                break;
            }
            vec.extend(get_layout(rect, count, tree));
        } else {
            vec.push(rect);
        }
        increment();
        i += 1;
    }

    vec
}

fn consume_window(mut window_remain: usize, tree: &[LayoutNode]) -> (usize, usize) {
    let mut count = 0;
    for n in tree {
        if window_remain == 0 {
            break;
        }
        if matches!(n, LayoutNode::Window) {
            window_remain -= 1;
            count += 1;
        } else if let LayoutNode::SubTree(t) = n {
            let (c, r) = consume_window(window_remain, t.unwrap());
            window_remain = r;
            if c > 0 {
                count += 1;
            }
        }
    }

    (count, window_remain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    fn h(nodes: Vec<LayoutNode>) -> LayoutTree {
        LayoutTree::Horizontal(nodes)
    }

    fn v(nodes: Vec<LayoutNode>) -> LayoutTree {
        LayoutTree::Vertical(nodes)
    }

    macro_rules! empty_layout {
        () => {
            Vec::<BBox>::new()
        };
    }

    #[test_case(1, &[] => (0, 1); "empty_tree")]
    #[test_case(3, &[LayoutNode::Window] => (1, 2); "single_window")]
    #[test_case(3, &[LayoutNode::Window, LayoutNode::Window, LayoutNode::Window, LayoutNode::Window] => (3, 0); "more_windows_than_remain")]
    #[test_case(3, &[LayoutNode::Window, LayoutNode::SubTree(h(vec![LayoutNode::Window, LayoutNode::Window]))] => (2, 0); "nested_tree_consumes_all")]
    #[test_case(3, &[LayoutNode::Window, LayoutNode::SubTree(h(vec![LayoutNode::Window]))] => (2, 1); "nested_tree_consumes_partial")]
    #[test_case(3, &[LayoutNode::Window, LayoutNode::SubTree(h(vec![LayoutNode::Window])), LayoutNode::Window] => (3, 0); "nested_and_sibling")]
    #[test_case(0, &[LayoutNode::Window, LayoutNode::Window] => (0, 0); "zero_remain")]
    fn test_get_remaining_window(remain: usize, tree: &[LayoutNode]) -> (usize, usize) {
        consume_window(remain, tree)
    }

    #[test_case(
        Rectangle::new((0, 0).into(), (100, 100).into()),
        3,
        h(vec![LayoutNode::Window]) => vec![Rectangle::new((0, 0).into(), (100, 100).into())];
        "single_window_horizontal"
    )]
    #[test_case(
        Rectangle::new((0, 0).into(), (100, 100).into()),
        3,
        v(vec![LayoutNode::Window]) => vec![Rectangle::new((0, 0).into(), (100, 100).into())];
        "single_window_vertical"
    )]
    #[test_case(
        Rectangle::new((0, 0).into(), (100, 100).into()),
        3,
        h(vec![LayoutNode::Window, LayoutNode::Window]) => vec![
            Rectangle::new((0, 0).into(), (50, 100).into()),
            Rectangle::new((50, 0).into(), (50, 100).into())
        ];
        "two_windows_horizontal"
    )]
    #[test_case(
        Rectangle::new((0, 0).into(), (100, 100).into()),
        3,
        v(vec![LayoutNode::Window, LayoutNode::Window]) => vec![
            Rectangle::new((0, 0).into(), (100, 50).into()),
            Rectangle::new((0, 50).into(), (100, 50).into())
        ];
        "two_windows_vertical"
    )]
    #[test_case(
        Rectangle::new((0, 0).into(), (100, 100).into()),
        3,
        h(vec![
            LayoutNode::Window,
            LayoutNode::SubTree(v(vec![LayoutNode::Window, LayoutNode::Window]))
        ]) => vec![
            Rectangle::new((0, 0).into(), (50, 100).into()),
            Rectangle::new((50, 0).into(), (50, 50).into()),
            Rectangle::new((50, 50).into(), (50, 50).into())
        ];
        "horizontal_with_vertical_subtree"
    )]
    #[test_case(
        Rectangle::new((0, 0).into(), (100, 100).into()),
        3,
        h(vec![
            LayoutNode::Window,
            LayoutNode::SubTree(v(vec![
                LayoutNode::Window,
                LayoutNode::SubTree(h(vec![LayoutNode::Window, LayoutNode::Window])),
                LayoutNode::Window
            ]))
        ]) => vec![
            Rectangle::new((0, 0).into(), (50, 100).into()),
            Rectangle::new((50, 0).into(), (50, 50).into()),
            Rectangle::new((50, 50).into(), (50, 50).into())
        ];
        "deeply_nested_layout"
    )]
    #[test_case(
        Rectangle::new((0, 0).into(), (100, 100).into()),
        0,
        h(vec![LayoutNode::Window, LayoutNode::Window]) => empty_layout!();
        "zero_windows_remain"
    )]
    #[test_case(
        Rectangle::new((0, 0).into(), (100, 100).into()),
        2,
        h(vec![]) => empty_layout![];
        "empty_layout_tree"
    )]
    #[test_case(
        Rectangle::new((0, 0).into(), (100, 100).into()),
        1,
        h(vec![LayoutNode::Window, LayoutNode::Window]) => vec![Rectangle::new((0, 0).into(), (100, 100).into())];
        "more_nodes_than_windows"
    )]
    #[test_case(
        Rectangle::new((0, 0).into(), (100, 100).into()),
        2,
        h(vec![LayoutNode::Window, LayoutNode::SubTree(v(vec![]))]) => vec![Rectangle::new((0, 0).into(), (100, 100).into())];
        "subtree_with_no_windows"
    )]
    #[test_case(
        Rectangle::new((0, 0).into(), (0, 100).into()),
        2,
        h(vec![LayoutNode::Window, LayoutNode::Window]) => vec![Rectangle::new((0, 0).into(), (0, 100).into()), Rectangle::new((0, 0).into(), (0, 100).into())];
        "zero_width_layout"
    )]
    #[test_case(
        Rectangle::new((0, 0).into(), (100, 0).into()),
        2,
        v(vec![LayoutNode::Window, LayoutNode::Window]) => vec![Rectangle::new((0, 0).into(), (100, 0).into()), Rectangle::new((0, 0).into(), (100, 0).into())];
        "zero_height_layout"
    )]
    #[test_case(
        Rectangle::new((0, 0).into(), (120, 120).into()),
        4,
        h(vec![
            LayoutNode::Window,
            LayoutNode::SubTree(v(vec![
                LayoutNode::Window,
                LayoutNode::SubTree(h(vec![
                    LayoutNode::Window,
                    LayoutNode::Window
                ]))
            ]))
        ]) => vec![
            Rectangle::new((0, 0).into(), (60, 120).into()),
            Rectangle::new((60, 0).into(), (60, 60).into()),
            Rectangle::new((60, 60).into(), (30, 60).into()),
            Rectangle::new((90, 60).into(), (30, 60).into())
        ];
        "three_level_nested_h_in_v_in_h"
    )]
    #[test_case(
        Rectangle::new((0, 0).into(), (100, 100).into()),
        4,
        h(vec![
            LayoutNode::SubTree(v(vec![LayoutNode::Window, LayoutNode::Window])),
            LayoutNode::SubTree(v(vec![LayoutNode::Window, LayoutNode::Window]))
        ]) => vec![
            Rectangle::new((0, 0).into(), (50, 50).into()),
            Rectangle::new((0, 50).into(), (50, 50).into()),
            Rectangle::new((50, 0).into(), (50, 50).into()),
            Rectangle::new((50, 50).into(), (50, 50).into())
        ];
        "two_separate_nested_subtrees"
    )]
    #[test_case(
        Rectangle::new((0, 0).into(), (100, 100).into()),
        3,
        h(vec![
            LayoutNode::Window,
            LayoutNode::SubTree(v(vec![
                LayoutNode::Window,
                LayoutNode::Window,
                LayoutNode::Window
            ])),
            LayoutNode::Window
        ]) => vec![
            Rectangle::new((0, 0).into(), (50, 100).into()),
            Rectangle::new((50, 0).into(), (50, 50).into()),
            Rectangle::new((50, 50).into(), (50, 50).into())
        ];
        "window_count_exhausted_in_deep_subtree"
    )]
    #[test_case(
        Rectangle::new((0, 0).into(), (200, 200).into()),
        10,
        v(vec![
            LayoutNode::Window,
            LayoutNode::SubTree(h(vec![
                LayoutNode::Window,
                LayoutNode::SubTree(v(vec![
                    LayoutNode::Window,
                    LayoutNode::Window
                ]))
            ])),
            LayoutNode::Window
        ]) => vec![
            Rectangle::new((0, 0).into(), (200, 66).into()),
            Rectangle::new((0, 66).into(), (100, 66).into()),
            Rectangle::new((100, 66).into(), (100, 33).into()),
            Rectangle::new((100, 99).into(), (100, 33).into()),
            Rectangle::new((0, 132).into(), (200, 66).into())
        ];
        "complex_layout_with_surplus_windows"
    )]
    fn test_get_layout(
        layout_size: BBox,
        window_remain: usize,
        layout_tree: LayoutTree,
    ) -> Vec<BBox> {
        get_layout(layout_size, window_remain, &layout_tree)
    }
}
