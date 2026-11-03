//! A box that establishes a block formatting context contains its floats: its
//! content box grows to their bottom, and its bottom padding and border go
//! below that (CSS 2.2 §10.6.7).
#![cfg(all(feature = "block_layout", feature = "float_layout"))]

use taffy::prelude::*;
use taffy::{Float, Point};

#[test]
fn a_padded_flow_root_holds_its_floats_above_its_bottom_padding() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let float = tree
        .new_leaf(Style {
            display: Display::Block,
            float: Float::Left,
            size: Size { width: length(40.0), height: length(22.0) },
            ..Default::default()
        })
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::FlowRoot,
                size: Size { width: length(300.0), height: auto() },
                padding: Rect { left: length(8.0), right: length(8.0), top: length(8.0), bottom: length(8.0) },
                border: Rect { left: zero(), right: zero(), top: length(1.0), bottom: length(2.0) },
                ..Default::default()
            },
            &[float],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    // 1 + 8 + 22 + 8 + 2: Chrome 156 gives 41 for the same box.
    assert_eq!(tree.layout(root).unwrap().size.height, 41.0);
    assert_eq!(tree.layout(float).unwrap().location, Point { x: 8.0, y: 9.0 });
}
