//! `round_layout` rounds a box's location as an absolute edge (rinch #2437):
//! `round(abs) - round(parent abs)`, so the rounded locations down a chain sum
//! to `round` of the exact absolute position at any depth.
//!
//! Ten nested blocks, each with `padding-left: 13.774px; padding-top: 7.3px`,
//! in a 400px column. Chrome 157 puts the innermost box at x 137.65625, y
//! 72.96875 (1/64px layout units), right edge 400; rounding each
//! parent-relative offset on its own put it at 140, 70, right edge 402.
#![cfg(feature = "block_layout")]

use taffy::prelude::*;

#[test]
fn nested_fractional_insets_round_once_on_the_absolute_position() {
    let mut taffy: TaffyTree<()> = TaffyTree::new();
    let mut node =
        taffy.new_leaf(Style { size: Size { width: auto(), height: length(1.0) }, ..Default::default() }).unwrap();
    let mut chain = vec![node];
    for _ in 0..10 {
        node = taffy
            .new_with_children(
                Style {
                    display: Display::Block,
                    padding: Rect { left: length(13.774), right: zero(), top: length(7.3), bottom: zero() },
                    ..Default::default()
                },
                &[node],
            )
            .unwrap();
        chain.push(node);
    }
    let root = taffy
        .new_with_children(
            Style {
                display: Display::Block,
                size: Size { width: length(400.0), height: auto() },
                ..Default::default()
            },
            &[node],
        )
        .unwrap();
    taffy.compute_layout(root, Size::MAX_CONTENT).unwrap();

    // From the outermost padded block (level 0, at 0) inwards.
    let (mut x, mut y) = (0.0_f32, 0.0_f32);
    for (level, &n) in chain.iter().rev().enumerate() {
        let l = taffy.layout(n).unwrap();
        x += l.location.x;
        y += l.location.y;
        let exact = level as f32 * 13.774;
        assert_eq!(x, exact.round(), "level {level}: x");
        assert_eq!(y, (level as f32 * 7.3).round(), "level {level}: y");
        assert_eq!(x + l.size.width, 400.0, "level {level}: right edge");
    }
    assert_eq!((x, y), (138.0, 73.0));
}
