//! `Style::stretch_past_max_width`: a stretched grid item fills its grid
//! area's width past its `max_size.width`, which still caps what the item
//! contributes to its tracks. The sizing CSS tables give a cell (Blink's
//! `CreateCellInlineConstraint` and `SetupTableCellConstraintSpaceBuilder`).
#![cfg(feature = "grid")]

use std::cell::Cell;

use taffy::compute_leaf_layout;
use taffy::prelude::*;

/// A text-like leaf: `words` boxes of `word` px on a line, 20px a line.
fn words(known: Size<Option<f32>>, avail: Size<AvailableSpace>, word: f32, words: f32) -> Size<f32> {
    let width = known.width.unwrap_or(match avail.width {
        AvailableSpace::Definite(w) => w.min(word * words),
        AvailableSpace::MinContent => word,
        AvailableSpace::MaxContent => word * words,
    });
    let per_line = (width / word).floor().max(1.0);
    Size { width, height: known.height.unwrap_or((words / per_line).ceil() * 20.0) }
}

/// A grid of `columns`, holding one row of leaves of the given max widths,
/// each two 70px words; `flag` sets `stretch_past_max_width` on every item.
/// Returns each item's size and the width its last measure was offered.
fn grid(columns: Vec<TrackSizingFunction>, width: Dimension, max: &[f32], flag: bool) -> Vec<(Size<f32>, f32)> {
    let mut tree: TaffyTree<usize> = TaffyTree::new();
    let items: Vec<NodeId> = max
        .iter()
        .enumerate()
        .map(|(i, &m)| {
            tree.new_leaf_with_context(
                Style {
                    max_size: Size { width: length(m), height: auto() },
                    stretch_past_max_width: flag,
                    ..Default::default()
                },
                i,
            )
            .unwrap()
        })
        .collect();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                size: Size { width, height: auto() },
                grid_template_columns: columns.into_iter().map(GridTemplateComponent::Single).collect(),
                ..Default::default()
            },
            &items,
        )
        .unwrap();
    let offered: Vec<Cell<f32>> = max.iter().map(|_| Cell::new(f32::NAN)).collect();
    tree.compute_layout_with_measure(
        root,
        Size { width: AvailableSpace::Definite(600.0), height: AvailableSpace::MaxContent },
        |inputs, _, ctx, style| {
            let i = *ctx.unwrap();
            compute_leaf_layout(
                inputs,
                style,
                |_, _| 0.0,
                |known, avail| {
                    if let AvailableSpace::Definite(w) = avail.width {
                        offered[i].set(known.width.unwrap_or(w));
                    }
                    words(known, avail, 70.0, 2.0)
                },
            )
        },
    )
    .unwrap();
    items.iter().enumerate().map(|(i, &n)| (tree.layout(n).unwrap().size, offered[i].get())).collect()
}

#[test]
fn a_flagged_item_fills_a_wider_track_and_its_text_wraps_at_it() {
    // One 300px track; the item's max width is 100, its content 140.
    let flagged = grid(vec![length(300.0)], auto(), &[100.0], true);
    assert_eq!(flagged[0].0, Size { width: 300.0, height: 20.0 });
    assert_eq!(flagged[0].1, 300.0);
    // Without the flag the item is clamped, and its two words take two lines.
    let plain = grid(vec![length(300.0)], auto(), &[100.0], false);
    assert_eq!(plain[0].0, Size { width: 100.0, height: 40.0 });
}

#[test]
fn a_flagged_items_max_width_still_caps_its_contribution() {
    // Two auto tracks in a shrink-to-fit grid: the first item asks at most
    // its 100 of its column (its content is 140), the second its 140.
    let flagged = grid(vec![auto(), auto()], Dimension::fit_content(), &[100.0, 1000.0], true);
    assert_eq!(flagged[0].0.width, 100.0);
    assert_eq!(flagged[1].0.width, 140.0);
}
