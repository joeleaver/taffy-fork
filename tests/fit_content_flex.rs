//! A flex container sized to fit its content takes
//! `min(max-content, max(min-content, room))` in its main axis (CSS Sizing 3
//! §5.2 "fit-content"; rinch #2370). It took its max-content width, the sum of
//! its items' flex bases, whatever the room: a float holding wrapping text
//! came out as wide as the text unwrapped.
//!
//! The min-content size is the sum of the items' min-content contributions in
//! a single-line container, and the largest of them in a multi-line one (CSS
//! Flexbox 1 §9.9.1).
//!
//! The leaf is text-like: 520 wide at max-content, 100 at min-content, and
//! otherwise as wide as its room, in 20px lines. Each container is a float
//! (or another shrink-to-fit box) with margins 23 and 17 in a 300px block, so
//! its room is 260. `rinch-dom`'s `fit_content_flex_tests.rs` has the same
//! shapes with real text and Chrome 156's numbers.
#![cfg(all(feature = "block_layout", feature = "flexbox", feature = "grid", feature = "float_layout"))]

use taffy::prelude::*;
use taffy::{compute_leaf_layout, Float, Overflow, Point};

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

/// What a test builds: the container's style, its parent's, and its items.
struct Case {
    parent: Style,
    container: Style,
    /// One style per item; each item is a block holding one text leaf.
    items: Vec<Style>,
}

/// The border box of the container: (width, height).
fn layout(case: Case) -> (f32, f32) {
    let mut tree: TaffyTree<bool> = TaffyTree::new();
    let mut items = Vec::new();
    for item in case.items {
        let leaf = tree.new_leaf_with_context(Style { display: Display::Block, ..Default::default() }, true).unwrap();
        items.push(tree.new_with_children(item, &[leaf]).unwrap());
    }
    let container = tree.new_with_children(Style { margin: margins(), ..case.container }, &items).unwrap();
    let mut parent_style = case.parent;
    parent_style.size.width = length(300.0);
    let parent = tree.new_with_children(parent_style, &[container]).unwrap();
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
    let size = tree.layout(container).unwrap().size;
    (size.width, size.height)
}

fn block() -> Style {
    Style { display: Display::Block, ..Default::default() }
}

fn row() -> Style {
    Style { display: Display::Flex, ..Default::default() }
}

fn floated(style: Style) -> Style {
    Style { float: Float::Left, ..style }
}

fn items(n: usize) -> Vec<Style> {
    vec![block(); n]
}

/// The issue's shape: one wrapping item. Its 520 does not fit the 260, so
/// the container is 260 and the item wraps to two lines.
#[test]
fn a_float_row_of_one_wrapping_item() {
    assert_eq!(layout(Case { parent: block(), container: floated(row()), items: items(1) }), (260.0, 40.0));
}

/// Content narrower than the room still sets the width: a 100px-wide item.
#[test]
fn a_float_row_narrower_than_its_room() {
    let narrow = Style { size: Size { width: length(130.0), height: auto() }, ..block() };
    assert_eq!(layout(Case { parent: block(), container: floated(row()), items: vec![narrow] }).0, 130.0);
}

/// Two items: max-content 1040, min-content 200 (single line: the sum), so
/// the room. Each shrinks to 130 and wraps to four lines (520 / 130).
#[test]
fn a_float_row_of_two_items() {
    assert_eq!(layout(Case { parent: block(), container: floated(row()), items: items(2) }), (260.0, 80.0));
}

/// Three items: their min-contents sum to 300, more than the room: a
/// single-line container is never narrower than that.
#[test]
fn a_float_row_of_three_items_overflows_to_its_min_content() {
    assert_eq!(layout(Case { parent: block(), container: floated(row()), items: items(3) }).0, 300.0);
}

/// A gap counts toward the min-content: 3 x 100 + 2 x 15.
#[test]
fn a_float_row_of_three_items_with_a_gap() {
    let container = Style { gap: Size { width: length(15.0), height: zero() }, ..floated(row()) };
    assert_eq!(layout(Case { parent: block(), container, items: items(3) }).0, 330.0);
}

/// Two items with a gap in the room: they share 260 - 15.
#[test]
fn a_float_row_of_two_items_with_a_gap() {
    let container = Style { gap: Size { width: length(15.0), height: zero() }, ..floated(row()) };
    assert_eq!(layout(Case { parent: block(), container, items: items(2) }).0, 260.0);
}

/// An item's padding and margins count toward its min-content
/// contribution: 2 x (100 + 2 x 7 + 9 + 4) = 254, under the room: 260.
/// With three, 381.
#[test]
fn item_padding_and_margins_count() {
    let item = Style {
        padding: Rect { left: length(7.0), right: length(7.0), top: zero(), bottom: zero() },
        margin: Rect { left: length(9.0), right: length(4.0), top: zero(), bottom: zero() },
        ..block()
    };
    let case = |n| Case { parent: block(), container: floated(row()), items: vec![item.clone(); n] };
    assert_eq!(layout(case(2)).0, 260.0);
    assert_eq!(layout(case(3)).0, 381.0);
}

/// A multi-line container's min-content is its largest item's: three items
/// wrap onto three lines of 260 each.
#[test]
fn a_float_wrapping_row_of_three_items() {
    let container = Style { flex_wrap: FlexWrap::Wrap, ..floated(row()) };
    assert_eq!(layout(Case { parent: block(), container, items: items(3) }), (260.0, 120.0));
}

/// A wrapping container of one item: the item alone on its line is wider
/// than the room, and the container is the room.
#[test]
fn a_float_wrapping_row_of_one_item() {
    let container = Style { flex_wrap: FlexWrap::Wrap, ..floated(row()) };
    assert_eq!(layout(Case { parent: block(), container, items: items(1) }), (260.0, 40.0));
}

/// An item with a fixed width wider than the room: a wrapping container's
/// min-content is that width, 280.
#[test]
fn a_float_wrapping_row_with_a_wide_fixed_item() {
    let wide = Style { size: Size { width: length(280.0), height: auto() }, ..block() };
    let container = Style { flex_wrap: FlexWrap::Wrap, ..floated(row()) };
    assert_eq!(layout(Case { parent: block(), container, items: vec![block(), wide] }).0, 280.0);
}

/// A column's width is its cross size: the item measured in the room.
#[test]
fn a_float_column() {
    let container = Style { flex_direction: FlexDirection::Column, ..floated(row()) };
    assert_eq!(layout(Case { parent: block(), container, items: items(2) }), (260.0, 80.0));
}

#[test]
fn an_absolutely_positioned_row() {
    let parent = Style { position: Position::Relative, ..block() };
    let container = Style { position: Position::Absolute, ..row() };
    assert_eq!(layout(Case { parent, container, items: items(1) }), (260.0, 40.0));
}

#[test]
fn a_fit_content_row() {
    let container = Style { size: Size { width: Dimension::fit_content(), height: auto() }, ..row() };
    assert_eq!(layout(Case { parent: block(), container, items: items(1) }), (260.0, 40.0));
}

#[test]
fn a_start_aligned_grid_item_row() {
    let grid = Style { display: Display::Grid, justify_items: Some(AlignItems::START), ..Default::default() };
    assert_eq!(layout(Case { parent: grid, container: row(), items: items(1) }), (260.0, 40.0));
}

/// A row in a column aligned to its start: its width is fit-content too.
#[test]
fn a_row_in_a_start_aligned_column() {
    let column = Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::FLEX_START),
        ..Default::default()
    };
    assert_eq!(layout(Case { parent: column, container: row(), items: items(1) }), (260.0, 40.0));
}

/// A max-width below the fit-content width still wins, and a min-width above it.
#[test]
fn min_and_max_width_clamp_it() {
    let capped = Style { max_size: Size { width: length(210.0), height: auto() }, ..floated(row()) };
    assert_eq!(layout(Case { parent: block(), container: capped, items: items(1) }).0, 210.0);
    let floored = Style { min_size: Size { width: length(290.0), height: auto() }, ..floated(row()) };
    assert_eq!(layout(Case { parent: block(), container: floored, items: items(1) }).0, 290.0);
}

/// An item that does not shrink keeps its basis and overflows: its
/// min-content contribution is its min-content size, not its basis (Chrome
/// 156 does not clamp a contribution by the flex base size), so the
/// container is the room.
#[test]
fn an_item_that_does_not_shrink() {
    let rigid = Style { flex_shrink: 0.0, ..block() };
    assert_eq!(layout(Case { parent: block(), container: floated(row()), items: vec![rigid] }), (260.0, 20.0));
}

/// Items with `flex: 1 1 0`: their bases sum to 0, but the max-content size
/// is their max-content contributions' (1040), so the container is the room.
#[test]
fn items_of_zero_basis_that_grow() {
    let grow = Style { flex_grow: 1.0, flex_basis: length(0.0), ..block() };
    assert_eq!(layout(Case { parent: block(), container: floated(row()), items: vec![grow; 2] }), (260.0, 80.0));
}

/// An item's min-width floors its min-content contribution: 2 x 200.
#[test]
fn items_with_a_min_width() {
    let floored = Style { min_size: Size { width: length(200.0), height: auto() }, ..block() };
    assert_eq!(layout(Case { parent: block(), container: floated(row()), items: vec![floored; 2] }), (400.0, 60.0));
}

/// A fixed item beside a wrapping one: 120 + 100 fits, so the room.
#[test]
fn a_fixed_item_beside_a_wrapping_one() {
    let fixed = Style { size: Size { width: length(120.0), height: auto() }, flex_shrink: 0.0, ..block() };
    // The fixed item holds text too: 520 in 120 is five lines.
    assert_eq!(
        layout(Case { parent: block(), container: floated(row()), items: vec![fixed, block()] }),
        (260.0, 100.0)
    );
}

/// A scroll container item still contributes its content's min-content
/// size (its automatic minimum size is 0, its contribution is not): three
/// of them are 300 wide.
#[test]
fn scroll_container_items() {
    let clipped = Style { overflow: Point { x: Overflow::Hidden, y: Overflow::Hidden }, ..block() };
    assert_eq!(
        layout(Case { parent: block(), container: floated(row()), items: vec![clipped.clone()] }),
        (260.0, 40.0)
    );
    assert_eq!(layout(Case { parent: block(), container: floated(row()), items: vec![clipped; 3] }).0, 300.0);
}

/// A column is fit-content in its cross axis: items narrower than the room
/// set it.
#[test]
fn a_float_column_of_narrow_items() {
    let narrow = Style { size: Size { width: length(79.0), height: auto() }, ..block() };
    let container = Style { flex_direction: FlexDirection::Column, ..floated(row()) };
    assert_eq!(layout(Case { parent: block(), container, items: vec![narrow; 2] }).0, 79.0);
}

/// The container's own padding comes on top of its items.
#[test]
fn padding_on_the_container_comes_on_top() {
    let container = Style {
        padding: Rect { left: length(11.0), right: length(6.0), top: zero(), bottom: zero() },
        ..floated(row())
    };
    // 260 border box: 243 for the item, which wraps to three lines.
    assert_eq!(layout(Case { parent: block(), container, items: items(1) }), (260.0, 60.0));
}

/// A wrapping container whose lines each fit: the room, padding included
/// (it was the room less the padding: 243). 3 x 100 + 17 is more than 260.
#[test]
fn a_float_wrapping_row_with_padding() {
    let fixed = Style { size: Size { width: length(100.0), height: auto() }, ..block() };
    let container = Style {
        flex_wrap: FlexWrap::Wrap,
        padding: Rect { left: length(11.0), right: length(6.0), top: zero(), bottom: zero() },
        ..floated(row())
    };
    assert_eq!(layout(Case { parent: block(), container, items: vec![fixed; 3] }).0, 260.0);
}

/// A column's main size is its height, its max-content size even where the
/// room is definite and shorter: 2 x 40, in a parent 30 tall.
#[test]
fn a_float_column_taller_than_its_parent() {
    let parent = Style { size: Size { width: auto(), height: length(30.0) }, ..block() };
    let container = Style { flex_direction: FlexDirection::Column, ..floated(row()) };
    assert_eq!(layout(Case { parent, container, items: items(2) }).1, 80.0);
}
