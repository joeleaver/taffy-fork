//! `justify-self` aligns an absolutely positioned box's static position in a
//! block container (rinch #2541, css-position-3 §4.1): the content box is the
//! static-position rectangle, turned into the inset-modified containing block
//! the box is aligned in.
//!
//! A block 400px wide with `padding-left: 11px` (so the content box is 11..400)
//! holding one absolute box, insets `auto`. Chrome 157's x for each, relative to
//! the block's border box.
#![cfg(feature = "block_layout")]

use taffy::prelude::*;
use taffy::style::Direction;

fn x_of(container: Style, justify: Option<AlignItems>, width: f32) -> f32 {
    let mut taffy: TaffyTree<()> = TaffyTree::new();
    taffy.disable_rounding();
    let child = taffy
        .new_leaf(Style {
            position: Position::Absolute,
            size: Size { width: length(width), height: length(20.0) },
            justify_self: justify,
            ..Default::default()
        })
        .unwrap();
    let root = taffy
        .new_with_children(
            Style {
                display: Display::Block,
                size: Size { width: length(400.0), height: length(80.0) },
                padding: Rect { left: length(11.0), right: zero(), top: length(7.0), bottom: zero() },
                ..container
            },
            &[child],
        )
        .unwrap();
    taffy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    taffy.layout(child).unwrap().location.x
}

fn ltr() -> Style {
    Style::default()
}

fn rtl() -> Style {
    Style { direction: Direction::Rtl, ..Default::default() }
}

#[test]
fn an_absolute_box_is_aligned_in_the_content_box() {
    assert_eq!(x_of(ltr(), None, 40.0), 11.0);
    assert_eq!(x_of(ltr(), Some(AlignItems::END), 40.0), 360.0);
    assert_eq!(x_of(ltr(), Some(AlignItems::CENTER), 40.0), 185.5);
    assert_eq!(x_of(rtl(), None, 40.0), 360.0);
    assert_eq!(x_of(rtl(), Some(AlignItems::END), 40.0), 11.0);
    // The container's `justify-items` when the box has no `justify-self`.
    assert_eq!(x_of(Style { justify_items: Some(AlignItems::END), ..ltr() }, None, 40.0), 360.0);
    // `stretch` is `normal`.
    assert_eq!(x_of(ltr(), Some(AlignItems::STRETCH), 40.0), 11.0);
}

#[test]
fn an_overflowing_box_spills_from_the_inset_modified_containing_block() {
    // End-aligned: from the block's start (0) to the content end (400).
    assert_eq!(x_of(ltr(), Some(AlignItems::END), 450.0), -50.0);
    // Centred on the content box's centre (205.5), as wide as the block allows: 11..400.
    assert_eq!(x_of(ltr(), Some(AlignItems::CENTER), 450.0), -19.5);
    // `safe` starts it at that block's start, 0 for `end`, 11 for `center`.
    assert_eq!(x_of(ltr(), Some(AlignItems::SAFE_END), 450.0), 0.0);
    assert_eq!(x_of(ltr(), Some(AlignItems::SAFE_CENTER), 450.0), 11.0);
}
