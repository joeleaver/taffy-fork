#![cfg(feature = "float_layout")]
use taffy::geometry::Point;
use taffy::prelude::*;
use taffy::style::{Clear, Float};
use taffy_test_helpers::new_test_tree;

/// Regression test for <https://wpt.live/css/CSS2/floats-clear/floats-146.xht>
///
/// A right float placed after two left floats must not be placed higher than the
/// outer top of the earlier floats (CSS2 float rule 5). This scenario previously
/// panicked in `FloatContext::subdivide_segment` when the float's end coincided
/// with an existing segment boundary due to floating point imprecision.
#[test]
fn float_no_higher_than_earlier_floats() {
    let mut taffy = new_test_tree();

    let block = |width: f32, height: f32, float: Float| Style {
        display: Display::Block,
        float,
        size: Size { width: length(width), height: length(height) },
        ..Default::default()
    };

    // Spacer block advancing the flow position to a fractional y offset (43.2)
    let spacer = taffy.new_leaf(block(400.0, 43.2, Float::None)).unwrap();
    let float_a = taffy.new_leaf(block(258.0, 42.0, Float::Left)).unwrap();
    let float_b = taffy.new_leaf(block(298.0, 42.0, Float::Left)).unwrap();
    let float_c = taffy.new_leaf(block(98.0, 42.0, Float::Right)).unwrap();

    let root = taffy
        .new_with_children(
            Style {
                display: Display::Block,
                size: Size { width: length(400.0), height: auto() },
                ..Default::default()
            },
            &[spacer, float_a, float_b, float_c],
        )
        .unwrap();

    taffy.compute_layout(root, Size::MAX_CONTENT).unwrap();

    let layout_a = taffy.layout(float_a).unwrap();
    let layout_b = taffy.layout(float_b).unwrap();
    let layout_c = taffy.layout(float_c).unwrap();

    // A is placed below the spacer
    assert_eq!(layout_a.location, Point { x: 0.0, y: 43.0 });
    // B does not fit next to A, so is placed below it
    assert_eq!(layout_b.location, Point { x: 0.0, y: 85.0 });
    // C must not be placed higher than B (it fits next to B on the right)
    assert_eq!(layout_c.location, Point { x: 302.0, y: 85.0 });
}

/// Regression test for <https://wpt.live/css/CSS2/floats-clear/margin-collapse-033.xht>
///
/// Floats that occupy no horizontal space (e.g. a zero-width float) still affect
/// `clear`: a cleared block must be placed past the bottom of all floats on the
/// relevant side, regardless of their width.
#[test]
fn clear_past_zero_width_float() {
    let mut taffy = new_test_tree();

    // float: left with auto width resolving to 0
    let floated = taffy
        .new_leaf(Style {
            display: Display::Block,
            float: Float::Left,
            size: Size { width: auto(), height: length(1.0) },
            ..Default::default()
        })
        .unwrap();
    let cleared = taffy.new_leaf(Style { display: Display::Block, clear: Clear::Left, ..Default::default() }).unwrap();
    let sibling = taffy
        .new_leaf(Style {
            display: Display::Block,
            margin: Rect { top: length(99.0), ..Rect::zero() },
            ..Default::default()
        })
        .unwrap();

    let container = taffy
        .new_with_children(
            Style {
                display: Display::Block,
                size: Size { width: length(100.0), height: auto() },
                ..Default::default()
            },
            &[floated, cleared, sibling],
        )
        .unwrap();
    let root = taffy.new_with_children(Style { display: Display::Block, ..Default::default() }, &[container]).unwrap();

    taffy.compute_layout(root, Size::MAX_CONTENT).unwrap();

    // The cleared (self-collapsing) block is placed below the float (y=1). Its clearance
    // prevents the sibling's 99px margin from collapsing with preceding margins, so the
    // container's height is 1 + 99 = 100.
    assert_eq!(taffy.layout(container).unwrap().size.height, 100.0);
}

fn float_block(width: f32, height: f32, float: Float) -> Style {
    Style {
        display: Display::Block,
        float,
        size: Size { width: length(width), height: length(height) },
        ..Default::default()
    }
}

fn root_style(width: f32) -> Style {
    Style { display: Display::Block, size: Size { width: length(width), height: auto() }, ..Default::default() }
}

/// Regression test for <https://wpt.live/css/CSS2/floats/floats-rule3-outside-left-001.xht>
///
/// CSS2 float rule 7: a float that is wider than its containing block and has no other
/// float beside it may overflow the containing block's edge rather than being pushed down.
#[test]
fn oversized_float_overflows_containing_block() {
    let mut taffy = new_test_tree();

    let float_a = taffy.new_leaf(float_block(150.0, 50.0, Float::Left)).unwrap();
    let root = taffy.new_with_children(root_style(100.0), &[float_a]).unwrap();

    taffy.compute_layout(root, Size::MAX_CONTENT).unwrap();

    assert_eq!(taffy.layout(float_a).unwrap().location, Point { x: 0.0, y: 0.0 });
}

/// CSS2 float rules 3 & 7: a float with another float beside it must not overlap that float
/// (rule 3) nor extend past the containing block's opposite edge (rule 7) - it must move down.
#[test]
fn float_beside_existing_float_moves_down_instead_of_overflowing() {
    let mut taffy = new_test_tree();

    // Rule 3: an opposite-direction float that doesn't fit must move down, not overlap
    let left = taffy.new_leaf(float_block(60.0, 50.0, Float::Left)).unwrap();
    let right = taffy.new_leaf(float_block(60.0, 50.0, Float::Right)).unwrap();
    // Rule 7: a same-direction float that doesn't fit must move down, not overflow the
    // containing block's trailing edge
    let left2 = taffy.new_leaf(float_block(60.0, 50.0, Float::Left)).unwrap();

    let root = taffy.new_with_children(root_style(100.0), &[left, right, left2]).unwrap();

    taffy.compute_layout(root, Size::MAX_CONTENT).unwrap();

    assert_eq!(taffy.layout(left).unwrap().location, Point { x: 0.0, y: 0.0 });
    assert_eq!(taffy.layout(right).unwrap().location, Point { x: 40.0, y: 50.0 });
    assert_eq!(taffy.layout(left2).unwrap().location, Point { x: 0.0, y: 100.0 });
}

/// rinch #2362: a float that reaches below every segment there is keeps its
/// whole height in the segments. Left floats 100x30 and 100x50: the second
/// stands beside the first and reaches 20px below it, and that tail was in no
/// segment, so a third float, and any line, saw the full width there. Chrome
/// 156: a third left float 120x10 goes below the second, to (0, 50), not
/// into the 100px gap left of it at y 30; a right float 120x10 goes to
/// (180, 50).
#[test]
fn float_reaching_below_every_segment_keeps_its_tail() {
    for (side, x) in [(Float::Left, 0.0), (Float::Right, 180.0)] {
        let mut taffy = new_test_tree();
        let a = taffy.new_leaf(float_block(100.0, 30.0, Float::Left)).unwrap();
        let b = taffy.new_leaf(float_block(100.0, 50.0, Float::Left)).unwrap();
        let c = taffy.new_leaf(float_block(120.0, 10.0, side)).unwrap();
        let root = taffy.new_with_children(root_style(300.0), &[a, b, c]).unwrap();
        taffy.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(taffy.layout(b).unwrap().location, Point { x: 100.0, y: 0.0 });
        assert_eq!(taffy.layout(c).unwrap().location, Point { x, y: 50.0 }, "{side:?}");
    }
}

/// The same off the round numbers, three floats deep: 150x23, then 60x47
/// beside it, then 40x61 beside both. A fourth left float 100x10 fits
/// beside none of them until the last ends: Chrome puts it at (0, 61).
#[test]
fn floats_reaching_below_each_other_keep_their_tails() {
    let mut taffy = new_test_tree();
    let a = taffy.new_leaf(float_block(150.0, 23.0, Float::Left)).unwrap();
    let b = taffy.new_leaf(float_block(60.0, 47.0, Float::Left)).unwrap();
    let c = taffy.new_leaf(float_block(40.0, 61.0, Float::Left)).unwrap();
    let d = taffy.new_leaf(float_block(100.0, 10.0, Float::Left)).unwrap();
    let root = taffy.new_with_children(root_style(300.0), &[a, b, c, d]).unwrap();
    taffy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(taffy.layout(c).unwrap().location, Point { x: 210.0, y: 0.0 });
    assert_eq!(taffy.layout(d).unwrap().location, Point { x: 0.0, y: 61.0 });
}

/// rinch #2397: a box that establishes a formatting context and sizes itself
/// (a sizing keyword width, or a table: `item_is_table`) sits beside a float
/// only if the width it takes there fits, and otherwise moves below it. The
/// fit test took the width an auto width stretches to, which always fits, so
/// an auto table (rinch's `width: fit-content` grid) holding a 40px box sat
/// beside a 280px float in 300px, overflowing by 20. Chrome 156: (0, 50); and
/// beside a 237px float, where 63px is room enough, (237, 0); at exactly 40px
/// of room (a 260px float), beside it.
#[test]
fn self_sized_formatting_context_root_moves_below_a_float_it_does_not_fit_beside() {
    for (float_width, width, item_is_table, expected) in [
        (280.0, Dimension::fit_content(), false, Point { x: 0.0, y: 50.0 }),
        (237.0, Dimension::fit_content(), false, Point { x: 237.0, y: 0.0 }),
        (260.0, Dimension::fit_content(), false, Point { x: 260.0, y: 0.0 }),
        (261.0, Dimension::fit_content(), false, Point { x: 0.0, y: 50.0 }),
        (280.0, Dimension::max_content(), false, Point { x: 0.0, y: 50.0 }),
        (280.0, Dimension::auto(), true, Point { x: 0.0, y: 50.0 }),
        (237.0, Dimension::auto(), true, Point { x: 237.0, y: 0.0 }),
    ] {
        let mut taffy = new_test_tree();
        let float = taffy.new_leaf(float_block(float_width, 50.0, Float::Left)).unwrap();
        let content = taffy
            .new_leaf(Style { size: Size { width: length(40.0), height: length(10.0) }, ..Default::default() })
            .unwrap();
        let subject = taffy
            .new_with_children(
                Style {
                    display: Display::Grid,
                    size: Size { width, height: auto() },
                    item_is_table,
                    ..Default::default()
                },
                &[content],
            )
            .unwrap();
        let root = taffy.new_with_children(root_style(300.0), &[float, subject]).unwrap();
        taffy.compute_layout(root, Size::MAX_CONTENT).unwrap();
        let layout = taffy.layout(subject).unwrap();
        assert_eq!(
            (layout.location, layout.size.width),
            (expected, 40.0),
            "float {float_width}, width {width:?}, item_is_table {item_is_table}"
        );
    }
}

/// The control: an auto-width formatting-context root that is stretched (a
/// grid with no width) still takes the room beside the float, however narrow,
/// as Chrome lays out an `overflow: hidden` or `display: flex` block there.
#[test]
fn stretched_formatting_context_root_stays_beside_a_float() {
    let mut taffy = new_test_tree();
    let float = taffy.new_leaf(float_block(280.0, 50.0, Float::Left)).unwrap();
    let content = taffy
        .new_leaf(Style { size: Size { width: length(40.0), height: length(10.0) }, ..Default::default() })
        .unwrap();
    let subject = taffy.new_with_children(Style { display: Display::Grid, ..Default::default() }, &[content]).unwrap();
    let root = taffy.new_with_children(root_style(300.0), &[float, subject]).unwrap();
    taffy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let layout = taffy.layout(subject).unwrap();
    assert_eq!((layout.location, layout.size.width), (Point { x: 280.0, y: 0.0 }, 20.0));
}

/// rinch #2415 review: the table rinch hands block layout is an `item_is_table`
/// grid with a *declared* width (and `table_rows`, as rinch builds a table),
/// which it widens past to its tracks' minimum.
/// A `width: 50px` table holding a 100px box is 100 wide, so beside a 250px
/// float in 300px it does not fit and moves below (Chrome 156: (0, 50), 100
/// wide). Fitting it by its declaration (50 <= 50) put it beside, overflowing.
#[test]
fn table_with_a_declared_width_fits_by_the_width_it_widens_to() {
    let mut taffy = new_test_tree();
    let float = taffy.new_leaf(float_block(250.0, 50.0, Float::Left)).unwrap();
    let content = taffy
        .new_leaf(Style { size: Size { width: length(100.0), height: length(10.0) }, ..Default::default() })
        .unwrap();
    let table = taffy
        .new_with_children(
            Style {
                display: Display::Grid,
                size: Size { width: length(50.0), height: auto() },
                item_is_table: true,
                proportional_column_sizing: true,
                // rinch's auto column in a table with a width, and its one body row
                grid_template_columns: vec![minmax(min_content(), auto())],
                table_rows: Some(taffy::TableRows {
                    rows: vec![taffy::TableRow { caption: false, section: 0, body: true }],
                    sized: true,
                }),
                ..Default::default()
            },
            &[content],
        )
        .unwrap();
    let root = taffy.new_with_children(root_style(300.0), &[float, table]).unwrap();
    taffy.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let layout = taffy.layout(table).unwrap();
    assert_eq!((layout.location, layout.size.width), (Point { x: 0.0, y: 50.0 }, 100.0));
}
