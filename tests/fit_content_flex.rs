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

/// The container's padding comes on top of its min-content size too: two
/// items' 200 and 70 of padding pass the room, 270.
#[test]
fn padding_on_top_of_the_min_content() {
    let container = Style {
        padding: Rect { left: length(35.0), right: length(35.0), top: zero(), bottom: zero() },
        ..floated(row())
    };
    assert_eq!(layout(Case { parent: block(), container, items: items(2) }).0, 270.0);
}

/// Like [`layout`], but each item is a text leaf itself (rinch's IFC leaf),
/// with the given style; the leaf's measure is not clamped by its max size.
fn layout_leaf_items(container: Style, items: Vec<Style>) -> f32 {
    let mut tree: TaffyTree<bool> = TaffyTree::new();
    let items: Vec<_> = items.into_iter().map(|s| tree.new_leaf_with_context(s, true).unwrap()).collect();
    let container = tree.new_with_children(Style { margin: margins(), ..container }, &items).unwrap();
    let parent = tree
        .new_with_children(Style { size: Size { width: length(300.0), height: auto() }, ..block() }, &[container])
        .unwrap();
    tree.compute_layout_with_measure(parent, Size::MAX_CONTENT, |inputs, _, ctx, style| match ctx {
        Some(true) => compute_leaf_layout(inputs, style, |_, _| 0.0, text),
        _ => compute_leaf_layout(inputs, style, |_, _| 0.0, |_, _| Size::ZERO),
    })
    .unwrap();
    tree.layout(container).unwrap().size.width
}

/// An item's max-width caps its max-content contribution (rinch #2389
/// review): 80 + 50 fits the room. Chrome 156 makes `max-width: 80px` text
/// beside "alpha" 121.11 wide; it was the room. A min-width above the max
/// wins: 120 + 50.
#[test]
fn an_item_max_width_caps_the_max_content() {
    let capped = Style { max_size: Size { width: length(80.0), height: auto() }, ..block() };
    let fixed = Style { size: Size { width: length(50.0), height: auto() }, ..block() };
    for wrap in [FlexWrap::NoWrap, FlexWrap::Wrap] {
        let container = Style { flex_wrap: wrap, ..floated(row()) };
        assert_eq!(layout_leaf_items(container, vec![capped.clone(), fixed.clone()]), 130.0, "{wrap:?}");
    }
    let floored = Style { min_size: Size { width: length(120.0), height: auto() }, ..capped };
    assert_eq!(layout_leaf_items(floated(row()), vec![floored, fixed]), 170.0);
}

/// Like [`layout`], also returning each item's border box width.
fn layout_with_items(case: Case) -> ((f32, f32), Vec<f32>) {
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
    tree.compute_layout_with_measure(parent, Size::MAX_CONTENT, |inputs, _, ctx, style| match ctx {
        Some(true) => compute_leaf_layout(inputs, style, |_, _| 0.0, text),
        _ => compute_leaf_layout(inputs, style, |_, _| 0.0, |_, _| Size::ZERO),
    })
    .unwrap();
    let size = tree.layout(container).unwrap().size;
    let widths = items.iter().map(|&i| tree.layout(i).unwrap().size.width).collect();
    ((size.width, size.height), widths)
}

fn width_percent(p: f32) -> Style {
    Style { size: Size { width: percent(p), height: auto() }, ..block() }
}

/// An item's percentage width resolves against the width the container
/// sizes itself to (rinch #2392): the items' percentages behave as `auto`
/// for that (the room, 260, as two plain items make it), then 30% and 70% of
/// it are 78 and 182: 7 lines of text 78 wide, and 3. They resolved against nothing, so both
/// items were 130 wide and 80 tall, as plain ones. Chrome 157 gives a float
/// with `width: 50%` items "alpha beta" and "gamma delta" 88.44 each, half
/// of the 176.88 they sum to (rinch-dom `fit_content_flex_tests.rs`).
#[test]
fn a_float_row_resolves_its_items_percentage_widths_against_its_width() {
    let case = Case { parent: block(), container: floated(row()), items: vec![width_percent(0.3), width_percent(0.7)] };
    assert_eq!(layout_with_items(case), ((260.0, 140.0), vec![78.0, 182.0]));
}

/// A percentage flex basis likewise, in a row whose width a start-aligned
/// grid item's fit-content sizing sets: bases 78 and 182, the first raised to
/// its automatic minimum, the text's 100 min-content, so the second shrinks to
/// 160. (They were 130 and 130.)
#[test]
fn a_grid_items_row_resolves_its_items_percentage_bases_against_its_width() {
    let basis = |p: f32| Style { flex_basis: percent(p), ..block() };
    let parent = Style { display: Display::Grid, justify_items: Some(AlignItems::START), ..Default::default() };
    let case = Case { parent, container: row(), items: vec![basis(0.3), basis(0.7)] };
    assert_eq!(layout_with_items(case), ((260.0, 120.0), vec![100.0, 160.0]));
}

/// A column's item resolves its percentage width, its cross size, against
/// the float's width: the max-content 520 does not fit the room, so 260;
/// 30% of it is 78.
#[test]
fn a_float_column_resolves_an_items_percentage_width_against_its_width() {
    let column = Style { flex_direction: FlexDirection::Column, ..floated(row()) };
    let case = Case { parent: block(), container: column, items: vec![width_percent(0.3), block()] };
    assert_eq!(layout_with_items(case).1, vec![78.0, 260.0]);
}

/// Measure calls for `depth` nested floats, each a flex row of a `width: 90%`
/// item holding the next and a text leaf (review of rinch #2611). Sizing the
/// width with a full flex pass before the layout laid each level out at more
/// distinct sizes the deeper it was, once the innermost text was narrower than
/// its minimum (from depth 11): 385 measures at depth 12, 1,450 at 13, 4,711
/// at 14. The width now comes from the container's min- and max-content sizes,
/// which the cache keeps: 146, 167, 191, settling at 28 more per level.
fn nested_percentage_floats_measures(depth: usize) -> usize {
    let mut tree: TaffyTree<bool> = TaffyTree::new();
    let mut inner = tree.new_leaf_with_context(block(), true).unwrap();
    for _ in 0..depth {
        let item = tree.new_with_children(width_percent(0.9), &[inner]).unwrap();
        let text = tree.new_leaf_with_context(block(), true).unwrap();
        inner = tree.new_with_children(floated(row()), &[item, text]).unwrap();
    }
    let root = tree
        .new_with_children(Style { size: Size { width: length(300.0), height: auto() }, ..block() }, &[inner])
        .unwrap();
    let mut calls = 0;
    tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |inputs, _, ctx, style| match ctx {
        Some(true) => {
            calls += 1;
            compute_leaf_layout(inputs, style, |_, _| 0.0, text)
        }
        _ => compute_leaf_layout(inputs, style, |_, _| 0.0, |_, _| Size::ZERO),
    })
    .unwrap();
    calls
}

#[test]
fn nested_percentage_floats_measure_linearly_in_depth() {
    // Past depth 11 a level costs 16 to 28 measures; it was 1,065 at depth 12 and tripling.
    let step = |depth: usize| nested_percentage_floats_measures(depth + 1) - nested_percentage_floats_measures(depth);
    let (at_11, at_13) = (step(11), step(13));
    assert!(at_13 <= 2 * at_11, "measures per level at depth 11: {at_11}, at 13: {at_13}");
}
