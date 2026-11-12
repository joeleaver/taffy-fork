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

/// A leaf of `n` words each `px` wide.
fn words_of(known: Size<Option<f32>>, avail: Size<AvailableSpace>, (n, px): (f32, f32)) -> Size<f32> {
    let min = f32::min(px, px * n);
    let width = known.width.unwrap_or(match avail.width {
        AvailableSpace::Definite(w) => w.clamp(min, px * n),
        AvailableSpace::MinContent => min,
        AvailableSpace::MaxContent => px * n,
    });
    let per_line = (width / px).floor().max(1.0);
    Size { width, height: known.height.unwrap_or((n / per_line).ceil() * 10.0) }
}

/// A grid `width` wide (`fit-content` when `None`) with a 4px column gap, of
/// one row of leaves `(words, word px)` in columns `tracks`, and an optional
/// second row of one leaf spanning every column. Returns the first row's
/// widths and the grid's width.
fn table(
    width: Option<f32>,
    tracks: &[TrackSizingFunction],
    cells: &[(f32, f32)],
    spanning: Option<(f32, f32)>,
) -> (Vec<f32>, f32) {
    let mut tree: TaffyTree<(f32, f32)> = TaffyTree::new();
    tree.disable_rounding();
    let mut items: Vec<NodeId> =
        cells.iter().map(|&c| tree.new_leaf_with_context(Style::default(), c).unwrap()).collect();
    let n = items.len();
    if let Some(c) = spanning {
        let style = Style {
            grid_row: Line { start: line(2), end: line(3) },
            grid_column: Line { start: line(1), end: line(n as i16 + 1) },
            ..Default::default()
        };
        items.push(tree.new_leaf_with_context(style, c).unwrap());
    }
    let root = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                size: Size { width: width.map(length).unwrap_or(Dimension::fit_content()), height: auto() },
                gap: Size { width: length(4.0), height: length(0.0) },
                grid_template_columns: tracks.iter().map(|t| GridTemplateComponent::Single(*t)).collect(),
                proportional_column_sizing: true,
                ..Default::default()
            },
            &items,
        )
        .unwrap();
    tree.compute_layout_with_measure(
        root,
        Size { width: AvailableSpace::Definite(1000.0), height: AvailableSpace::MaxContent },
        |inputs, _, ctx, _| {
            taffy::LayoutOutput::from_outer_size(words_of(
                inputs.known_dimensions,
                inputs.available_space,
                *ctx.unwrap(),
            ))
        },
    )
    .unwrap();
    let cols = items[..n].iter().map(|&i| tree.layout(i).unwrap().size.width).collect();
    (cols, tree.layout(root).unwrap().size.width)
}

fn assert_near(got: &[f32], want: &[f32]) {
    assert_eq!(got.len(), want.len());
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() < 0.05, "{got:?} vs {want:?}");
    }
}

fn mc_auto() -> TrackSizingFunction {
    minmax(min_content(), auto())
}

#[test]
fn a_grid_of_a_given_width_shares_it_by_each_columns_room_to_grow() {
    // A table with a width of its own, below its columns' max-content: 592
    // past the gaps, min 40 each, max 800 / 200 / 80 / 400: r = 432 / 1320.
    let (got, w) = table(
        Some(604.0),
        &[mc_auto(), mc_auto(), mc_auto(), mc_auto()],
        &[(20.0, 40.0), (5.0, 40.0), (2.0, 40.0), (10.0, 40.0)],
        None,
    );
    let r = 432.0 / 1320.0;
    assert_near(&got, &[40.0 + 760.0 * r, 40.0 + 160.0 * r, 40.0 + 40.0 * r, 40.0 + 360.0 * r]);
    assert_eq!(w, 604.0);
}

#[test]
fn fixed_columns_narrower_than_their_widths_grow_from_their_minimums() {
    // `<td width=390>` (min 0), `<td width=305>` (min 100) in 604: 600 past the
    // gap, r = 500 / 595 (Chrome 156's interpolation for #2306).
    let (got, _) = table(
        Some(604.0),
        &[minmax(min_content(), length(390.0)), minmax(min_content(), length(305.0))],
        &[(0.0, 40.0), (1.0, 100.0)],
        None,
    );
    let r = 500.0 / 595.0;
    assert_near(&got, &[390.0 * r, 100.0 + 205.0 * r]);
}

#[test]
fn the_excess_past_max_content_goes_to_the_auto_columns_by_their_max_content() {
    // A 100px fixed column and auto columns of max 120 / 80 in 704 (700 past
    // the gaps): the auto columns take the 400 excess 3:2.
    let (got, _) = table(
        Some(708.0),
        &[minmax(min_content(), length(100.0)), mc_auto(), mc_auto()],
        &[(1.0, 40.0), (3.0, 40.0), (2.0, 40.0)],
        None,
    );
    assert_near(&got, &[100.0, 360.0, 240.0]);
    // With fixed columns only, they take it by their widths.
    let f = |p| minmax(min_content(), length(p));
    let (got, _) = table(Some(708.0), &[f(100.0), f(200.0), f(50.0)], &[(1.0, 40.0), (1.0, 40.0), (1.0, 40.0)], None);
    assert_near(&got, &[200.0, 400.0, 100.0]);
}

#[test]
fn a_spanning_items_min_content_goes_to_its_columns_by_their_max_content() {
    // Columns of min 40 / 40 and max 120 / 40 under one unbreakable 300px
    // leaf (296 past the gap): the 136 past their max-content goes 3:1.
    let (got, w) = table(None, &[auto(), auto()], &[(3.0, 40.0), (1.0, 40.0)], Some((1.0, 300.0)));
    assert_near(&got, &[120.0 + 102.0, 40.0 + 34.0]);
    assert_eq!(w, 300.0);
}

#[test]
fn a_grid_of_a_given_width_widens_to_its_columns_minimum() {
    // Minimums of 300 and 50 and a 4px gap past a declared 100.
    let (got, w) = table(Some(100.0), &[mc_auto(), mc_auto()], &[(1.0, 300.0), (1.0, 50.0)], None);
    assert_eq!(got, vec![300.0, 50.0]);
    assert_eq!(w, 354.0);
}

#[test]
fn empty_auto_columns_share_the_excess_equally_before_the_declared_ones() {
    // A 100px column beside an empty auto column (an email's filler cell
    // beside its logo), 404 past the gap: the filler takes the 304. Chrome
    // 156: 108 / 280 for `<td width=100>` (padding 4) + an empty `<td>` in a
    // 400px table. Two empty columns share it equally.
    let fixed = minmax(min_content(), length(100.0));
    let (got, _) = table(Some(408.0), &[fixed, mc_auto()], &[(1.0, 40.0), (0.0, 40.0)], None);
    assert_near(&got, &[100.0, 304.0]);
    let (got, _) = table(Some(408.0), &[fixed, mc_auto(), mc_auto()], &[(1.0, 40.0), (0.0, 40.0), (0.0, 40.0)], None);
    assert_near(&got, &[100.0, 150.0, 150.0]);
    // An empty auto column beside one with content gets none of it.
    let (got, _) = table(Some(408.0), &[mc_auto(), mc_auto()], &[(1.0, 40.0), (0.0, 40.0)], None);
    assert_near(&got, &[404.0, 0.0]);
}

#[test]
fn the_excess_goes_to_a_declared_column_before_a_percentage_one() {
    // `minmax(min-content, 20%)` and a 100px column in a 808px grid: the
    // percentage column stops at 20% of the width less the 4px gutter (804:
    // 160.8, a table's basis leaves out every spacing, rinch #2333), the
    // declared one takes the rest of the 804. Chrome 156 for the same in an
    // 800px table with a 4px spacing (basis 800 - 3 x 4 = 788): 157.59 /
    // 630.41. Before #2333 the basis was the grid's 808, gap included: 161.6.
    let (got, _) = table(
        Some(808.0),
        &[minmax(min_content(), percent(0.2)), minmax(min_content(), length(100.0))],
        &[(1.0, 40.0), (1.0, 40.0)],
        None,
    );
    assert_near(&got, &[160.8, 643.2]);
}
