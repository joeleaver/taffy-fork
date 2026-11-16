//! A named line can resolve to an implicit line *before* the explicit grid
//! (css-grid-2 §8.3: a negative index past the named lines, or a name no
//! line carries, counts on into the implicit lines; rinch #2717). An item
//! placed there by its column and auto-placed by its row widens the implicit
//! grid at its start, as a numbered negative line does, so the auto-placement
//! cursor starts at that implicit start line and the item takes the first
//! free row. Taffy's grid size estimate ignored named lines, so the cursor
//! started at the explicit grid, read the item's column as behind it, and
//! moved the item down a row.
//!
//! Chrome 153 places `a` in row 1 in every case here (rinch-dom's
//! `grid_named_lines_2704_tests.rs`, row `k06`).
#![cfg(feature = "grid")]

use taffy::prelude::*;

/// The (x, y) of `a`, auto-placed by its row with `column`, beside `b` at
/// column 1, row 2, in a grid of three 100px columns and two 50px rows.
fn a_position(column: Line<GridPlacement>, names: Vec<Vec<String>>) -> (f32, f32) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let a = tree.new_leaf(Style { grid_column: column, ..Default::default() }).unwrap();
    let b = tree
        .new_leaf(Style {
            grid_column: Line { start: line(1), end: auto() },
            grid_row: Line { start: line(2), end: auto() },
            ..Default::default()
        })
        .unwrap();
    let grid = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                grid_template_columns: vec![length(100.0), length(100.0), length(100.0)],
                grid_template_rows: vec![length(50.0), length(50.0)],
                grid_template_column_names: names,
                ..Default::default()
            },
            &[a, b],
        )
        .unwrap();
    tree.compute_layout(grid, Size::MAX_CONTENT).unwrap();
    let l = tree.layout(a).unwrap();
    (l.location.x, l.location.y)
}

fn named(name: &str, idx: i16) -> GridPlacement {
    GridPlacement::NamedLine(name.into(), idx)
}

#[test]
fn a_negative_index_past_the_named_lines_keeps_the_first_free_row() {
    // `[m] 100px 100px [m] 100px`: `m -3` is the implicit line before line 1
    let names = vec![vec!["m".to_string()], vec![], vec!["m".to_string()], vec![]];
    assert_eq!(a_position(Line { start: named("m", -3), end: auto() }, names), (0.0, 0.0));
}

#[test]
fn a_missing_name_with_a_negative_index_keeps_the_first_free_row() {
    assert_eq!(a_position(Line { start: named("nosuch", -1), end: auto() }, Vec::new()), (0.0, 0.0));
}

#[test]
fn a_span_name_to_a_line_before_the_grid_keeps_the_first_free_row() {
    let column = Line { start: GridPlacement::NamedSpan("nosuch".into(), 1), end: line(-6) };
    assert_eq!(a_position(column, Vec::new()), (0.0, 0.0));
}

/// The control: a numbered negative line was always counted
#[test]
fn a_numbered_line_before_the_grid_keeps_the_first_free_row() {
    assert_eq!(a_position(Line { start: line(-5), end: auto() }, Vec::new()), (0.0, 0.0));
}
