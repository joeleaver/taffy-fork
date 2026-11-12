//! A CSS table's outer size in other formatting contexts (rinch #2403, #2395,
//! #2328): an automatic-layout table (`proportional_column_sizing` and sized
//! `table_rows`) is never narrower than its columns' minimum, whatever width it
//! is handed, and its insets place an absolutely positioned one without sizing
//! it. The numbers are Chrome 156's for the same markup: one column holding a
//! 300x20 box, `border-spacing: 0`, `padding: 0`.
#![cfg(all(feature = "block_layout", feature = "flexbox", feature = "grid"))]

use taffy::prelude::*;
use taffy::{TableRow, TableRows};

/// A one-cell table whose cell holds a `content_w`x20 box, with `size`.
fn table(tree: &mut TaffyTree<()>, content_w: f32, size: Size<Dimension>, extra: Style) -> NodeId {
    let content = tree
        .new_leaf(Style { size: Size { width: length(content_w), height: length(20.0) }, ..Default::default() })
        .unwrap();
    let cell = tree
        .new_with_children(
            Style {
                display: Display::Block,
                grid_row: Line { start: line(1), end: taffy::style_helpers::span(1) },
                grid_column: Line { start: line(1), end: taffy::style_helpers::span(1) },
                ..Default::default()
            },
            &[content],
        )
        .unwrap();
    tree.new_with_children(
        Style {
            display: Display::Grid,
            item_is_table: true,
            proportional_column_sizing: true,
            size,
            grid_template_columns: vec![GridTemplateComponent::Single(auto())],
            table_rows: Some(TableRows {
                rows: vec![TableRow { caption: false, section: 0, body: true }],
                sized: true,
            }),
            ..extra
        },
        &[cell],
    )
    .unwrap()
}

fn width_100() -> Size<Dimension> {
    Size { width: length(100.0), height: auto() }
}

fn rect(tree: &TaffyTree<()>, node: NodeId) -> (f32, f32, f32, f32) {
    let l = tree.layout(node).unwrap();
    (l.location.x, l.location.y, l.size.width, l.size.height)
}

#[test]
fn a_width_is_floored_at_the_columns_in_a_row_flex_container() {
    // Chrome: 300 wide, also with `min-width: 0`; the next item starts at 300.
    let mut tree = TaffyTree::new();
    let t = table(
        &mut tree,
        300.0,
        width_100(),
        Style { min_size: Size { width: length(0.0), height: auto() }, ..Default::default() },
    );
    let s =
        tree.new_leaf(Style { size: Size { width: length(10.0), height: length(5.0) }, ..Default::default() }).unwrap();
    let root = tree
        .new_with_children(
            Style { display: Display::Flex, size: Size { width: length(600.0), height: auto() }, ..Default::default() },
            &[t, s],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(rect(&tree, t).2, 300.0);
    assert_eq!(rect(&tree, s).0, 300.0);
}

#[test]
fn a_width_is_floored_at_the_columns_as_a_grid_item_and_in_a_column() {
    // Chrome: 300 in a 100px track (overflowing it) and in a 100px column flex container.
    for display in [Display::Grid, Display::Flex] {
        let mut tree = TaffyTree::new();
        let t = table(&mut tree, 300.0, width_100(), Style::default());
        let root = tree
            .new_with_children(
                Style {
                    display,
                    flex_direction: FlexDirection::Column,
                    grid_template_columns: vec![GridTemplateComponent::Single(length(100.0))],
                    size: Size { width: length(100.0), height: auto() },
                    ..Default::default()
                },
                &[t],
            )
            .unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(rect(&tree, t).2, 300.0, "{display:?}");
    }
}

#[test]
fn an_auto_width_table_is_stretched_in_a_column_and_fits_in_a_block() {
    // Chrome: 800 as a column flex item; 30 in a block container (shrink to fit).
    let mut tree = TaffyTree::new();
    let t = table(&mut tree, 30.0, Size::auto(), Style::default());
    let root = tree
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                size: Size { width: length(800.0), height: auto() },
                ..Default::default()
            },
            &[t],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(rect(&tree, t).2, 800.0);

    let mut tree = TaffyTree::new();
    let t = table(&mut tree, 30.0, Size::auto(), Style::default());
    let root = tree
        .new_with_children(
            Style {
                display: Display::Block,
                size: Size { width: length(800.0), height: auto() },
                ..Default::default()
            },
            &[t],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(rect(&tree, t).2, 30.0);
}

#[test]
fn a_shrink_to_fit_block_is_as_wide_as_a_tables_columns() {
    // Chrome: an inline-block (here a block measured at max-content) around a `width: 100px`
    // table of a 300px column is 300 wide.
    let mut tree = TaffyTree::new();
    let t = table(&mut tree, 300.0, width_100(), Style::default());
    let wrapper = tree.new_with_children(Style { display: Display::Block, ..Default::default() }, &[t]).unwrap();
    let root = tree
        .new_with_children(
            Style { display: Display::Flex, align_items: Some(AlignItems::START), ..Default::default() },
            &[wrapper],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(rect(&tree, wrapper).2, 300.0);
    assert_eq!(rect(&tree, t).2, 300.0);
}

#[test]
fn an_absolutely_positioned_tables_insets_do_not_size_it() {
    // Chrome: `top: 10px; bottom: 10px; left: 10px; right: 10px` in a 300px-tall box: the
    // table is 30x20 at (10, 10), in a block, flex and grid container alike; with `width:
    // 100px` it is 300 wide.
    for display in [Display::Block, Display::Flex, Display::Grid] {
        for (content_w, size, want_w) in [(30.0, Size::auto(), 30.0), (300.0, width_100(), 300.0)] {
            let mut tree = TaffyTree::new();
            let t = table(
                &mut tree,
                content_w,
                size,
                Style {
                    position: Position::Absolute,
                    inset: Rect { left: length(10.0), right: length(10.0), top: length(10.0), bottom: length(10.0) },
                    ..Default::default()
                },
            );
            let root = tree
                .new_with_children(
                    Style { display, size: Size { width: length(800.0), height: length(300.0) }, ..Default::default() },
                    &[t],
                )
                .unwrap();
            tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
            assert_eq!(rect(&tree, t), (10.0, 10.0, want_w, 20.0), "{display:?} {content_w}");
        }
    }
}

#[test]
fn a_table_between_left_and_right_shrinks_to_fit_their_room() {
    // Chrome: a cell holding ten 50x10 boxes that wrap, `left` and `right` 10px (or 50px) in a
    // 300px box: the table is 280 (200) wide at x=10 (50), two (three) lines tall. Laid out in
    // the whole width, it was 300 wide (review of rinch #2600). A wrapping flex row stands in
    // for the line of boxes.
    for display in [Display::Block, Display::Flex, Display::Grid] {
        for (inset, want) in [(10.0, (10.0, 0.0, 280.0, 20.0)), (50.0, (50.0, 0.0, 200.0, 30.0))] {
            let mut tree = TaffyTree::new();
            let boxes: Vec<NodeId> = (0..10)
                .map(|_| {
                    tree.new_leaf(Style {
                        flex_shrink: 0.0,
                        size: Size { width: length(50.0), height: length(10.0) },
                        ..Default::default()
                    })
                    .unwrap()
                })
                .collect();
            let row = tree
                .new_with_children(
                    Style { display: Display::Flex, flex_wrap: FlexWrap::Wrap, ..Default::default() },
                    &boxes,
                )
                .unwrap();
            let cell = tree
                .new_with_children(
                    Style {
                        display: Display::Block,
                        grid_row: Line { start: line(1), end: taffy::style_helpers::span(1) },
                        grid_column: Line { start: line(1), end: taffy::style_helpers::span(1) },
                        ..Default::default()
                    },
                    &[row],
                )
                .unwrap();
            let t = tree
                .new_with_children(
                    Style {
                        display: Display::Grid,
                        item_is_table: true,
                        proportional_column_sizing: true,
                        position: Position::Absolute,
                        inset: Rect { left: length(inset), right: length(inset), top: length(0.0), bottom: auto() },
                        grid_template_columns: vec![GridTemplateComponent::Single(auto())],
                        table_rows: Some(TableRows {
                            rows: vec![TableRow { caption: false, section: 0, body: true }],
                            sized: true,
                        }),
                        ..Default::default()
                    },
                    &[cell],
                )
                .unwrap();
            let root = tree
                .new_with_children(
                    Style { display, size: Size { width: length(300.0), height: length(200.0) }, ..Default::default() },
                    &[t],
                )
                .unwrap();
            tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
            assert_eq!(rect(&tree, t), want, "{display:?} {inset}");
        }
    }
}
