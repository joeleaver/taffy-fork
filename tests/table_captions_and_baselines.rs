//! Two `Style::table_rows` cases (rinch #2596 and #2598). The numbers are
//! Chrome 156's for the same tables (`border-spacing: 0`, `padding: 0` but
//! where a case says), unrounded.
//!
//! - **#2596.** A height the parent hands a table (a stretched or flexed
//!   item, an absolutely positioned table laid out at the height it
//!   measured) is its wrapper's, captions included; its own `height` leaves
//!   them out. The rows shared the whole handed height, so the table came
//!   out one caption too tall.
//! - **#2598.** `Style::table_row_baseline`: a row is at least as tall as its
//!   baseline-aligned cells' lowest baseline plus the deepest extent of one
//!   of span 1 below its own baseline.
#![cfg(all(feature = "block_layout", feature = "grid", feature = "flexbox"))]

use taffy::prelude::*;
use taffy::{TableRow, TableRows};

const BODY: TableRow = TableRow { caption: false, section: 0, body: true };
const CAPTION: TableRow = TableRow { caption: true, section: 0, body: false };

/// A cell at row line `row` holding a 10px-wide box `h` tall.
fn cell(tree: &mut TaffyTree<()>, row: i16, h: f32) -> NodeId {
    let content =
        tree.new_leaf(Style { size: Size { width: length(10.0), height: length(h) }, ..Default::default() }).unwrap();
    tree.new_with_children(
        Style {
            display: Display::Block,
            grid_row: Line { start: line(row), end: span(1) },
            grid_column: Line { start: line(1), end: span(1) },
            ..Default::default()
        },
        &[content],
    )
    .unwrap()
}

/// A 13px caption above rows of 27 and 19: 59 tall, as in flow.
fn captioned(tree: &mut TaffyTree<()>, extra: impl FnOnce(&mut Style)) -> (NodeId, [NodeId; 2]) {
    let caption = cell(tree, 1, 13.0);
    let a = cell(tree, 2, 27.0);
    let b = cell(tree, 3, 19.0);
    let mut style = Style {
        display: Display::Grid,
        grid_template_columns: vec![GridTemplateComponent::Single(auto())],
        table_rows: Some(TableRows { rows: vec![CAPTION, BODY, BODY], sized: true }),
        ..Default::default()
    };
    extra(&mut style);
    (tree.new_with_children(style, &[caption, a, b]).unwrap(), [a, b])
}

/// The table's height and its two rows', laid out in a 300x200 `parent`.
fn in_parent(parent: Style, extra: impl FnOnce(&mut Style)) -> [f32; 3] {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let (t, [a, b]) = captioned(&mut tree, extra);
    let root = tree
        .new_with_children(Style { size: Size { width: length(300.0), height: length(200.0) }, ..parent }, &[t])
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    [t, a, b].map(|n| tree.unrounded_layout(n).size.height)
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(a, b)| (a - b).abs() < 0.02)
}

#[test]
fn a_handed_height_holds_the_captions() {
    let flex_start = Style { display: Display::Flex, align_items: Some(AlignItems::START), ..Default::default() };
    let abspos = |s: &mut Style| {
        s.position = Position::Absolute;
        s.inset.top = length(7.0);
    };
    // As tall as in flow: 13 + 27 + 19 (was 72, the 13 given to the rows too).
    assert!(close(in_parent(flex_start.clone(), |_| {}), [59.0, 27.0, 19.0]));
    assert!(close(in_parent(Style::default(), abspos), [59.0, 27.0, 19.0]));
    // Stretched to 200: the rows share 187 by their heights (27 : 19).
    let stretch = Style { display: Display::Flex, ..Default::default() };
    assert!(close(in_parent(stretch, |_| {}), [200.0, 27.0 + 141.0 * 27.0 / 46.0, 19.0 + 141.0 * 19.0 / 46.0]));
    // Its own `height` is the table box's: 90 for the rows, the caption on top.
    let own = |s: &mut Style| s.size.height = length(90.0);
    assert!(close(in_parent(flex_start, own), [103.0, 27.0 + 44.0 * 27.0 / 46.0, 19.0 + 44.0 * 19.0 / 46.0]));
}

/// A leaf cell: `h` of content with its first baseline at `baseline`, below
/// `top` of padding and above `bottom`; `min_h` is a cell `height`.
struct Leaf {
    h: f32,
    baseline: Option<f32>,
    /// Its top padding, set by `baseline_cell`.
    top: f32,
}

fn baseline_cell(
    tree: &mut TaffyTree<Leaf>,
    row: i16,
    rows: u16,
    leaf: Leaf,
    (top, bottom): (f32, f32),
    min_h: Option<f32>,
) -> NodeId {
    let leaf = Leaf { top, ..leaf };
    tree.new_leaf_with_context(
        Style {
            grid_row: Line { start: line(row), end: span(rows) },
            grid_column: Line { start: auto(), end: span(1) },
            padding: Rect { left: zero(), right: zero(), top: length(top), bottom: length(bottom) },
            min_size: Size { width: auto(), height: min_h.map_or(auto(), length) },
            align_self: Some(AlignSelf::STRETCH),
            table_row_baseline: true,
            box_sizing: BoxSizing::ContentBox,
            ..Default::default()
        },
        leaf,
    )
    .unwrap()
}

/// Lay out a table of `cells` in `rows` body rows; their heights.
fn baseline_rows(tree: &mut TaffyTree<Leaf>, rows: usize, cells: &[NodeId]) -> Vec<f32> {
    let t = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                grid_auto_flow: GridAutoFlow::Column,
                table_rows: Some(TableRows { rows: vec![BODY; rows], sized: true }),
                ..Default::default()
            },
            cells,
        )
        .unwrap();
    tree.compute_layout_with_measure(t, Size::MAX_CONTENT, |inputs, _, leaf, style| {
        let Some(leaf) = leaf else { return taffy::LayoutOutput::HIDDEN };
        let (h, baseline, top) = (leaf.h, leaf.baseline, leaf.top);
        let mut out = taffy::compute_leaf_layout(inputs, style, |_, _| 0.0, |_, _| Size { width: 10.0, height: h });
        out.baselines.first = baseline.map(|b| top + b);
        out
    })
    .unwrap();
    cells.iter().map(|&c| tree.unrounded_layout(c).size.height).collect()
}

#[test]
fn a_row_grows_to_its_lowest_baseline_and_deepest_extent() {
    // The issue's row: a 20px line (baseline 16) below 5 and above 30, and
    // one below 20. The row's baseline is 36, the first cell reaching 34
    // below its own: 70 (55 before).
    let mut tree: TaffyTree<Leaf> = TaffyTree::new();
    let a = baseline_cell(&mut tree, 1, 1, Leaf { h: 20.0, baseline: Some(16.0), top: 0.0 }, (5.0, 30.0), None);
    let b = baseline_cell(&mut tree, 1, 1, Leaf { h: 20.0, baseline: Some(16.0), top: 0.0 }, (20.0, 0.0), None);
    assert_eq!(baseline_rows(&mut tree, 1, &[a, b]), [70.0, 70.0]);

    // A cell `height` (40, 75 with its padding) is a minimum on the row, not
    // a deeper extent: 75, not 36 + 54.
    let mut tree: TaffyTree<Leaf> = TaffyTree::new();
    let a = baseline_cell(&mut tree, 1, 1, Leaf { h: 20.0, baseline: Some(16.0), top: 0.0 }, (5.0, 30.0), Some(40.0));
    let b = baseline_cell(&mut tree, 1, 1, Leaf { h: 20.0, baseline: Some(16.0), top: 0.0 }, (20.0, 0.0), None);
    assert_eq!(baseline_rows(&mut tree, 1, &[a, b]), [75.0, 75.0]);

    // A cell with nothing in it (no baseline, no children) takes no part:
    // the row is the empty cell's 59.
    let mut tree: TaffyTree<Leaf> = TaffyTree::new();
    let a = baseline_cell(&mut tree, 1, 1, Leaf { h: 0.0, baseline: None, top: 0.0 }, (50.0, 9.0), None);
    let b = baseline_cell(&mut tree, 1, 1, Leaf { h: 20.0, baseline: Some(16.0), top: 0.0 }, (2.0, 0.0), None);
    assert_eq!(baseline_rows(&mut tree, 1, &[a, b]), [59.0, 59.0]);
}
