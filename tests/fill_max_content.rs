//! `Style::fill_max_content`: a grid whose percentage width has no basis (a
//! size request) answers a max-content width of `FILL_MAX_CONTENT_WIDTH`, so
//! the float or shrink-to-fit box around it fills its room. Blink's quirk for
//! a `table-layout: fixed` table with a percentage width (rinch #2322); the
//! numbers are Chrome 156's for the same tables.
#![cfg(all(feature = "block_layout", feature = "float_layout", feature = "grid"))]

use taffy::prelude::*;
use taffy::Float;

fn block(width: Dimension) -> Style {
    Style { display: Display::Block, size: Size { width, height: auto() }, ..Default::default() }
}

fn float_left() -> Style {
    Style { display: Display::Block, float: Float::Left, ..Default::default() }
}

fn sized(width: f32, height: f32) -> Style {
    Style { display: Display::Block, size: Size { width: length(width), height: length(height) }, ..Default::default() }
}

/// A 560px block holding an auto-width float that holds a grid of
/// `grid_width` (one auto column, holding a 40x10 box) with
/// `fill_max_content` set to `flag`. Returns the float's and the grid's
/// widths.
fn float_around_grid(grid_width: Dimension, max_width: LengthPercentageAuto, flag: bool) -> (f32, f32) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let content = tree.new_leaf(sized(40.0, 10.0)).unwrap();
    let grid = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                size: Size { width: grid_width, height: auto() },
                max_size: Size { width: max_width, height: auto() },
                grid_template_columns: vec![GridTemplateComponent::Single(auto())],
                fill_max_content: flag,
                ..Default::default()
            },
            &[content],
        )
        .unwrap();
    let float = tree.new_with_children(float_left(), &[grid]).unwrap();
    let root = tree.new_with_children(block(length(560.0)), &[float]).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    (tree.layout(float).unwrap().size.width, tree.layout(grid).unwrap().size.width)
}

#[test]
fn a_grid_that_fills_its_max_content_makes_the_float_around_it_fill_its_room() {
    // Chrome: the float is 560 and the 70% table in it 392.
    assert_eq!(float_around_grid(percent(0.7), auto(), true), (560.0, 392.0));
    // Without the flag the float is its content's width, and 70% of it.
    assert_eq!(float_around_grid(percent(0.7), auto(), false), (40.0, 28.0));
}

#[test]
fn a_grid_that_fills_its_max_content_is_clamped_by_its_max_width() {
    // Chrome: `max-width: 310px` on the table; the float is 310, the table
    // 70% of it.
    assert_eq!(float_around_grid(percent(0.7), length(310.0), true), (310.0, 217.0));
}

#[test]
fn a_grid_with_a_length_width_does_not_fill() {
    assert_eq!(float_around_grid(length(200.0), auto(), true), (200.0, 200.0));
}
