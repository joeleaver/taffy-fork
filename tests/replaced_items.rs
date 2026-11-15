//! Replaced items in flex and grid containers (rinch #2700, #1280).
//!
//! - `Style::normal_aligns_as_start`: a grid item whose `justify-self` and
//!   `align-self` are `normal` (set on neither it nor its container) is laid
//!   out at its own size in its grid area, as a replaced element is
//!   (css-align-3 §6.1), not stretched.
//! - A replaced flex item (`Style::item_is_replaced`) is measured with the
//!   container's definite main size as its parent's, so its percentage padding
//!   counts in its flex base size.
#![cfg(all(feature = "grid", feature = "flexbox"))]

use taffy::compute_leaf_layout;
use taffy::prelude::*;
use taffy::{MaybeResolve, ResolveOrZero, SizingMode};

/// A 2:1 picture, 800x400 natural, measured as the content box Taffy asks
/// about: a known width or height gives the other through the ratio.
fn picture(known: Size<Option<f32>>) -> Size<f32> {
    match (known.width, known.height) {
        (Some(w), Some(h)) => Size { width: w, height: h },
        (Some(w), None) => Size { width: w, height: w / 2.0 },
        (None, Some(h)) => Size { width: h * 2.0, height: h },
        (None, None) => Size { width: 800.0, height: 400.0 },
    }
}

/// `container` holding one leaf of `item` style measured as [`picture`] (its
/// known dimensions less its padding, resolved against the parent size it is
/// handed). Returns the item's layout size and every parent width its measure
/// saw.
fn lay_out(container: Style, item: Style) -> (Size<f32>, Vec<Option<f32>>) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let leaf = tree.new_leaf_with_context(item, ()).unwrap();
    let root = tree.new_with_children(container, &[leaf]).unwrap();
    let mut parents = Vec::new();
    tree.compute_layout_with_measure(
        root,
        Size { width: AvailableSpace::Definite(800.0), height: AvailableSpace::MaxContent },
        |inputs, _, _, style| {
            parents.push(inputs.parent_size.width);
            let pb = style.padding.resolve_or_zero(inputs.parent_size.width, |_, _| 0.0).sum_axes();
            // As rinch's measure does: the size its parent knows (a border
            // box) or, in `InherentSize` mode, its own style length (a
            // content box), the other through the ratio.
            let own = match inputs.sizing_mode {
                SizingMode::InherentSize => style.size.maybe_resolve(inputs.parent_size, |_, _| 0.0),
                SizingMode::ContentSize => Size::NONE,
            };
            let known = Size {
                width: inputs.known_dimensions.width.map(|w| w - pb.width).or(own.width),
                height: inputs.known_dimensions.height.map(|h| h - pb.height).or(own.height),
            };
            compute_leaf_layout(inputs, style, |_, _| 0.0, |_, _| picture(known))
        },
    )
    .unwrap();
    (tree.layout(leaf).unwrap().size, parents)
}

/// A grid of one 317px column and one row `height` tall.
fn grid(height: f32) -> Style {
    Style {
        display: Display::Grid,
        size: Size { width: length(317.0), height: length(height) },
        grid_template_columns: vec![GridTemplateComponent::Single(length(317.0))],
        grid_template_rows: vec![GridTemplateComponent::Single(length(height))],
        ..Default::default()
    }
}

#[track_caller]
fn near(got: Size<f32>, want: Size<f32>) {
    // Taffy rounds the layout to whole pixels.
    assert!((got.width - want.width).abs() <= 0.5 && (got.height - want.height).abs() <= 0.5, "{got:?} != {want:?}");
}

fn sized(width: f32, flag: bool) -> Style {
    Style { size: Size { width: length(width), height: auto() }, normal_aligns_as_start: flag, ..Default::default() }
}

#[test]
fn a_flagged_grid_item_keeps_its_height_in_a_taller_area() {
    // A 203px-wide picture in a 317x153 grid: 203x101.5 in Chrome 157, where
    // a stretched one is 153 tall.
    let (flagged, _) = lay_out(grid(153.0), sized(203.0, true));
    near(flagged, Size { width: 203.0, height: 101.5 });
    let (plain, _) = lay_out(grid(153.0), sized(203.0, false));
    assert_eq!(plain.height, 153.0);
}

#[test]
fn a_flagged_grid_item_keeps_its_width_in_a_narrower_area() {
    // No width: start-aligned, the picture is its natural 800x400 and
    // overflows the 317px area; stretched, it is 317 wide.
    let item = |flag| Style { normal_aligns_as_start: flag, ..Default::default() };
    let (flagged, _) = lay_out(grid(1000.0), item(true));
    assert_eq!(flagged, Size { width: 800.0, height: 400.0 });
    let (plain, _) = lay_out(grid(1000.0), item(false));
    assert_eq!(plain, Size { width: 317.0, height: 1000.0 });
}

#[test]
fn a_flagged_item_still_follows_its_containers_or_its_own_alignment() {
    // `align-items: stretch` on the container, or `align-self: stretch` on
    // the item, is not `normal`: the item stretches.
    let mut container = grid(153.0);
    container.align_items = Some(AlignItems::STRETCH);
    let (stretched, _) = lay_out(container, sized(203.0, true));
    assert_eq!(stretched.height, 153.0);
    let mut item = sized(203.0, true);
    item.align_self = Some(AlignSelf::STRETCH);
    let (stretched, _) = lay_out(grid(153.0), item);
    assert_eq!(stretched.height, 153.0);
    let mut item = sized(203.0, true);
    item.align_self = Some(AlignSelf::END);
    let (end, _) = lay_out(grid(153.0), item);
    near(end, Size { width: 203.0, height: 101.5 });
}

#[test]
fn a_replaced_flex_item_is_measured_with_the_rows_width() {
    // `padding: 7%` in a 317px row with `height: 153px`, stretched: the
    // content box is 153 - 2 x 22.19 tall, so 217.25 wide, and the border box
    // 261.625 (Chrome 157). Measured with no parent width, the padding counted
    // 0 and the item was 306 wide.
    let row = Style {
        display: Display::Flex,
        size: Size { width: length(317.0), height: length(153.0) },
        ..Default::default()
    };
    let item = |replaced| Style {
        padding: Rect { left: percent(0.07), right: percent(0.07), top: percent(0.07), bottom: percent(0.07) },
        item_is_replaced: replaced,
        ..Default::default()
    };
    let (size, parents) = lay_out(row.clone(), item(true));
    near(size, Size { width: 261.625, height: 153.0 });
    assert!(parents.iter().all(|p| *p == Some(317.0)), "{parents:?}");
    // Any other item still is measured with no main size, as upstream (and
    // its padding counts 0 there: 306 wide).
    let (size, parents) = lay_out(row, item(false));
    near(size, Size { width: 306.0, height: 153.0 });
    assert!(parents.contains(&None), "{parents:?}");
}
