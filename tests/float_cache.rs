//! Boxes laid out beside floats skip the layout cache (where the floats are is
//! not part of its key). Marking a descendant dirty must still reach the
//! ancestors above such a box, and an entry stored before the floats came must
//! not be served once they have gone.
#![cfg(all(feature = "block_layout", feature = "float_layout"))]

use std::cell::Cell;

use taffy::prelude::*;
use taffy::{compute_leaf_layout, Float, Overflow, Point};

fn float(tree: &mut TaffyTree<()>, width: f32) -> NodeId {
    tree.new_leaf(Style {
        display: Display::Block,
        float: Float::Left,
        size: Size { width: length(width), height: length(50.0) },
        ..Default::default()
    })
    .unwrap()
}

/// A paragraph beside a float grows from one line to three: the block after
/// it moves down, though the paragraph's own cache never held an entry.
#[test]
fn a_leaf_beside_a_float_that_grows_moves_what_follows() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let f = float(&mut tree, 100.0);
    let p = tree.new_leaf(Style { display: Display::Block, ..Default::default() }).unwrap();
    let q = tree.new_leaf(Style { display: Display::Block, ..Default::default() }).unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Block,
                size: Size { width: length(400.0), height: auto() },
                ..Default::default()
            },
            &[f, p, q],
        )
        .unwrap();
    let lines = Cell::new(1.0f32);
    let layout = |tree: &mut TaffyTree<()>| {
        tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |inputs, id, _, style| {
            let h = if id == p { 20.0 * lines.get() } else { 0.0 };
            compute_leaf_layout(inputs, style, |_, _| 0.0, |_, _| Size { width: 0.0, height: h })
        })
        .unwrap();
    };
    layout(&mut tree);
    assert_eq!(tree.layout(q).unwrap().location.y, 20.0);
    lines.set(3.0);
    tree.mark_dirty(p).unwrap();
    layout(&mut tree);
    assert_eq!(tree.layout(p).unwrap().size.height, 60.0);
    assert_eq!(tree.layout(q).unwrap().location.y, 60.0);
}

/// A block holding a box that avoids floats: laid out with no float, then
/// beside one (its child moves to x 40), then with the float gone again. Its
/// child goes back to x 0: the block's entry from before the float came is not
/// served, since its child's position was laid out since.
#[test]
fn an_entry_from_before_a_float_came_is_not_served_after_it_goes() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let avoider = tree
        .new_leaf(Style {
            display: Display::Block,
            overflow: Point { x: Overflow::Hidden, y: Overflow::Hidden },
            size: Size { width: length(50.0), height: length(20.0) },
            ..Default::default()
        })
        .unwrap();
    let block = tree.new_with_children(Style { display: Display::Block, ..Default::default() }, &[avoider]).unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Block,
                size: Size { width: length(400.0), height: auto() },
                ..Default::default()
            },
            &[block],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.layout(avoider).unwrap().location.x, 0.0);
    let f = float(&mut tree, 40.0);
    tree.insert_child_at_index(root, 0, f).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.layout(avoider).unwrap().location.x, 40.0);
    tree.remove_child(root, f).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.layout(avoider).unwrap().location.x, 0.0);
}
