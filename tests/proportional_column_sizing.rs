//! `Style::proportional_column_sizing`: the Maximise Tracks step shares
//! the free space between the columns in proportion to each one's room to
//! grow, so every column reaches its max-content width at once. The sizing
//! CSS tables give `auto` columns between their min-content and max-content
//! widths (CSS 2.1 §17.5.2.2; Blink's
//! `DistributeInlineSizeToComputedInlineSizeAuto`).
#![cfg(feature = "grid")]

use taffy::prelude::*;

/// A text-like leaf of `words` 40px words: min-content 40, max-content
/// `40 * words`.
fn measure(known: Size<Option<f32>>, avail: Size<AvailableSpace>, words: f32) -> Size<f32> {
    // An empty leaf (no words) is 0 wide.
    let min = f32::min(40.0, 40.0 * words);
    let width = known.width.unwrap_or(match avail.width {
        AvailableSpace::Definite(w) => w.clamp(min, 40.0 * words),
        AvailableSpace::MinContent => min,
        AvailableSpace::MaxContent => 40.0 * words,
    });
    let per_line = (width / 40.0).floor().max(1.0);
    Size { width, height: known.height.unwrap_or((words / per_line).ceil() * 10.0) }
}

/// One row of `auto` columns holding leaves of `words` words, in a grid that
/// fits its content into `avail`. Returns the column widths.
fn columns(words: &[f32], avail: f32, flag: bool) -> Vec<f32> {
    columns_with_span(words, None, avail, flag)
}

/// [`columns`], with a second row of one leaf of `span.1` words spanning the
/// columns from `span.0` to the last.
fn columns_with_span(words: &[f32], spanning: Option<(i16, f32)>, avail: f32, flag: bool) -> Vec<f32> {
    let tracks: Vec<TrackSizingFunction> = words.iter().map(|_| auto()).collect();
    grid_of(&tracks, words, spanning, avail, flag)
}

/// [`columns_with_span`] with the columns' own track sizing functions.
fn grid_of(
    tracks: &[TrackSizingFunction],
    words: &[f32],
    spanning: Option<(i16, f32)>,
    avail: f32,
    flag: bool,
) -> Vec<f32> {
    let mut tree: TaffyTree<f32> = TaffyTree::new();
    tree.disable_rounding();
    let mut items: Vec<NodeId> =
        words.iter().map(|&w| tree.new_leaf_with_context(Style::default(), w).unwrap()).collect();
    let cells = items.len();
    if let Some((from, w)) = spanning {
        let style = Style {
            grid_row: Line { start: line(2), end: line(3) },
            grid_column: Line { start: line(from), end: line(cells as i16 + 1) },
            ..Default::default()
        };
        items.push(tree.new_leaf_with_context(style, w).unwrap());
    }
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                size: Size { width: Dimension::fit_content(), height: auto() },
                grid_template_columns: tracks.iter().map(|t| GridTemplateComponent::Single(*t)).collect(),
                proportional_column_sizing: flag,
                ..Default::default()
            },
            &items,
        )
        .unwrap();
    tree.compute_layout_with_measure(
        root,
        Size { width: AvailableSpace::Definite(avail), height: AvailableSpace::MaxContent },
        |inputs, _, ctx, _| {
            taffy::LayoutOutput::from_outer_size(measure(
                inputs.known_dimensions,
                inputs.available_space,
                *ctx.unwrap(),
            ))
        },
    )
    .unwrap();
    items[..cells].iter().map(|&i| tree.layout(i).unwrap().size.width).collect()
}

#[test]
fn columns_share_the_free_space_by_their_room_to_grow() {
    // min 40 each (160), max 800 + 200 + 80 + 400 = 1480, 600 to fill: each
    // column is 40 + (max - 40) * 440 / 1320. Chrome 156 lays a table of
    // these columns out at 293.3, 93.3, 53.3, 160.
    let got = columns(&[20.0, 5.0, 2.0, 10.0], 600.0, true);
    let want = [40.0 + 760.0 / 3.0, 40.0 + 160.0 / 3.0, 40.0 + 40.0 / 3.0, 160.0];
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() < 0.05, "{got:?} vs {want:?}");
    }
}

#[test]
fn a_grid_shares_equally() {
    // The same grid without the flag: 110 each, then the narrow column stops at
    // 80 and the other three share its remaining 70.
    let got = columns(&[20.0, 5.0, 2.0, 10.0], 600.0, false);
    let third = 150.0 + 70.0 / 3.0;
    for (g, w) in got.iter().zip([third, third, 80.0, third]) {
        assert!((g - w).abs() < 0.05, "{got:?}");
    }
}

#[test]
fn content_that_fits_is_at_its_max_content_either_way() {
    for flag in [false, true] {
        assert_eq!(columns(&[3.0, 2.0, 1.0, 2.0], 600.0, flag), vec![120.0, 80.0, 40.0, 80.0]);
    }
}

#[test]
fn a_spanning_items_max_content_goes_to_its_columns_by_their_own() {
    // Columns of max 120 and 40 (min 40 each) under an item of max 320
    // spanning both: its 160 beyond them goes 3:1, to 240 and 80, which fit.
    // Equal shares would give 200 and 120.
    assert_eq!(columns_with_span(&[3.0, 1.0], Some((1, 8.0)), 600.0, true), vec![240.0, 80.0]);
    assert_eq!(columns_with_span(&[3.0, 1.0], Some((1, 8.0)), 600.0, false), vec![200.0, 120.0]);
}

#[test]
fn a_fixed_width_column_reaches_its_width_before_the_auto_columns_grow() {
    // `minmax(min-content, 200px)` (a table column with `width: 200px`, one
    // word) beside an auto column of 10 words, into 300: the fixed column
    // takes its 160 first (Blink's "specified guess"), the auto one the
    // remaining 60. Chrome 156: 200 / 100.
    let fixed = minmax(min_content(), length(200.0));
    let got = grid_of(&[fixed, auto()], &[1.0, 10.0], None, 300.0, true);
    assert_eq!(got, vec![200.0, 100.0]);
    // Two 20px spacers around an auto column of 50 words, into 600: 20/560/20.
    let spacer = minmax(min_content(), length(20.0));
    let got = grid_of(&[spacer, auto(), spacer], &[0.0, 50.0, 0.0], None, 600.0, true);
    assert_eq!(got, vec![20.0, 560.0, 20.0]);
}
