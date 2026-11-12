//! RINCH: sizes a grid's rows as a CSS table sizes its rows ([`crate::Style::table_rows`]).
//!
//! CSS 2.2 §17.5.3 leaves the algorithm to the user agent; this is Blink's (LayoutNG
//! `table_layout_utils.cc`: `ComputeMinimumRowBlockSize` with its `RowBaselineTabulator`,
//! `DistributeRowspanCellToRows`, `DistributeTableBlockSizeToSections` and
//! `DistributeExcessBlockSizeToRows`), less percentage rows.
use super::types::{GridItem, GridTrack};
use crate::geometry::{AbstractAxis, Size};
use crate::style::{TableRow, TableRows};
use crate::tree::{LayoutPartialTree, LayoutPartialTreeExt};
use crate::util::sys::{f32_max, Vec};
use crate::util::MaybeMath;
use crate::CompactLength;

/// A length, not `auto`, a percentage or a `calc()`: a `height` that constrains a row.
fn is_length(value: CompactLength) -> bool {
    value.tag() == CompactLength::LENGTH_TAG
}

/// One table row while it is sized.
#[derive(Clone, Copy, Default)]
struct Row {
    /// Its height so far.
    size: f32,
    /// A row `height`, or a cell of span 1 with a `height`.
    constrained: bool,
    /// A cell spanning more than one row starts in it.
    rowspan_start: bool,
    /// The lowest first baseline of its cells aligned by the row's baseline, from its top.
    ascent: Option<f32>,
    /// How far the deepest of those of span 1 reaches below its baseline.
    descent: f32,
}

/// A cell spanning more than one row.
#[derive(Clone, Copy)]
struct Rowspan {
    /// Its first row.
    start: usize,
    /// How many rows it spans.
    span: usize,
    /// Its outer height.
    size: f32,
}

impl Rowspan {
    /// Blink's distribution order (`TableTypes::RowspanCell::operator<`): of two cells over the
    /// same rows the taller first, a cell inside another before it, else the higher one first.
    fn before(&self, other: &Rowspan) -> bool {
        let encloses = |outer: &Rowspan, inner: &Rowspan| {
            inner.start >= outer.start && inner.start + inner.span <= outer.start + outer.span
        };
        if self.start == other.start && self.span == other.span {
            return self.size > other.size;
        }
        if encloses(other, self) {
            return true;
        }
        if encloses(self, other) {
            return false;
        }
        self.start < other.start
    }
}

/// Size the row tracks of a table. `axis_tracks` are the rows (gutters included, sized here
/// from their fixed sizing functions), `other_axis_tracks` the columns, already sized.
/// `negative_implicit` is the number of row tracks before the first grid line. `table_height`
/// is the table's content-box height to share between its rows, when [`TableRows::sized`] and
/// the table has a definite height. With `wrapper_height` (the table's content-box min and max
/// heights) it is its wrapper's, a height its parent handed it: its captions are taken out of it,
/// and those limits clamp what is left.
#[allow(clippy::too_many_arguments)]
pub(super) fn size_table_rows<Tree: LayoutPartialTree>(
    tree: &mut Tree,
    table: &TableRows,
    axis_tracks: &mut [GridTrack],
    other_axis_tracks: &[GridTrack],
    items: &mut [GridItem],
    inner_node_size: Size<Option<f32>>,
    negative_implicit: usize,
    table_height: Option<f32>,
    wrapper_height: Option<(Option<f32>, Option<f32>)>,
) {
    // Fixed sizing functions: a row's `height`, and the gutters.
    for track in axis_tracks.iter_mut() {
        track.base_size = track
            .min_track_sizing_function
            .definite_value(inner_node_size.height, |val, basis| tree.calc(val, basis))
            .unwrap_or(0.0);
    }

    // Track `2k + 1` is row `k`; gutter `2k + 2` follows it.
    let count = axis_tracks.len() / 2;
    let kind = |k: usize| -> TableRow {
        k.checked_sub(negative_implicit).and_then(|i| table.rows.get(i).copied()).unwrap_or(TableRow {
            caption: false,
            section: 0,
            body: true,
        })
    };
    let mut rows: Vec<Row> = (0..count)
        .map(|k| {
            let track = &axis_tracks[2 * k + 1];
            Row {
                size: track.base_size,
                constrained: !kind(k).caption && is_length(track.min_track_sizing_function.0),
                rowspan_start: false,
                ascent: None,
                descent: 0.0,
            }
        })
        .collect();
    // A caption is outside the table box (CSS 2.2 §17.4), so no spacing separates it from the
    // rows or from another caption: the gutters on either side of a caption row are empty. The
    // table's padding (its outer spacing) is the embedder's to take out of the caption's margins.
    for k in 0..count.saturating_sub(1) {
        if kind(k).caption || kind(k + 1).caption {
            axis_tracks[2 * k + 2].base_size = 0.0;
        }
    }
    let gap = |k: usize, tracks: &[GridTrack]| tracks.get(2 * k + 2).map_or(0.0, |t| t.base_size);

    // How many cells aligned by its row's baseline start in each row. A cell of span 1 alone in
    // its row needs no room beyond its own height, so it is not measured for it (a
    // `display: table-cell` box is `vertical-align: baseline` by default, and the measure is a
    // layout of its own).
    let mut baseline_cells: Vec<u16> = (0..count).map(|_| 0).collect();
    for item in items.iter() {
        let start = (item.track_range_excluding_lines(AbstractAxis::Block).start.saturating_sub(1)) / 2;
        if item.table_row_baseline && start < count {
            baseline_cells[start] = baseline_cells[start].saturating_add(1);
        }
    }

    // Each cell of span 1 sets its row's least height; the spanning ones are kept for later.
    let mut rowspans: Vec<Rowspan> = Vec::new();
    for item in items.iter_mut() {
        let grid_area_size = item.grid_area_size(
            AbstractAxis::Block,
            axis_tracks,
            other_axis_tracks,
            inner_node_size,
            |track: &GridTrack, _| Some(track.base_size),
            &|val, basis| tree.calc(val, basis),
        );
        let available_space = grid_area_size.with(AbstractAxis::Block, None);
        let margins = item.margins_axis_sums_with_baseline_shims(available_space.width, tree).height;
        let size =
            item.max_content_contribution_cached(AbstractAxis::Block, tree, grid_area_size, available_space) + margins;
        let range = item.track_range_excluding_lines(AbstractAxis::Block);
        let start = (range.start.saturating_sub(1)) / 2;
        let span = (item.span(AbstractAxis::Block) as usize).max(1);
        if start >= count {
            continue;
        }
        // A cell aligned by its row's baseline lines up in its first row: the row is at least as
        // tall as the lowest baseline plus the deepest extent below it of a cell of span 1 (Blink's
        // `RowBaselineTabulator`; a spanning cell's extent is its rows' to share, as its height is).
        if item.table_row_baseline && !kind(start).caption && (span > 1 || baseline_cells[start] > 1) {
            if let Some((ascent, descent)) = item.table_row_baseline_extent(tree, grid_area_size) {
                let row = &mut rows[start];
                row.ascent = Some(row.ascent.map_or(ascent, |a| f32_max(a, ascent)));
                if span == 1 {
                    row.descent = f32_max(row.descent, descent);
                }
            }
        }
        if span == 1 {
            let row = &mut rows[start];
            row.size = f32_max(row.size, size);
            if !kind(start).caption && is_length(item.min_size.height.0) {
                row.constrained = true;
            }
        } else {
            let span = span.min(count - start);
            rows[start].rowspan_start = true;
            rowspans.push(Rowspan { start, span, size });
        }
    }

    for row in rows.iter_mut() {
        if let Some(ascent) = row.ascent {
            row.size = f32_max(row.size, ascent + row.descent);
        }
    }

    // A section's height is its rows' before the spanning cells grow them: Blink records
    // `section_block_size` in `ComputeSectionMinimumRowBlockSizes` before its rowspan step, and
    // shares a taller table's height between sections by those.
    let before_rowspans: Vec<f32> = rows.iter().map(|r| r.size).collect();

    // Spanning cells grow their rows, in Blink's order (a stable insertion sort: the order is
    // not a total one).
    let mut ordered: Vec<Rowspan> = Vec::with_capacity(rowspans.len());
    for cell in rowspans {
        let at = ordered.iter().position(|o| cell.before(o)).unwrap_or(ordered.len());
        ordered.insert(at, cell);
    }
    for cell in &ordered {
        let gaps: f32 = (cell.start..cell.start + cell.span - 1).map(|k| gap(k, axis_tracks)).sum();
        let ids: Vec<usize> = (cell.start..cell.start + cell.span).collect();
        distribute_excess(&mut rows, &ids, cell.size - gaps, true);
    }

    // A taller table's extra height goes to its body sections, then to their rows.
    if let Some(height) = table_height.filter(|_| table.sized) {
        let body: Vec<usize> = (0..count).filter(|&k| !kind(k).caption).collect();
        // Sections: runs of rows of one section, with the gaps inside them, as tall as their
        // rows were before the spanning cells grew them.
        let mut sections: Vec<(Vec<usize>, f32, bool)> = Vec::new();
        for &k in &body {
            let new = match sections.last() {
                Some((ids, _, _)) => kind(*ids.last().unwrap()).section != kind(k).section,
                None => true,
            };
            if new {
                sections.push((Vec::new(), 0.0, kind(k).body));
            } else {
                let last = sections.last().unwrap().0.last().copied().unwrap();
                sections.last_mut().unwrap().1 += gap(last, axis_tracks);
            }
            let section = sections.last_mut().unwrap();
            section.0.push(k);
            section.1 += before_rowspans[k];
        }
        let between: f32 =
            sections.iter().skip(1).map(|(ids, _, _)| gap(ids[0].saturating_sub(1), axis_tracks)).sum::<f32>();
        let least: f32 = sections.iter().map(|s| s.1).sum();
        // A caption is outside the table box: a height handed to the table's wrapper holds them
        // (no gutter separates them from the rows), and the table's own min and max heights apply
        // to what is left; its own `height` holds none.
        let height = match wrapper_height {
            Some((min, max)) => {
                let captions: f32 = (0..count).filter(|&k| kind(k).caption).map(|k| rows[k].size).sum();
                (height - captions).maybe_clamp(min, max)
            }
            None => height,
        };
        let extra = height - between - least;
        if extra > 0.0 && !sections.is_empty() {
            let any_body = sections.iter().any(|s| s.2);
            let grow: Vec<usize> = (0..sections.len()).filter(|&i| !any_body || sections[i].2).collect();
            let total: f32 = grow.iter().map(|&i| sections[i].1).sum();
            for &i in &grow {
                let share = if total > 0.0 { extra * sections[i].1 / total } else { extra / grow.len() as f32 };
                let (ids, size, _) = &sections[i];
                let inside: f32 = ids[..ids.len() - 1].iter().map(|&k| gap(k, axis_tracks)).sum();
                let target = size + share - inside;
                distribute_excess(&mut rows, ids, target, false);
            }
        }
    }

    for (k, row) in rows.iter().enumerate() {
        let track = &mut axis_tracks[2 * k + 1];
        track.base_size = row.size;
        track.growth_limit = row.size;
    }
    for track in axis_tracks.iter_mut().step_by(2) {
        track.growth_limit = track.base_size;
    }
}

/// Grow the rows `ids` (consecutive) to `target` in all, gaps excluded: Blink's
/// `DistributeExcessBlockSizeToRows`, less its percentage step.
fn distribute_excess(rows: &mut [Row], ids: &[usize], target: f32, rowspan: bool) {
    let Some(&first) = ids.first() else { return };
    let total: f32 = ids.iter().map(|&k| rows[k].size).sum();
    let extra = target - total;
    if extra <= 0.0 {
        return;
    }
    // Only a rowspan's distribution prefers the rows where another starts.
    let spanning: Vec<usize> =
        ids.iter().copied().filter(|&k| rowspan && k != first && rows[k].rowspan_start).collect();
    if !spanning.is_empty() {
        for &k in &spanning {
            rows[k].size += extra / spanning.len() as f32;
        }
        return;
    }
    let empty: Vec<usize> = ids.iter().copied().filter(|&k| rows[k].size == 0.0).collect();
    let non_empty: Vec<usize> = ids.iter().copied().filter(|&k| rows[k].size != 0.0).collect();
    let free: Vec<usize> = non_empty.iter().copied().filter(|&k| !rows[k].constrained).collect();
    if !free.is_empty() {
        let sum: f32 = free.iter().map(|&k| rows[k].size).sum();
        for &k in &free {
            rows[k].size += extra * rows[k].size / sum;
        }
        return;
    }
    if !empty.is_empty() {
        if rowspan {
            if empty.len() == ids.len() {
                rows[*empty.last().unwrap()].size += extra;
                return;
            }
        } else {
            // Every non-empty row is constrained here: the empty ones grow, the unconstrained
            // of them when there are any.
            let loose: Vec<usize> = empty.iter().copied().filter(|&k| !rows[k].constrained).collect();
            let grow = if loose.is_empty() { &empty } else { &loose };
            for &k in grow {
                rows[k].size = extra / grow.len() as f32;
            }
            return;
        }
    }
    for &k in &non_empty {
        rows[k].size += extra * rows[k].size / total;
    }
}
