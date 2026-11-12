//! Style types for Block layout
use crate::style::{AlignContent, AlignItems, AlignSelf};
use crate::{CoreStyle, Style};

/// The set of styles required for a Block layout container
pub trait BlockContainerStyle: CoreStyle {
    /// Defines which row in the grid the item should start and end at
    #[inline(always)]
    fn text_align(&self) -> TextAlign {
        Style::<Self::CustomIdent>::DEFAULT.text_align
    }

    /// How children of this block container are aligned in the block (cross) axis
    #[inline(always)]
    fn align_content(&self) -> Option<AlignContent> {
        Style::<Self::CustomIdent>::DEFAULT.align_content
    }

    /// RINCH: `justify-items`: where a block-level child with `justify-self: auto` goes on the
    /// inline axis (css-align-3 §6.1). `None` is `normal`.
    #[inline(always)]
    fn justify_items(&self) -> Option<AlignItems> {
        None
    }
}

/// The set of styles required for a Block layout item (child of a Block container)
pub trait BlockItemStyle: CoreStyle {
    /// Whether the item is a table. Table children are handled specially in block layout.
    #[inline(always)]
    fn is_table(&self) -> bool {
        false
    }

    /// RINCH: whether the item is a table that resolves its own height: its `height` is the
    /// least it is, and it grows to its rows ([`crate::Style::table_rows`]). Block layout hands
    /// it no height.
    #[inline(always)]
    fn resolves_own_height(&self) -> bool {
        false
    }

    /// RINCH: whether the item is an automatic-layout table ([`crate::Style::table_rows`] and
    /// [`crate::Style::proportional_column_sizing`]): it is never narrower than its columns'
    /// minimum, whatever width its parent hands it or its `width` gives it (CSS 2.1 §17.5.2.2),
    /// so its contribution to its parent's intrinsic width is at least that minimum (rinch #2328).
    #[inline(always)]
    fn resolves_own_width(&self) -> bool {
        false
    }

    /// RINCH: `justify-self`: where the item goes on its container's inline axis (css-align-3
    /// §6.1). `None` is `auto` (the container's `justify-items`).
    #[inline(always)]
    fn justify_self(&self) -> Option<AlignSelf> {
        None
    }

    /// Whether the item is a floated
    #[cfg(feature = "float_layout")]
    #[inline(always)]
    fn float(&self) -> super::Float {
        super::Float::None
    }

    /// Whether the item is a floated
    #[cfg(feature = "float_layout")]
    #[inline(always)]
    fn clear(&self) -> super::Clear {
        super::Clear::None
    }
}

/// Used by block layout to implement the legacy behaviour of `<center>` and `<div align="left | right | center">`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TextAlign {
    /// No special legacy text align behaviour.
    #[default]
    Auto,
    /// Corresponds to `-webkit-left` or `-moz-left` in browsers
    LegacyLeft,
    /// Corresponds to `-webkit-right` or `-moz-right` in browsers
    LegacyRight,
    /// Corresponds to `-webkit-center` or `-moz-center` in browsers
    LegacyCenter,
}

#[cfg(feature = "parse")]
crate::util::parse::impl_parse_for_keyword_enum!(TextAlign,
    "auto" => Auto,
    "-webkit-left" => LegacyLeft,
    "-webkit-right" => LegacyRight,
    "-webkit-center" => LegacyCenter,
);
