//! `Style::table_rows`: a grid's rows sized as a CSS table sizes its rows
//! (rinch #2312). The numbers are Chrome 156's for the same tables (cells of
//! fixed-size boxes, `padding: 0`), unrounded.
#![cfg(all(feature = "block_layout", feature = "grid"))]

use taffy::prelude::*;
use taffy::{TableRow, TableRows};

/// A cell of one column at row line `row`, spanning `span` rows, holding a
/// 10px-wide box `h` tall; `min_h` is a cell `height`.
fn cell(tree: &mut TaffyTree<()>, row: i16, span: u16, h: f32, min_h: Option<f32>) -> NodeId {
    let content = tree
        .new_leaf(Style {
            display: Display::Block,
            size: Size { width: length(10.0), height: length(h) },
            ..Default::default()
        })
        .unwrap();
    tree.new_with_children(
        Style {
            display: Display::Block,
            grid_row: Line { start: line(row), end: taffy::style_helpers::span(span) },
            grid_column: Line { start: line(1), end: taffy::style_helpers::span(1) },
            min_size: Size { width: auto(), height: min_h.map_or(auto(), length) },
            ..Default::default()
        },
        &[content],
    )
    .unwrap()
}

/// A table of the row tracks `rows` (a declared height, or none), `gap`
/// between rows, `height`, the given row kinds, holding `cells`.
fn table(
    tree: &mut TaffyTree<()>,
    rows: &[Option<f32>],
    kinds: Vec<TableRow>,
    gap: f32,
    height: Dimension,
    cells: &[NodeId],
) -> NodeId {
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                size: Size { width: auto(), height },
                gap: Size { width: length(0.0), height: length(gap) },
                padding: Rect { left: length(0.0), right: length(0.0), top: length(gap), bottom: length(gap) },
                grid_template_columns: vec![GridTemplateComponent::Single(auto())],
                grid_template_rows: rows
                    .iter()
                    .map(|h| {
                        GridTemplateComponent::Single(match h {
                            Some(h) => minmax(length(*h), auto()),
                            None => auto(),
                        })
                    })
                    .collect(),
                table_rows: Some(TableRows { rows: kinds, sized: true }),
                ..Default::default()
            },
            cells,
        )
        .unwrap();
    let outer = tree.new_with_children(Style { display: Display::Block, ..Default::default() }, &[root]).unwrap();
    tree.compute_layout(outer, Size::MAX_CONTENT).unwrap();
    root
}

fn body(n: usize) -> Vec<TableRow> {
    vec![TableRow { caption: false, section: 0, body: true }; n]
}

fn heights(tree: &TaffyTree<()>, nodes: &[NodeId]) -> Vec<f32> {
    nodes.iter().map(|&n| tree.unrounded_layout(n).size.height).collect()
}

fn close(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| (a - b).abs() < 0.02)
}

#[test]
fn a_rowspan_grows_its_rows_in_proportion_to_their_heights() {
    // Rows of 30 and 20 under a 120px rowspan: 72 and 48.
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let s = cell(&mut tree, 1, 2, 120.0, None);
    let b = cell(&mut tree, 1, 1, 0.0, None);
    let c = cell(&mut tree, 2, 1, 0.0, None);
    table(&mut tree, &[Some(30.0), Some(20.0)], body(2), 0.0, auto(), &[s, b, c]);
    let got = heights(&tree, &[b, c]);
    assert!(close(&got, &[72.0, 48.0]), "{got:?}");
}

#[test]
fn a_taller_table_shares_by_content_and_grows_its_body_only() {
    // 2px spacing, a 200px table: a header row of 20, body rows of 10 and
    // 30, a footer row of 15. Chrome: 20, 38.75, 116.25, 15.
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let cells: Vec<NodeId> =
        [20.0, 10.0, 30.0, 15.0].iter().enumerate().map(|(i, &h)| cell(&mut tree, i as i16 + 1, 1, h, None)).collect();
    let kinds = vec![
        TableRow { caption: false, section: 0, body: false },
        TableRow { caption: false, section: 1, body: true },
        TableRow { caption: false, section: 1, body: true },
        TableRow { caption: false, section: 2, body: false },
    ];
    let t = table(&mut tree, &[None; 4], kinds, 2.0, length(200.0), &cells);
    let got = heights(&tree, &cells);
    assert!(close(&got, &[20.0, 38.75, 116.25, 15.0]), "{got:?}");
    assert_eq!(tree.unrounded_layout(t).size.height, 200.0);
}

#[test]
fn a_caption_takes_none_of_the_height_and_a_short_table_grows() {
    // A 20px caption, a 30px row and an auto row in a 200px table: the rows
    // are 30 and 170, the table 220.
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let cap = cell(&mut tree, 1, 1, 20.0, None);
    let a = cell(&mut tree, 2, 1, 0.0, None);
    let b = cell(&mut tree, 3, 1, 0.0, None);
    let mut kinds = body(3);
    kinds[0].caption = true;
    let t = table(&mut tree, &[None, Some(30.0), None], kinds.clone(), 0.0, length(200.0), &[cap, a, b]);
    assert!(close(&heights(&tree, &[cap, a, b]), &[20.0, 30.0, 170.0]));
    assert_eq!(tree.unrounded_layout(t).size.height, 220.0);

    // Two 30px rows in a 50px table: the table is 60.
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let a = cell(&mut tree, 1, 1, 0.0, None);
    let b = cell(&mut tree, 2, 1, 0.0, None);
    let t = table(&mut tree, &[Some(30.0), Some(30.0)], body(2), 0.0, length(50.0), &[a, b]);
    assert_eq!(tree.unrounded_layout(t).size.height, 60.0);
}

#[test]
fn a_cells_height_constrains_its_row() {
    // 2px spacing, a 150px table; a cell `height: 30px`, then rows of 10 and
    // 20: Chrome 30, 37.33, 74.67.
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let a = cell(&mut tree, 1, 1, 0.0, Some(30.0));
    let b = cell(&mut tree, 2, 1, 10.0, None);
    let c = cell(&mut tree, 3, 1, 20.0, None);
    table(&mut tree, &[None; 3], body(3), 2.0, length(150.0), &[a, b, c]);
    let got = heights(&tree, &[a, b, c]);
    assert!(close(&got, &[30.0, 37.328, 74.672]), "{got:?}");
}

#[test]
fn sections_share_a_taller_table_by_their_heights_before_rowspans() {
    // A 200px table. The first tbody: rows of 10 and 20 and a 60px cell
    // spanning both (which grows them to 20 and 40); the second: a row of
    // 10. The sections share the 160px by 30 and 10, their rows' heights
    // before the spanning cell: Chrome 50, 100 | 50 (by 60 and 10 it would
    // be 57, 114 | 29).
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let a = cell(&mut tree, 1, 1, 10.0, None);
    let b = cell(&mut tree, 2, 1, 20.0, None);
    let c = cell(&mut tree, 3, 1, 10.0, None);
    let s = cell(&mut tree, 1, 2, 60.0, None);
    tree.set_style(s, {
        let mut st = tree.style(s).unwrap().clone();
        st.grid_column = Line { start: line(2), end: taffy::style_helpers::span(1) };
        st
    })
    .unwrap();
    let kinds = vec![
        TableRow { caption: false, section: 0, body: true },
        TableRow { caption: false, section: 0, body: true },
        TableRow { caption: false, section: 1, body: true },
    ];
    table(&mut tree, &[None; 3], kinds, 0.0, length(200.0), &[a, b, c, s]);
    let got = heights(&tree, &[a, b, c, s]);
    assert!(close(&got, &[50.0, 100.0, 50.0, 150.0]), "{got:?}");
}
