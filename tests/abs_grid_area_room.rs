//! An absolutely positioned child of a grid container is sized in its grid
//! area (its containing block, css-grid-1 §9.1) less its margins **and its
//! insets** (CSS 2.1 §10.3.7; rinch #2377). An `auto` width shrinks to fit
//! there, and `fit-content`/`stretch` resolve against it. The insets used to
//! be taken out for a table only, so a box with `left: 5px` wrapped its text
//! 5px wider than its room and overflowed the area by that much.
//!
//! The leaf is text-like: 520 wide at max-content, 100 at min-content, and
//! otherwise as wide as its room, in 20px lines. The grid has columns of 100
//! and 200 (no gap, no padding), 300 wide; the box sits in `grid-column: 2`,
//! with margins 23 and 17, so its area is 200 and its room 160 before insets.
//! `rinch-dom`'s `abs_grid_area_2377_tests.rs` has the same shapes with real
//! text and Chrome 153's numbers.
#![cfg(feature = "grid")]

use taffy::compute_leaf_layout;
use taffy::prelude::*;

/// Text-like: 520 at max-content, 100 at min-content, else its room.
fn text(known: Size<Option<f32>>, avail: Size<AvailableSpace>) -> Size<f32> {
    let width = known.width.unwrap_or(match avail.width {
        AvailableSpace::MaxContent => 520.0,
        AvailableSpace::MinContent => 100.0,
        AvailableSpace::Definite(w) => w.clamp(100.0, 520.0),
    });
    Size { width, height: known.height.unwrap_or(20.0 * (520.0 / width).ceil()) }
}

/// The box's (x, width, height) for an absolute child of `grid-column: 2`
/// with `inset` and `width`.
fn lay_out(inset: Rect<LengthPercentageAuto>, width: Dimension) -> (f32, f32, f32) {
    let mut tree: TaffyTree<bool> = TaffyTree::new();
    let leaf = tree.new_leaf_with_context(Style::default(), true).unwrap();
    let item = tree
        .new_leaf(Style {
            position: Position::Absolute,
            grid_column: Line { start: line(2), end: line(3) },
            inset,
            size: Size { width, height: auto() },
            margin: Rect { left: length(23.0), right: length(17.0), top: zero(), bottom: zero() },
            ..Default::default()
        })
        .unwrap();
    // The text is the item's content: a block item holding the leaf.
    tree.set_style(item, Style { display: Display::Block, ..tree.style(item).unwrap().clone() }).unwrap();
    tree.add_child(item, leaf).unwrap();
    let grid = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                grid_template_columns: vec![length(100.0), length(200.0)],
                size: Size { width: length(300.0), height: length(300.0) },
                ..Default::default()
            },
            &[item],
        )
        .unwrap();
    tree.compute_layout_with_measure(grid, Size::MAX_CONTENT, |inputs, _, ctx, style| match ctx {
        Some(true) => compute_leaf_layout(inputs, style, |_, _| 0.0, text),
        _ => compute_leaf_layout(inputs, style, |_, _| 0.0, |_, _| Size::ZERO),
    })
    .unwrap();
    let l = tree.layout(item).unwrap();
    (l.location.x, l.size.width, l.size.height)
}

fn insets(left: Option<f32>, right: Option<f32>) -> Rect<LengthPercentageAuto> {
    let side = |v: Option<f32>| v.map(length).unwrap_or(auto());
    Rect { left: side(left), right: side(right), top: auto(), bottom: auto() }
}

#[test]
fn an_auto_width_shrinks_to_fit_in_the_area_less_its_margins() {
    // 200 - 23 - 17 = 160, from the area's start + 23.
    assert_eq!(lay_out(insets(None, None), auto()), (123.0, 160.0, 80.0));
}

#[test]
fn a_left_inset_comes_out_of_the_room() {
    // 160 - 5 = 155 (it was 160, overflowing the area by 5).
    assert_eq!(lay_out(insets(Some(5.0), None), auto()), (128.0, 155.0, 80.0));
}

#[test]
fn a_right_inset_comes_out_of_the_room() {
    // 160 - 9 = 151, its right margin edge 9 short of the area's end.
    assert_eq!(lay_out(insets(None, Some(9.0)), auto()), (123.0, 151.0, 80.0));
}

#[test]
fn two_insets_give_the_width_between_them() {
    assert_eq!(lay_out(insets(Some(5.0), Some(9.0)), auto()), (128.0, 146.0, 80.0));
}

#[test]
fn fit_content_and_stretch_resolve_against_the_room_less_insets() {
    assert_eq!(lay_out(insets(Some(5.0), None), Dimension::fit_content()), (128.0, 155.0, 80.0));
    assert_eq!(lay_out(insets(Some(5.0), None), Dimension::stretch()), (128.0, 155.0, 80.0));
}
