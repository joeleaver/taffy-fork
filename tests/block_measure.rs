//! `TaffyTree::compute_layout_with_block_measure`: a leaf is handed the block
//! context it is laid out in, and sees the floats placed before it.
#![cfg(all(feature = "block_layout", feature = "float_layout"))]

use std::cell::RefCell;

use taffy::prelude::*;
use taffy::{compute_leaf_layout, Clear, Float};

/// A slot's `x` and `width`.
type Slot = (f32, f32);

/// A 300px block holding a 100x50 left float and then a leaf; returns the room
/// the leaf found beside the float at its top and at y = 60, once for every
/// time it was measured.
fn slots(tree: &mut TaffyTree<()>, root: NodeId) -> (Vec<Slot>, Vec<Slot>) {
    let top = RefCell::new(Vec::new());
    let below = RefCell::new(Vec::new());
    tree.compute_layout_with_block_measure(root, Size::MAX_CONTENT, |inputs, _, _, style, ctx| {
        if let Some(ctx) = ctx {
            let at = ctx.find_content_slot(0.0, Clear::None, None);
            top.borrow_mut().push((at.x, at.width));
            let at = ctx.find_content_slot(60.0, Clear::None, None);
            below.borrow_mut().push((at.x, at.width));
        }
        compute_leaf_layout(inputs, style, |_, _| 0.0, |_, _| Size { width: 0.0, height: 20.0 })
    })
    .unwrap();
    (top.into_inner(), below.into_inner())
}

fn tree() -> (TaffyTree<()>, NodeId, NodeId) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let float = tree
        .new_leaf(Style {
            display: Display::Block,
            float: Float::Left,
            size: Size { width: length(100.0), height: length(50.0) },
            ..Default::default()
        })
        .unwrap();
    let leaf = tree.new_leaf(Style { display: Display::Block, ..Default::default() }).unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Block,
                size: Size { width: length(300.0), height: auto() },
                ..Default::default()
            },
            &[float, leaf],
        )
        .unwrap();
    (tree, root, leaf)
}

#[test]
fn a_leaf_beside_a_float_is_handed_its_room() {
    let (mut tree, root, _) = tree();
    let (top, below) = slots(&mut tree, root);
    assert!(!top.is_empty());
    for (x, width) in top {
        assert_eq!((x, width), (100.0, 200.0));
    }
    for (x, width) in below {
        assert_eq!((x, width), (0.0, 300.0));
    }
}

#[test]
fn a_leaf_beside_a_float_is_measured_again_when_the_float_moves() {
    let (mut tree, root, _) = tree();
    slots(&mut tree, root);
    // The float grows wider; only the float and its ancestors are marked
    // dirty, not the leaf.
    let float = tree.child_at_index(root, 0).unwrap();
    let mut style = tree.style(float).unwrap().clone();
    style.size.width = length(150.0);
    tree.set_style(float, style).unwrap();
    let (top, _) = slots(&mut tree, root);
    assert!(!top.is_empty(), "the leaf was answered from its cache");
    for (x, width) in top {
        assert_eq!((x, width), (150.0, 150.0));
    }
}

#[test]
fn a_leaf_with_no_float_beside_it_is_not_handed_a_context_with_floats() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let leaf = tree.new_leaf(Style { display: Display::Block, ..Default::default() }).unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Block,
                size: Size { width: length(300.0), height: auto() },
                ..Default::default()
            },
            &[leaf],
        )
        .unwrap();
    let (top, _) = slots(&mut tree, root);
    for (x, width) in top {
        assert_eq!((x, width), (0.0, 300.0));
    }
}

#[test]
fn a_content_slot_ends_where_the_room_beside_the_floats_changes() {
    let (mut tree, root, _) = tree();
    let heights = RefCell::new(Vec::new());
    tree.compute_layout_with_block_measure(root, Size::MAX_CONTENT, |inputs, _, _, style, ctx| {
        if let Some(ctx) = ctx {
            heights.borrow_mut().push(ctx.find_content_slot(10.0, Clear::None, None).height);
        }
        compute_leaf_layout(inputs, style, |_, _| 0.0, |_, _| Size { width: 0.0, height: 20.0 })
    })
    .unwrap();
    let heights = heights.into_inner();
    assert!(!heights.is_empty());
    // From y 10 to the float's bottom at 50.
    assert!(heights.iter().all(|&h| h == 40.0), "{heights:?}");
}
