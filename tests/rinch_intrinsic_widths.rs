//! Two intrinsic-width rules of block layout rinch's floats need (rinch
//! #2322, #2326), each checked against Chrome 156:
//!
//! - A block sized from its content (a float) resolves its children's
//!   percentages against the width it took.
//! - The floats right before a leaf that places them are measured with it
//!   (`BlockContext::is_intrinsic`): an inline formatting context's floats
//!   share its lines.
#![cfg(all(feature = "block_layout", feature = "float_layout"))]

use std::cell::RefCell;

use taffy::prelude::*;
use taffy::{compute_leaf_layout, Float};

fn block(width: Dimension) -> Style {
    Style { display: Display::Block, size: Size { width, height: auto() }, ..Default::default() }
}

fn float_left() -> Style {
    Style { display: Display::Block, float: Float::Left, ..Default::default() }
}

fn sized(width: f32, height: f32) -> Style {
    Style { display: Display::Block, size: Size { width: length(width), height: length(height) }, ..Default::default() }
}

#[test]
fn a_float_resolves_its_childrens_percentages_against_its_own_width() {
    // Chrome: an auto-width float holding a `width: 70%` box and a 333px box
    // is 333 wide, and the first box 70% of that.
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let pct = tree
        .new_leaf(Style {
            display: Display::Block,
            size: Size { width: percent(0.7), height: length(10.0) },
            ..Default::default()
        })
        .unwrap();
    let wide = tree.new_leaf(sized(333.0, 10.0)).unwrap();
    let float = tree.new_with_children(float_left(), &[pct, wide]).unwrap();
    let root = tree.new_with_children(block(length(560.0)), &[float]).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.layout(float).unwrap().size.width, 333.0);
    // 233.1, rounded.
    assert_eq!(tree.layout(pct).unwrap().size.width, 233.0);
}

/// What a leaf that places floats was asked, in intrinsic-width questions:
/// the available width and the widths of the floats it was handed.
type Asked = Vec<(AvailableSpace, Vec<f32>)>;

/// A 560px block holding an auto-width float, which holds a 100x20 left
/// float and then a leaf of 83px max-content (40 min-content) that places
/// the floats before it. Measured beside its floats, the leaf answers its
/// line with them, as an inline formatting context does. Returns the outer
/// float's width and what the leaf was asked.
fn float_with_floats_and_leaf() -> (f32, Asked) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let inner = tree
        .new_leaf(Style {
            display: Display::Block,
            float: Float::Left,
            size: Size { width: length(100.0), height: length(20.0) },
            ..Default::default()
        })
        .unwrap();
    let leaf = tree.new_leaf(Style { display: Display::Block, ..Default::default() }).unwrap();
    let outer = tree.new_with_children(float_left(), &[inner, leaf]).unwrap();
    let root = tree.new_with_children(block(length(560.0)), &[outer]).unwrap();
    let asked = RefCell::new(Vec::new());
    tree.compute_layout_with_block_measure(
        root,
        Size::MAX_CONTENT,
        |inputs, node, _, style, ctx| {
            let beside: f32 = match &ctx {
                Some(ctx) if ctx.is_intrinsic() => {
                    let widths: Vec<f32> = ctx.pending_floats().iter().map(|f| f.size.width).collect();
                    asked.borrow_mut().push((inputs.available_space.width, widths.clone()));
                    widths.iter().sum()
                }
                _ => 0.0,
            };
            let _ = node;
            compute_leaf_layout(
                inputs,
                style,
                |_, _| 0.0,
                |known, avail| {
                    let max = 83.0 + beside;
                    let width = known.width.unwrap_or(match avail.width {
                        AvailableSpace::Definite(w) => w.min(max).max(40.0),
                        AvailableSpace::MinContent => 40.0,
                        AvailableSpace::MaxContent => max,
                    });
                    Size { width, height: 20.0 }
                },
            )
        },
        move |n, _| n == leaf,
    )
    .unwrap();
    (tree.layout(outer).unwrap().size.width, asked.into_inner())
}

#[test]
fn the_floats_before_a_leaf_are_measured_with_it() {
    let (width, asked) = float_with_floats_and_leaf();
    // Chrome: a float holding `<float 100px> text 83px` is 183 wide.
    assert_eq!(width, 183.0);
    assert!(!asked.is_empty(), "the leaf was never asked beside its floats");
    for (avail, widths) in asked {
        assert_ne!(avail, AvailableSpace::MinContent, "a min-content question is the floats' own");
        assert_eq!(widths, vec![100.0]);
    }
}
