//! Code for resolving name grid lines and areas

use crate::{
    CheapCloneStr, GenericGridTemplateComponent, GenericRepetition as _, GridAreaAxis, GridAreaEnd, GridContainerStyle,
    GridPlacement, GridTemplateArea, Line, NonNamedGridPlacement, RepetitionCount,
};
use core::{borrow::Borrow, cmp::Ordering, fmt::Debug};

use super::{GridLine, MAX_GRID_TRACKS};
#[cfg(feature = "detailed_layout_info")]
use crate::geometry::AbsoluteAxis;
#[cfg(feature = "detailed_layout_info")]
use crate::sys::DefaultCheapStr;
// use alloc::fmt::format;
use crate::sys::{format, Map, Vec};
use smallvec::{smallvec, SmallVec};

/// Wrap an `AsRef<str>` type with a type which implements Hash by first
/// deferring to the underlying `&str`'s implementation of Hash.
#[derive(Debug, Clone)]
pub(crate) struct StrHasher<T: CheapCloneStr>(pub T);
impl<T: CheapCloneStr> PartialOrd for StrHasher<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<T: CheapCloneStr> Ord for StrHasher<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.as_ref().cmp(other.0.as_ref())
    }
}
impl<T: CheapCloneStr> PartialEq for StrHasher<T> {
    fn eq(&self, other: &Self) -> bool {
        other.0.as_ref() == self.0.as_ref()
    }
}
impl<T: CheapCloneStr> Eq for StrHasher<T> {}
#[cfg(feature = "std")]
impl<T: CheapCloneStr> std::hash::Hash for StrHasher<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_ref().hash(state)
    }
}
impl<T: CheapCloneStr> Borrow<str> for StrHasher<T> {
    fn borrow(&self) -> &str {
        self.0.as_ref()
    }
}

/// The one-indexed line positions of a single grid line name. Inline capacity of 4 keeps the
/// type the same size as `Vec<u32>` (24 bytes) while avoiding a heap allocation for names
/// mapping to at most 4 lines (the common case)
pub(crate) type LinePositions = SmallVec<[u32; 4]>;

/// Map from a grid line name to its one-indexed line positions
type NamedGridLinesMap<S> = Map<StrHasher<S>, LinePositions>;

/// Resolver for named placements in one grid axis
struct NamedLineResolverAxis<'a, S: CheapCloneStr> {
    /// Named lines and their one-indexed positions
    lines: &'a NamedGridLinesMap<S>,
    /// Number of explicit tracks in this axis
    explicit_track_count: u16,
}

/// Resolver that takes grid lines names and area names as input and can then be used to
/// resolve line names of grid placement properties into line numbers.
pub(crate) struct NamedLineResolver<S: CheapCloneStr> {
    /// Map of row line names to line numbers. Each line name may correspond to multiple lines
    /// so we store a `SmallVec`
    row_lines: NamedGridLinesMap<S>,
    /// Map of column line names to line numbers. Each line name may correspond to multiple lines
    /// so we store a `SmallVec`
    column_lines: NamedGridLinesMap<S>,
    /// Map of area names to area definitions (start and end lines numbers in each axis)
    areas: Map<StrHasher<S>, GridTemplateArea<S>>,
    /// Number of columns implied by grid area definitions
    area_column_count: u16,
    /// Number of rows implied by grid area definitions
    area_row_count: u16,
    /// The number of explicit columns in the grid. This is an *input* to the `NamedLineResolver` and is
    /// used when computing the fallback line when a non-existent named line is specified.
    explicit_column_count: u16,
    /// The number of explicit rows in the grid. This is an *input* to the `NamedLineResolver` and is
    /// used when computing the fallback line when a non-existent named line is specified.
    explicit_row_count: u16,
    /// The (1-indexed line number, name) pairs of every named column line, in source order
    /// (template names before `grid-template-areas`-generated names)
    #[cfg(feature = "detailed_layout_info")]
    column_line_name_pairs: Vec<(u32, S)>,
    /// The (1-indexed line number, name) pairs of every named row line, in source order
    /// (template names before `grid-template-areas`-generated names)
    #[cfg(feature = "detailed_layout_info")]
    row_line_name_pairs: Vec<(u32, S)>,
}

/// Utility function to create or update an entry in a line name map
fn upsert_line_name_map<S: CheapCloneStr>(map: &mut NamedGridLinesMap<S>, key: S, value: u32) {
    map.entry(StrHasher(key)).and_modify(|lines| lines.push(value)).or_insert_with(|| smallvec![value]);
}

impl<S: CheapCloneStr> NamedLineResolverAxis<'_, S> {
    /// Resolve named lines and spans into numeric placements
    fn resolve_line_names(&self, line: &Line<GridPlacement<S>>) -> Line<NonNamedGridPlacement> {
        let start_holder;
        let start_line_resolved = if let GridPlacement::NamedLine(name, idx) = &line.start {
            start_holder = GridPlacement::Line(self.find_line_index(name, *idx as i32, GridAreaEnd::Start));
            &start_holder
        } else {
            &line.start
        };

        let end_holder;
        let end_line_resolved = if let GridPlacement::NamedLine(name, idx) = &line.end {
            end_holder = GridPlacement::Line(self.find_line_index(name, *idx as i32, GridAreaEnd::End));
            &end_holder
        } else {
            &line.end
        };

        // If both the *-start and *-end values of its grid-placement properties specify a line, its grid span is implicit.
        // If it has an explicit span value, its grid span is explicit.
        // Otherwise, its grid span is automatic:
        //   - if it is subgridded in that axis, its grid span is determined from its <line-name-list>;
        //   - otherwise its grid span is 1.
        //
        // <https://drafts.csswg.org/css-grid-2/#grid-span>
        match (&start_line_resolved, &end_line_resolved) {
            // `<line> / span <name> N`: the Nth line with the name after the start line
            (GridPlacement::Line(start_line), GridPlacement::NamedSpan(name, idx)) => {
                let end_line = self.find_span_line(name, *idx, GridAreaEnd::End, *start_line);
                Line { start: NonNamedGridPlacement::Line(*start_line), end: NonNamedGridPlacement::Line(end_line) }
            }
            // `span <name> N / <line>`: the Nth line with the name before the end line, counting
            // back from it (the nearest is the first)
            (GridPlacement::NamedSpan(name, idx), GridPlacement::Line(end_line)) => {
                let start_line = self.find_span_line(name, *idx, GridAreaEnd::Start, *end_line);
                Line { start: NonNamedGridPlacement::Line(start_line), end: NonNamedGridPlacement::Line(*end_line) }
            }
            (start, end) => Line {
                start: match start {
                    GridPlacement::Auto => NonNamedGridPlacement::Auto,
                    GridPlacement::Line(grid_line) => NonNamedGridPlacement::Line(*grid_line),
                    GridPlacement::Span(span) => NonNamedGridPlacement::Span(*span),
                    GridPlacement::NamedSpan(_, _) => NonNamedGridPlacement::Span(1),
                    _ => unreachable!(),
                },
                end: match end {
                    GridPlacement::Auto => NonNamedGridPlacement::Auto,
                    GridPlacement::Line(grid_line) => NonNamedGridPlacement::Line(*grid_line),
                    GridPlacement::Span(span) => NonNamedGridPlacement::Span(*span),
                    GridPlacement::NamedSpan(_, _) => NonNamedGridPlacement::Span(1),
                    _ => unreachable!(),
                },
            },
        }
    }

    /// The sorted one-indexed lines that carry the passed name: the lines named so, or failing
    /// that the implicit `<name>-start` / `<name>-end` lines of an area. Empty if neither exists.
    fn lines_named(&self, name: &S, end: GridAreaEnd) -> &[u32] {
        let name = name.as_ref();
        if let Some(lines) = self.lines.get(name) {
            return lines;
        }
        // TODO: eliminate string allocations
        let implicit_name = match end {
            GridAreaEnd::Start => format!("{name}-start"),
            GridAreaEnd::End => format!("{name}-end"),
        };
        self.lines.get(&*implicit_name).map(|lines| &lines[..]).unwrap_or(&[])
    }

    /// The position of a grid line counted from the first explicit line (`1`), so that the
    /// implicit lines before the explicit grid are `0`, `-1`, ...
    fn line_position(&self, line: GridLine) -> i64 {
        let line = line.as_i16() as i64;
        if line < 0 {
            // `-1` is the last explicit line, `explicit_track_count + 1`
            self.explicit_track_count as i64 + 2 + line
        } else {
            line
        }
    }

    /// The grid line at a position counted as in [`Self::line_position`]: lines before the
    /// explicit grid (position `0` and below) are expressed as negative lines
    fn grid_line_at(&self, position: i64) -> GridLine {
        let line = if position >= 1 { position } else { position - (self.explicit_track_count as i64 + 2) };
        GridLine::from(line.clamp(i16::MIN as i64, i16::MAX as i64) as i16)
    }

    /// Resolve the grid line for `<name> N` (`<name>` alone is `<name> 1`).
    ///
    /// A positive N counts the lines with the name from the start of the explicit grid, a
    /// negative one counts them back from its end. If fewer lines carry the name, all implicit
    /// lines are assumed to carry it: the count goes on into the implicit lines after the
    /// explicit grid (N > 0) or before it (N < 0). A name no line carries is that case with no
    /// named lines at all.
    ///
    /// <https://drafts.csswg.org/css-grid-2/#line-placement>
    fn find_line_index(&self, name: &S, idx: i32, end: GridAreaEnd) -> GridLine {
        // An index of 0 is used to represent "no index specified".
        let idx = if idx == 0 { 1 } else { idx as i64 };
        let lines = self.lines_named(name, end);
        let count = lines.len() as i64;
        let explicit_line_count = self.explicit_track_count as i64 + 1;
        let position = if idx > 0 {
            match lines.get(idx as usize - 1) {
                Some(&line) => line as i64,
                None => explicit_line_count + (idx - count),
            }
        } else if -idx <= count {
            lines[(count + idx) as usize] as i64
        } else {
            1 - (-idx - count)
        };
        self.grid_line_at(position)
    }

    /// Resolve the grid line that `span <name> N` reaches from the passed line: the Nth line
    /// with the name after `from` (`end` is [`GridAreaEnd::End`]) or before it
    /// ([`GridAreaEnd::Start`]), counting outward from `from`.
    ///
    /// If fewer lines carry the name, the implicit lines on the side of the explicit grid the
    /// search goes towards are assumed to carry it (those beyond `from`, when `from` is itself
    /// already past the explicit grid).
    ///
    /// <https://drafts.csswg.org/css-grid-2/#grid-placement-span-int>
    fn find_span_line(&self, name: &S, idx: u16, end: GridAreaEnd, from: GridLine) -> GridLine {
        let idx = idx.max(1) as i64;
        let from = self.line_position(from);
        let lines = self.lines_named(name, end);
        let position = match end {
            GridAreaEnd::End => {
                let after = &lines[lines.partition_point(|&line| line as i64 <= from)..];
                match after.get(idx as usize - 1) {
                    Some(&line) => line as i64,
                    None => from.max(self.explicit_track_count as i64 + 1) + (idx - after.len() as i64),
                }
            }
            GridAreaEnd::Start => {
                let before = &lines[..lines.partition_point(|&line| (line as i64) < from)];
                match before.len().checked_sub(idx as usize) {
                    Some(i) => before[i] as i64,
                    None => from.min(1) - (idx - before.len() as i64),
                }
            }
        };
        self.grid_line_at(position)
    }
}

impl<S: CheapCloneStr> NamedLineResolver<S> {
    /// Create and initialise a new `NamedLineResolver`
    pub(crate) fn new(
        style: &impl GridContainerStyle<CustomIdent = S>,
        column_auto_repetitions: u16,
        row_auto_repetitions: u16,
    ) -> Self {
        let mut areas: Map<StrHasher<S>, GridTemplateArea<_>> = Map::new();
        let mut column_lines: NamedGridLinesMap<S> = Map::new();
        let mut row_lines: NamedGridLinesMap<S> = Map::new();

        #[cfg(feature = "detailed_layout_info")]
        let mut column_line_name_pairs: Vec<(u32, S)> = Vec::new();
        #[cfg(feature = "detailed_layout_info")]
        let mut row_line_name_pairs: Vec<(u32, S)> = Vec::new();

        let mut current_line = 0;
        if let Some(mut column_tracks) = style.grid_template_columns() {
            if let Some(column_line_names_iter) = style.grid_template_column_names() {
                for line_names in column_line_names_iter {
                    current_line += 1;
                    for line_name in line_names.into_iter() {
                        #[cfg(feature = "detailed_layout_info")]
                        column_line_name_pairs.push((current_line, line_name.clone()));
                        upsert_line_name_map(&mut column_lines, line_name.clone(), current_line);
                    }

                    if let Some(GenericGridTemplateComponent::Repeat(repeat)) = column_tracks.next() {
                        let repeat_count = match repeat.count() {
                            RepetitionCount::Count(count) => count,
                            RepetitionCount::AutoFill | RepetitionCount::AutoFit => column_auto_repetitions,
                        };

                        // Line name sets are positional: set `i` names the `i`th line of each
                        // repetition, and the final line name set of each repetition collapses
                        // with the first line name set of the following one. An empty list means
                        // the repetition's lines are unnamed; any other length must be exactly
                        // `track_count + 1` (one set per line, including both edge lines).
                        let line_name_set_count = repeat.lines_names().len() as u32;
                        let lines_per_repetition = repeat.track_count() as u32;
                        assert!(
                            line_name_set_count == 0 || line_name_set_count == lines_per_repetition + 1,
                            "grid template repetition must have no line name sets or exactly track count + 1 of them ({} tracks but {} line name sets)",
                            lines_per_repetition,
                            line_name_set_count,
                        );

                        for _ in 0..repeat_count {
                            for (line, line_name_set) in (current_line..).zip(repeat.lines_names()) {
                                for line_name in line_name_set {
                                    #[cfg(feature = "detailed_layout_info")]
                                    column_line_name_pairs.push((line, line_name.clone()));
                                    upsert_line_name_map(&mut column_lines, line_name.clone(), line);
                                }
                            }
                            current_line += lines_per_repetition;

                            // Names for lines beyond the maximum track limit are never resolvable:
                            // stop generating them (the explicit grid is clamped to MAX_GRID_TRACKS)
                            if current_line > MAX_GRID_TRACKS as u32 {
                                break;
                            }
                        }
                        // Last line name set collapses with following line name set
                        if repeat_count > 0 {
                            current_line = current_line.saturating_sub(1);
                        }
                    }
                }
            }
        }

        let mut current_line = 0;
        if let Some(mut row_tracks) = style.grid_template_rows() {
            if let Some(row_line_names_iter) = style.grid_template_row_names() {
                for line_names in row_line_names_iter {
                    current_line += 1;
                    for line_name in line_names.into_iter() {
                        #[cfg(feature = "detailed_layout_info")]
                        row_line_name_pairs.push((current_line, line_name.clone()));
                        upsert_line_name_map(&mut row_lines, line_name.clone(), current_line);
                    }

                    if let Some(GenericGridTemplateComponent::Repeat(repeat)) = row_tracks.next() {
                        let repeat_count = match repeat.count() {
                            RepetitionCount::Count(count) => count,
                            RepetitionCount::AutoFill | RepetitionCount::AutoFit => row_auto_repetitions,
                        };

                        // Line name sets are positional: set `i` names the `i`th line of each
                        // repetition, and the final line name set of each repetition collapses
                        // with the first line name set of the following one. An empty list means
                        // the repetition's lines are unnamed; any other length must be exactly
                        // `track_count + 1` (one set per line, including both edge lines).
                        let line_name_set_count = repeat.lines_names().len() as u32;
                        let lines_per_repetition = repeat.track_count() as u32;
                        assert!(
                            line_name_set_count == 0 || line_name_set_count == lines_per_repetition + 1,
                            "grid template repetition must have no line name sets or exactly track count + 1 of them ({} tracks but {} line name sets)",
                            lines_per_repetition,
                            line_name_set_count,
                        );

                        for _ in 0..repeat_count {
                            for (line, line_name_set) in (current_line..).zip(repeat.lines_names()) {
                                for line_name in line_name_set {
                                    #[cfg(feature = "detailed_layout_info")]
                                    row_line_name_pairs.push((line, line_name.clone()));
                                    upsert_line_name_map(&mut row_lines, line_name.clone(), line);
                                }
                            }
                            current_line += lines_per_repetition;

                            // Names for lines beyond the maximum track limit are never resolvable:
                            // stop generating them (the explicit grid is clamped to MAX_GRID_TRACKS)
                            if current_line > MAX_GRID_TRACKS as u32 {
                                break;
                            }
                        }
                        // Last line name set collapses with following line name set
                        if repeat_count > 0 {
                            current_line = current_line.saturating_sub(1);
                        }
                    }
                }
            }
        }
        // The size of the area template may be larger than the extents of the named areas
        // due to unnamed (`.`) cells, so it is taken from the style rather than being derived
        // from the areas themselves.
        let area_column_count = style.grid_template_area_column_count();
        let area_row_count = style.grid_template_area_row_count();
        if let Some(area_iter) = style.grid_template_areas() {
            for area in area_iter.into_iter() {
                // TODO: Investigate eliminating clones
                areas.insert(StrHasher(area.name.clone()), area.clone());

                let col_start_name = S::from(format!("{}-start", area.name.as_ref()));
                #[cfg(feature = "detailed_layout_info")]
                column_line_name_pairs.push((area.column_start as u32, col_start_name.clone()));
                upsert_line_name_map(&mut column_lines, col_start_name, area.column_start as u32);
                let col_end_name = S::from(format!("{}-end", area.name.as_ref()));
                #[cfg(feature = "detailed_layout_info")]
                column_line_name_pairs.push((area.column_end as u32, col_end_name.clone()));
                upsert_line_name_map(&mut column_lines, col_end_name, area.column_end as u32);
                let row_start_name = S::from(format!("{}-start", area.name.as_ref()));
                #[cfg(feature = "detailed_layout_info")]
                row_line_name_pairs.push((area.row_start as u32, row_start_name.clone()));
                upsert_line_name_map(&mut row_lines, row_start_name, area.row_start as u32);
                let row_end_name = S::from(format!("{}-end", area.name.as_ref()));
                #[cfg(feature = "detailed_layout_info")]
                row_line_name_pairs.push((area.row_end as u32, row_end_name.clone()));
                upsert_line_name_map(&mut row_lines, row_end_name, area.row_end as u32);
            }
        }

        // Sort and dedup lines for each column name
        for lines in column_lines.values_mut() {
            lines.sort_unstable();
            lines.dedup();
        }
        // Sort and dedup lines for each row name
        for lines in row_lines.values_mut() {
            lines.sort_unstable();
            lines.dedup();
        }

        Self {
            area_column_count,
            area_row_count,
            explicit_column_count: 0, // Overwritten later
            explicit_row_count: 0,    // Overwritten later
            areas,
            row_lines,
            column_lines,
            #[cfg(feature = "detailed_layout_info")]
            column_line_name_pairs,
            #[cfg(feature = "detailed_layout_info")]
            row_line_name_pairs,
        }
    }

    /// Build the per-line name groups of the explicit grid in the passed axis as a
    /// [`GridLineNames`], with `repeat()`s expanded and including the implicit
    /// `<name>-start`/`<name>-end` names generated by `grid-template-areas`.
    ///
    /// Line indices are relative to the explicit grid (line 0 = start of the first explicit track).
    #[cfg(feature = "detailed_layout_info")]
    pub(crate) fn detailed_line_names(&self, axis: AbsoluteAxis) -> GridLineNames<S> {
        let (pairs, explicit_track_count) = match axis {
            AbsoluteAxis::Horizontal => (&self.column_line_name_pairs, self.explicit_column_count),
            AbsoluteAxis::Vertical => (&self.row_line_name_pairs, self.explicit_row_count),
        };

        if pairs.is_empty() {
            return GridLineNames::default();
        }

        // Stable sort by line number preserves the source order of names within each line
        // (template names before area-generated names)
        let mut sorted_pairs: Vec<&(u32, S)> = pairs.iter().collect();
        sorted_pairs.sort_by_key(|(line, _)| *line);

        let line_count = explicit_track_count as usize + 1;
        let mut line_names = GridLineNames::with_capacity(sorted_pairs.len(), line_count + 1);
        let mut pair_iter = sorted_pairs.into_iter().peekable();
        for line in 1..=(line_count as u32) {
            line_names.start_line();
            while let Some(&&(pair_line, ref name)) = pair_iter.peek() {
                if pair_line != line {
                    break;
                }
                pair_iter.next();
                if !line_names.current_line_contains(name.as_ref()) {
                    line_names.push_name(name.clone());
                }
            }
        }
        line_names
    }

    /// Resolve named lines for both the `start` and `end` of a row-axis grid placement
    #[inline(always)]
    pub(crate) fn resolve_row_names(&self, line: &Line<GridPlacement<S>>) -> Line<NonNamedGridPlacement> {
        self.resolve_line_names(line, GridAreaAxis::Row)
    }

    /// Resolve named lines for both the `start` and `end` of a column-axis grid placement
    #[inline(always)]
    pub(crate) fn resolve_column_names(&self, line: &Line<GridPlacement<S>>) -> Line<NonNamedGridPlacement> {
        self.resolve_line_names(line, GridAreaAxis::Column)
    }

    /// Resolve named lines for both the `start` and `end` of a grid placement
    #[inline(always)]
    pub(crate) fn resolve_line_names(
        &self,
        line: &Line<GridPlacement<S>>,
        axis: GridAreaAxis,
    ) -> Line<NonNamedGridPlacement> {
        match axis {
            GridAreaAxis::Row => {
                NamedLineResolverAxis { lines: &self.row_lines, explicit_track_count: self.explicit_row_count }
            }
            GridAreaAxis::Column => {
                NamedLineResolverAxis { lines: &self.column_lines, explicit_track_count: self.explicit_column_count }
            }
        }
        .resolve_line_names(line)
    }

    /// Move the row and column line-name maps into the detailed grid information
    #[cfg(feature = "detailed_layout_info")]
    pub(crate) fn populate_detailed_line_resolvers(self, rows: &mut GridLineNames<S>, columns: &mut GridLineNames<S>) {
        rows.resolver = self.row_lines;
        columns.resolver = self.column_lines;
    }

    /// Get the number of columns defined by the grid areas
    pub(crate) fn area_column_count(&self) -> u16 {
        self.area_column_count
    }

    /// Get the number of rows defined by the grid areas
    pub(crate) fn area_row_count(&self) -> u16 {
        self.area_row_count
    }

    /// Set the number of columns in the explicit grid
    pub(crate) fn set_explicit_column_count(&mut self, count: u16) {
        self.explicit_column_count = count;
    }

    /// Set the number of rows in the explicit grid
    pub(crate) fn set_explicit_row_count(&mut self, count: u16) {
        self.explicit_row_count = count;
    }
}

impl<S: CheapCloneStr> Debug for NamedLineResolver<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "Grid Areas:")?;
        for area in self.areas.values() {
            writeln!(
                f,
                "{}: row:{}/{} col: {}/{}",
                area.name.as_ref(),
                area.row_start,
                area.row_end,
                area.column_start,
                area.column_end
            )?;
        }

        writeln!(f, "Grid Rows:")?;
        for (name, lines) in self.row_lines.iter() {
            write!(f, "{}: ", name.0.as_ref())?;
            for line in lines {
                write!(f, "{line}  ")?;
            }
            writeln!(f)?;
        }

        writeln!(f, "Grid Columns:")?;
        for (name, lines) in self.column_lines.iter() {
            write!(f, "{}: ", name.0.as_ref())?;
            for line in lines {
                write!(f, "{line}  ")?;
            }
            writeln!(f)?;
        }

        Ok(())
    }
}

/// The names of each explicit grid line in a single axis, stored in CSR (compressed sparse
/// row) format: a single flat `Vec` of names plus a `Vec` of offsets into it (one more offset
/// than there are lines). The names of line `i` (0-indexed) are `names[offsets[i]..offsets[i + 1]]`.
///
/// Line indices here are relative to the explicit grid;
/// [`DetailedGridTracksInfo`](crate::DetailedGridTracksInfo) provides accessors with indices
/// relative to the full grid (including implicit tracks).
///
/// Iterate over per-line name groups with [`GridLineNames::iter`], or access a single line's
/// names with [`GridLineNames::line`]. A grid with no named lines is represented by two empty
/// `Vec`s (see [`GridLineNames::is_empty`]).
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg(feature = "detailed_layout_info")]
pub struct GridLineNames<S: CheapCloneStr = DefaultCheapStr> {
    /// The names of every grid line in the axis, concatenated in line order
    names: Vec<S>,
    /// Offsets into `names`: line `i`'s names are `names[offsets[i]..offsets[i + 1]]`.
    /// Either empty (no named lines) or of length `line count + 1`.
    offsets: Vec<u32>,
    /// Named line lookup used to resolve arbitrary grid placements
    resolver: NamedGridLinesMap<S>,
}

#[cfg(feature = "detailed_layout_info")]
impl<S: CheapCloneStr> GridLineNames<S> {
    /// Create an empty `GridLineNames` with pre-allocated capacity
    pub(crate) fn with_capacity(name_capacity: usize, offset_capacity: usize) -> Self {
        let mut offsets = Vec::with_capacity(offset_capacity);
        offsets.push(0);
        Self { names: Vec::with_capacity(name_capacity), offsets, resolver: Map::new() }
    }

    /// Start a new (initially empty) line
    pub(crate) fn start_line(&mut self) {
        self.offsets.push(self.names.len() as u32);
    }

    /// Append a name to the current (last) line
    pub(crate) fn push_name(&mut self, name: S) {
        self.names.push(name);
        *self.offsets.last_mut().unwrap() = self.names.len() as u32;
    }

    /// Whether the current (last) line already contains the passed name
    pub(crate) fn current_line_contains(&self, name: &str) -> bool {
        self.line(self.line_count().wrapping_sub(1)).iter().any(|n| n.as_ref() == name)
    }

    /// Resolve named lines and spans using the retained line-name map
    pub(crate) fn resolve_line_names(
        &self,
        line: &Line<GridPlacement<S>>,
        explicit_track_count: u16,
    ) -> Line<NonNamedGridPlacement> {
        NamedLineResolverAxis { lines: &self.resolver, explicit_track_count }.resolve_line_names(line)
    }

    /// Whether the axis has any named lines at all
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// The number of grid lines represented (zero if the grid has no named lines)
    pub fn line_count(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    /// The names of the line with the passed 0-indexed line index.
    /// Returns an empty slice if the line has no names or the index is out of range.
    pub fn line(&self, line_index: usize) -> &[S] {
        match (self.offsets.get(line_index), self.offsets.get(line_index + 1)) {
            (Some(&start), Some(&end)) => &self.names[start as usize..end as usize],
            _ => &[],
        }
    }

    /// Iterate over the name group (`&[S]`) of each grid line in line order
    pub fn iter(&self) -> GridLineNamesIter<'_, S> {
        self.iter_padded(0, 0)
    }

    /// Iterate over the name group (`&[S]`) of each grid line in line order, additionally
    /// yielding `leading_empty` empty groups before the stored lines and `trailing_empty`
    /// empty groups after them (representing unnamed implicit grid lines)
    pub(crate) fn iter_padded(&self, leading_empty: usize, trailing_empty: usize) -> GridLineNamesIter<'_, S> {
        GridLineNamesIter { names: &self.names, offsets: self.offsets.windows(2), leading_empty, trailing_empty }
    }
}

#[cfg(feature = "detailed_layout_info")]
impl<'a, S: CheapCloneStr> IntoIterator for &'a GridLineNames<S> {
    type Item = &'a [S];
    type IntoIter = GridLineNamesIter<'a, S>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over the per-line name groups of a [`GridLineNames`]. Yields one `&[S]` per grid
/// line (which is empty for unnamed lines)
#[derive(Debug, Clone)]
#[cfg(feature = "detailed_layout_info")]
pub struct GridLineNamesIter<'a, S: CheapCloneStr> {
    /// The flat name storage being iterated over
    names: &'a [S],
    /// Iterator over adjacent pairs of offsets into `names`
    offsets: core::slice::Windows<'a, u32>,
    /// Number of (unnamed, implicit) lines remaining before the stored lines
    leading_empty: usize,
    /// Number of (unnamed, implicit) lines remaining after the stored lines
    trailing_empty: usize,
}

#[cfg(feature = "detailed_layout_info")]
impl<'a, S: CheapCloneStr> Iterator for GridLineNamesIter<'a, S> {
    type Item = &'a [S];

    fn next(&mut self) -> Option<Self::Item> {
        if self.leading_empty > 0 {
            self.leading_empty -= 1;
            return Some(&[]);
        }
        if let Some(window) = self.offsets.next() {
            return Some(&self.names[window[0] as usize..window[1] as usize]);
        }
        if self.trailing_empty > 0 {
            self.trailing_empty -= 1;
            return Some(&[]);
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.leading_empty + self.offsets.len() + self.trailing_empty;
        (len, Some(len))
    }
}

#[cfg(feature = "detailed_layout_info")]
impl<S: CheapCloneStr> ExactSizeIterator for GridLineNamesIter<'_, S> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::GenericGridPlacement;
    use crate::sys::DefaultCheapStr;
    use crate::GridTemplateAreas;
    use crate::Style;

    fn resolver(explicit_track_count: u16) -> NamedLineResolver<DefaultCheapStr> {
        let mut resolver = NamedLineResolver::new(&Style::DEFAULT, 0, 0);
        resolver.set_explicit_column_count(explicit_track_count);
        resolver
    }

    fn resolved_start_line(
        resolver: &NamedLineResolver<DefaultCheapStr>,
        placement: GridPlacement<DefaultCheapStr>,
    ) -> i16 {
        let resolved = resolver.resolve_column_names(&Line { start: placement, end: GridPlacement::Auto });
        match resolved.start {
            GenericGridPlacement::Line(line) => line.as_i16(),
            _ => panic!("expected a resolved line"),
        }
    }

    #[test]
    fn extreme_missing_named_line_indices_do_not_overflow() {
        let resolver = resolver(10_000);
        assert_eq!(
            resolved_start_line(&resolver, GridPlacement::NamedLine(DefaultCheapStr::from("missing"), i16::MAX)),
            i16::MAX
        );
        assert_eq!(
            resolved_start_line(&resolver, GridPlacement::NamedLine(DefaultCheapStr::from("missing"), i16::MIN)),
            i16::MIN
        );
    }

    /// `[s] 97px [m] 213px [m] 71px [e]` in the column axis: three tracks, `m` on lines 2 and 3
    fn columns_s_m_m_e() -> NamedLineResolver<DefaultCheapStr> {
        let mut resolver = resolver(3);
        for (name, line) in [("s", 1), ("m", 2), ("m", 3), ("e", 4)] {
            upsert_line_name_map(&mut resolver.column_lines, DefaultCheapStr::from(name), line);
        }
        resolver
    }

    fn resolved_columns(
        resolver: &NamedLineResolver<DefaultCheapStr>,
        start: GridPlacement<DefaultCheapStr>,
        end: GridPlacement<DefaultCheapStr>,
    ) -> (i16, i16) {
        let resolved = resolver.resolve_column_names(&Line { start, end });
        match (resolved.start, resolved.end) {
            (GenericGridPlacement::Line(start), GenericGridPlacement::Line(end)) => (start.as_i16(), end.as_i16()),
            other => panic!("expected two resolved lines, got {other:?}"),
        }
    }

    fn named(name: &str, idx: i16) -> GridPlacement<DefaultCheapStr> {
        GridPlacement::NamedLine(DefaultCheapStr::from(name), idx)
    }

    fn named_span(name: &str, idx: u16) -> GridPlacement<DefaultCheapStr> {
        GridPlacement::NamedSpan(DefaultCheapStr::from(name), idx)
    }

    fn line(line: i16) -> GridPlacement<DefaultCheapStr> {
        GridPlacement::Line(GridLine::from(line))
    }

    /// A negative index past the named lines counts on into the implicit lines *before* the
    /// explicit grid (css-grid-2 §8.3): with three tracks, `-5` is the line before line 1
    /// and `-6` the one before that.
    #[test]
    fn negative_index_past_the_named_lines_is_before_the_grid() {
        let resolver = columns_s_m_m_e();
        // Two `m` lines: `m -3` is one implicit line before line 1, `m -4` two
        assert_eq!(resolved_start_line(&resolver, named("m", -3)), -5);
        assert_eq!(resolved_start_line(&resolver, named("m", -4)), -6);
        // Within the named lines, unchanged
        assert_eq!(resolved_start_line(&resolver, named("m", -1)), 3);
        assert_eq!(resolved_start_line(&resolver, named("m", -2)), 2);
        // A name no line carries: every implicit line carries it
        assert_eq!(resolved_start_line(&resolver, named("nosuch", -1)), -5);
        assert_eq!(resolved_start_line(&resolver, named("nosuch", -2)), -6);
        assert_eq!(resolved_start_line(&resolver, named("nosuch", 1)), 5);
        // Positive past the named lines: after the grid, unchanged
        assert_eq!(resolved_start_line(&resolver, named("m", 3)), 5);
    }

    /// `span <name> N / <line>` counts back from the end line, so the nearest line is the first
    #[test]
    fn span_name_before_a_line_counts_back_from_it() {
        let resolver = columns_s_m_m_e();
        assert_eq!(resolved_columns(&resolver, named_span("m", 1), named("e", 0)), (3, 4));
        assert_eq!(resolved_columns(&resolver, named_span("m", 2), named("e", 0)), (2, 4));
        // Past the named lines: implicit lines before the grid
        assert_eq!(resolved_columns(&resolver, named_span("m", 3), named("e", 0)), (-5, 4));
        // A negative end line: `-1` is line 4, `-2` line 3
        assert_eq!(resolved_columns(&resolver, named_span("m", 1), line(-1)), (3, -1));
        assert_eq!(resolved_columns(&resolver, named_span("m", 1), line(-2)), (2, -2));
        // An end line before the grid: every line before it is implicit and carries the name
        assert_eq!(resolved_columns(&resolver, named_span("m", 1), line(-6)), (-7, -6));
    }

    /// `<line> / span <name> N` counts on from the start line
    #[test]
    fn span_name_after_a_line_counts_on_from_it() {
        let resolver = columns_s_m_m_e();
        // A negative start line: `-4` is line 1, `-3` line 2
        assert_eq!(resolved_columns(&resolver, line(-4), named_span("m", 1)), (-4, 2));
        assert_eq!(resolved_columns(&resolver, line(-3), named_span("m", 1)), (-3, 3));
        // Past the named lines: implicit lines after the grid
        assert_eq!(resolved_columns(&resolver, line(2), named_span("m", 3)), (2, 6));
        // A start line already past the grid: the lines after it
        assert_eq!(resolved_columns(&resolver, line(6), named_span("nosuch", 1)), (6, 7));
    }

    #[test]
    fn large_named_span_does_not_wrap_negative() {
        let resolver = resolver(10_000);
        let resolved = resolver.resolve_column_names(&Line {
            start: GridPlacement::Line(GridLine::from(1)),
            end: GridPlacement::NamedSpan(DefaultCheapStr::from("missing"), u16::MAX),
        });
        match resolved.end {
            GenericGridPlacement::Line(line) => assert_eq!(line.as_i16(), i16::MAX),
            _ => panic!("expected a resolved line"),
        }
    }

    #[test]
    fn area_lines_saturate_when_converted_to_grid_lines() {
        let style = Style {
            grid_template_areas: Some(GridTemplateAreas {
                areas: vec![GridTemplateArea {
                    name: DefaultCheapStr::from("area"),
                    row_start: 1,
                    row_end: 2,
                    column_start: u16::MAX,
                    column_end: u16::MAX,
                }],
                row_count: 1,
                column_count: u16::MAX,
            }),
            ..Style::DEFAULT
        };
        let resolver = NamedLineResolver::new(&style, 0, 0);
        assert_eq!(
            resolved_start_line(&resolver, GridPlacement::NamedLine(DefaultCheapStr::from("area-start"), 1)),
            i16::MAX
        );
    }
}
