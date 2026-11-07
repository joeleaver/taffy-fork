//! A box sized to fit its content takes its horizontal margins out of its
//! room once (rinch #2361). The room a parent hands a child is for the
//! child's border box: the parent takes the child's margins out of it, as
//! the block algorithm does for a float, a `fit-content` width or an
//! absolutely positioned box, and grid for its items. A leaf took them out
//! a second time, and so did a flex container; a column flex handed its
//! items its whole width, which only a leaf put right.
//!
//! The leaf is text-like: 520 wide at max-content, 100 at min-content, and
//! otherwise as wide as its room, in 20px lines. Every box here has margins
//! 23 and 17 in a 300px container: Chrome 156 makes each 260 wide (the
//! same shapes with real text, `rinch-dom`'s `float_room_round3_tests.rs`).
#![cfg(all(feature = "block_layout", feature = "flexbox", feature = "grid", feature = "float_layout"))]

use taffy::prelude::*;
use taffy::{compute_leaf_layout, Float};

/// Text-like: 520 at max-content, 100 at min-content, else its room.
fn text(known: Size<Option<f32>>, avail: Size<AvailableSpace>) -> Size<f32> {
    let width = known.width.unwrap_or(match avail.width {
        AvailableSpace::MaxContent => 520.0,
        AvailableSpace::MinContent => 100.0,
        AvailableSpace::Definite(w) => w.clamp(100.0, 520.0),
    });
    Size { width, height: known.height.unwrap_or(20.0 * (520.0 / width).ceil()) }
}

fn margins() -> Rect<LengthPercentageAuto> {
    Rect { left: length(23.0), right: length(17.0), top: zero(), bottom: zero() }
}

/// The border-box width of `item`, a child of `parent` (`parent` holds it,
/// in a 300px block), laid out with the text measure on every leaf marked.
fn width_of(parent: Style, item: Style, wrap_item_in_block: bool) -> f32 {
    let mut tree: TaffyTree<bool> = TaffyTree::new();
    let leaf_style =
        if wrap_item_in_block { Style { display: Display::Block, ..Default::default() } } else { item.clone() };
    let leaf = tree.new_leaf_with_context(leaf_style, true).unwrap();
    let item_node = if wrap_item_in_block { tree.new_with_children(item, &[leaf]).unwrap() } else { leaf };
    let parent = tree
        .new_with_children(Style { size: Size { width: length(300.0), height: auto() }, ..parent }, &[item_node])
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Block,
                size: Size { width: length(560.0), height: auto() },
                ..Default::default()
            },
            &[parent],
        )
        .unwrap();
    tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |inputs, _, ctx, style| match ctx {
        Some(true) => compute_leaf_layout(inputs, style, |_, _| 0.0, text),
        _ => compute_leaf_layout(inputs, style, |_, _| 0.0, |_, _| Size::ZERO),
    })
    .unwrap();
    tree.layout(item_node).unwrap().size.width
}

fn block() -> Style {
    Style { display: Display::Block, ..Default::default() }
}

fn item(style: Style) -> Style {
    Style { margin: margins(), ..style }
}

#[test]
fn a_float_leaf() {
    let float = item(Style { display: Display::Block, float: Float::Left, ..Default::default() });
    assert_eq!(width_of(block(), float, false), 260.0);
}

#[test]
fn a_fit_content_leaf() {
    let fit = item(Style {
        display: Display::Block,
        size: Size { width: Dimension::fit_content(), height: auto() },
        ..Default::default()
    });
    assert_eq!(width_of(block(), fit, false), 260.0);
}

#[test]
fn an_absolutely_positioned_leaf() {
    let parent = Style { position: Position::Relative, ..block() };
    let abs = item(Style { display: Display::Block, position: Position::Absolute, ..Default::default() });
    assert_eq!(width_of(parent, abs, false), 260.0);
}

#[test]
fn a_start_aligned_grid_item_leaf() {
    let grid = Style { display: Display::Grid, justify_items: Some(AlignItems::START), ..Default::default() };
    assert_eq!(width_of(grid, item(block()), false), 260.0);
}

fn column() -> Style {
    Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::FLEX_START),
        ..Default::default()
    }
}

/// Right before: the leaf took its margins out of the column's whole width.
#[test]
fn a_column_flex_item_leaf() {
    assert_eq!(width_of(column(), item(block()), false), 260.0);
}

/// A block container in a column was handed the column's whole width and
/// took nothing out of it: 300.
#[test]
fn a_column_flex_item_block() {
    assert_eq!(width_of(column(), item(block()), true), 260.0);
}

#[test]
fn a_column_flex_item_leaf_of_fit_content_width() {
    let fit = item(Style {
        display: Display::Block,
        size: Size { width: Dimension::fit_content(), height: auto() },
        ..Default::default()
    });
    assert_eq!(width_of(column(), fit, false), 260.0);
}

/// A row's item is shrunk to the room its margins leave: right before too.
#[test]
fn a_row_flex_item_leaf() {
    let row = Style { display: Display::Flex, align_items: Some(AlignItems::FLEX_START), ..Default::default() };
    assert_eq!(width_of(row, item(block()), false), 260.0);
}

/// An absolutely positioned block container was handed the whole width and
/// took nothing out of it: 300.
#[test]
fn an_absolutely_positioned_block() {
    let parent = Style { position: Position::Relative, ..block() };
    let abs = item(Style { display: Display::Block, position: Position::Absolute, ..Default::default() });
    assert_eq!(width_of(parent, abs, true), 260.0);
}

/// A column flex item that is a column flex container itself: the outer one
/// takes the item's margins out, and the item no longer takes its own out
/// too (it did, as a leaf did, before the outer one took them).
#[test]
fn a_column_flex_item_flex_container() {
    let inner = item(Style { display: Display::Flex, flex_direction: FlexDirection::Column, ..Default::default() });
    assert_eq!(width_of(column(), inner, true), 260.0);
}

/// An absolutely positioned leaf in a flex container: measured in the area
/// less its margins.
#[test]
fn an_absolutely_positioned_leaf_in_a_flex_container() {
    let parent = Style { display: Display::Flex, position: Position::Relative, ..Default::default() };
    let abs = item(Style { display: Display::Block, position: Position::Absolute, ..Default::default() });
    assert_eq!(width_of(parent, abs, false), 260.0);
}
