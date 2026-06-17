// Copyright (c) 2023-present, Raphael Amorim.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! Shared atlas types used by both the Metal and wgpu grid backends.
//!
//! The atlas texture itself is backend-specific (Metal `Texture` vs
//! wgpu `Texture`), so each backend owns its own atlas struct. These
//! types are the common vocabulary: how callers identify a glyph
//! (`GlyphKey`), where it landed in the atlas (`AtlasSlot`), and the
//! caller-supplied rasterized pixels (`RasterizedGlyph`).

/// Identifier for a rasterized glyph. `(font_id, glyph_id)` is
/// enough when a grid renders at one font size; `size_bucket` lets
/// us share the atlas across minor size changes (e.g. during a
/// resize animation) without re-rasterizing. `color_variant` is zero
/// for ordinary glyphs and carries exact RGBA for pre-painted custom
/// glyphs whose COLR graph depends on the current foreground.
/// Quantize size to 1/4 of a physical pixel to keep the cache hit rate
/// high: `size_bucket = (scaled_px * 4.0).round() as u16`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GlyphKey {
    pub font_id: u32,
    pub glyph_id: u32,
    pub size_bucket: u16,
    pub color_variant: u32,
}

/// Atlas position + glyph metrics for one rasterized glyph. Exactly
/// the fields the `grid_text_vertex` shader reads via `CellText`:
/// `glyph_pos`, `glyph_size`, `bearings`.
///
/// `page` is the index of the atlas page this glyph lives in. The
/// Vulkan backend's per-kind atlas is a list of fixed-size pages
/// (`grid/vulkan.rs`); the other backends use a single image / texture
/// and leave `page` at 0.
#[derive(Clone, Copy, Debug, Default)]
pub struct AtlasSlot {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    pub bearing_x: i16,
    pub bearing_y: i16,
    pub page: u8,
}

/// Raw rasterized glyph bitmap, caller-supplied. Bytes are row-major
/// with no row stride: one byte per pixel for grayscale-atlas glyphs,
/// premultiplied RGBA8 for color-atlas glyphs. The atlas doesn't
/// rasterize itself — that stays in whatever shaping / scaling path
/// the caller uses (sugarloaf's swash-backed `ScaleContext`).
#[derive(Clone, Copy)]
pub struct RasterizedGlyph<'a> {
    pub width: u16,
    pub height: u16,
    pub bearing_x: i16,
    pub bearing_y: i16,
    pub bytes: &'a [u8],
}

/// Convert straight RGBA bytes to premultiplied RGBA in place.
pub fn premultiply_straight_rgba_in_place(bytes: &mut [u8]) {
    for px in bytes.chunks_exact_mut(4) {
        let a = px[3] as u16;
        px[0] = ((px[0] as u16 * a + 127) / 255) as u8;
        px[1] = ((px[1] as u16 * a + 127) / 255) as u8;
        px[2] = ((px[2] as u16 * a + 127) / 255) as u8;
    }
}

/// Swash color bitmaps are decoded as straight RGBA. Swash COLR
/// outlines are already composited into premultiplied RGBA.
pub fn swash_color_source_needs_premultiply(source: swash::scale::Source) -> bool {
    matches!(source, swash::scale::Source::ColorBitmap(_))
}

#[cfg(test)]
mod tests {
    // Test lane: default

    use super::*;

    #[test]
    // Defends: straight-RGBA color bitmap glyphs match the premultiplied atlas blend contract.
    fn straight_rgba_bytes_are_premultiplied_for_color_atlas() {
        let mut bytes = vec![
            200, 100, 50, 128, //
            20, 40, 60, 255, //
            250, 120, 80, 0,
        ];

        premultiply_straight_rgba_in_place(&mut bytes);

        assert_eq!(
            bytes,
            vec![
                100, 50, 25, 128, //
                20, 40, 60, 255, //
                0, 0, 0, 0,
            ]
        );
    }

    #[test]
    // Defends: Swash COLR outlines are not premultiplied twice before color-atlas upload.
    fn swash_color_outline_is_not_premultiplied_twice() {
        use swash::scale::{Source, StrikeWith};

        assert!(!swash_color_source_needs_premultiply(Source::ColorOutline(
            0
        )));
        assert!(swash_color_source_needs_premultiply(Source::ColorBitmap(
            StrikeWith::BestFit
        )));
    }
}
