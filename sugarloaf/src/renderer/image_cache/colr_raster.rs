// CPU rasteriser for OpenType COLR v0 / v1 paint graphs.
//
// Drives ttf-parser's `colr::Painter` trait against a `tiny-skia`
// backend. The result is a premultiplied RGBA8 bitmap that the caller
// uploads into sugarloaf's colour atlas — so the same code path works
// across all three backends (Wgpu, native Metal, Cpu) since
// rasterisation happens on CPU and only the upload is backend-specific.
//
// Correctness budget:
//   * Linear + radial gradients are handled correctly, including the
//     3-point → 2-point projection COLR v1 requires (porting the
//     math from skrifa/color/traversal.rs).
//   * Sweep gradients degrade to the first stop's solid colour.
//   * Variable-font coordinates render at the default instance.
//   * Composite modes beyond the painter's-algorithm `SrcOver` map
//     through `CompositeMode → BlendMode`; tiny-skia implements the
//     full set so there's no loss, but we pass them through rather
//     than validating each one per font.
//
// Rasterisation is cached upstream by the glyph atlas key. Glyph
// Protocol COLR payloads that use paletteIndex 0xFFFF include the
// current foreground RGBA in that key because this rasterizer
// pre-paints color pixels before upload. Normal font COLR glyphs keep
// their ink inside the requested visual size so emoji fonts with loose
// paint bounds do not spill out of terminal cells.

use ttf_parser::colr::{
    ClipBox, CompositeMode, GradientExtend, LinearGradient as TtfLinear, Paint, Painter,
    RadialGradient as TtfRadial,
};
use ttf_parser::{GlyphId, OutlineBuilder, RgbaColor, Transform as TtfTransform};

use tiny_skia::{
    BlendMode, Color, FillRule, GradientStop, LinearGradient, Mask, Paint as SkPaint,
    Path, PathBuilder, Pixmap, PixmapPaint, Point, RadialGradient, Rect, Shader,
    SpreadMode, Transform,
};

use crate::font::glyf_decode;

/// Bitmap + placement metadata produced by [`rasterize_payload`] (and
/// internally by [`rasterize`]). `is_color` distinguishes the two
/// underlying formats: `false` → A8 alpha mask (mono `glyf`), `true`
/// → premultiplied RGBA8 (`colrv0` / `colrv1`). The grid renderer
/// routes mono entries to the grayscale atlas and colour entries to
/// the colour atlas based on this flag.
pub struct RasterizedPayload {
    pub data: Vec<u8>,
    pub width: u16,
    pub height: u16,
    /// Pixel offset from the cell's pen position to the bitmap's
    /// left edge.
    pub left: i32,
    /// Pixel offset from the baseline to the bitmap's top edge
    /// (positive = baseline below the top).
    pub top: i32,
    pub is_color: bool,
}

const EMPTY_CPAL: [u8; 12] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0C,
];
const RASTER_PAD: f32 = 1.0;
const MAX_RASTER_SIDE: u32 = 8192;

/// Rasterise a COLR/CPAL glyph from a normal font face. This covers
/// installed colour-vector emoji fonts used through normal font
/// fallback/symbol-map routing, not only Glyph Protocol payloads.
pub fn rasterize_font_glyph(
    font: swash::FontRef<'_>,
    glyph_id: u16,
    pixel_size: u16,
    foreground_rgba: [u8; 4],
) -> Option<RasterizedPayload> {
    let face = face_from_swash_font(font)?;
    rasterize_face_glyph(&face, GlyphId(glyph_id), pixel_size, foreground_rgba)
}

/// Rasterise a registered Glyph Protocol payload. Dispatches the
/// monochrome `glyf` path through tiny-skia's anti-aliased fill (A8
/// output) and the colour `colrv0`/`colrv1` paths through the COLR
/// painter graph (premultiplied RGBA8 output). Returns `None` on
/// malformed payload or degenerate sizing.
pub fn rasterize_payload(
    payload: &crate::font::glyph_registry::StoredPayload,
    upm: u16,
    pixel_size: u16,
    foreground_rgba: [u8; 4],
) -> Option<RasterizedPayload> {
    use crate::font::glyph_registry::StoredPayload;
    match payload {
        StoredPayload::Glyf { glyf } => rasterize_mono(glyf, upm, pixel_size),
        StoredPayload::ColrV0 { glyphs, colr, cpal }
        | StoredPayload::ColrV1 { glyphs, colr, cpal } => {
            rasterize(glyphs, colr, cpal, upm, pixel_size, foreground_rgba)
        }
    }
}

pub fn payload_depends_on_foreground(
    payload: &crate::font::glyph_registry::StoredPayload,
) -> bool {
    use crate::font::glyph_registry::StoredPayload;
    match payload {
        StoredPayload::Glyf { .. } => false,
        StoredPayload::ColrV0 { glyphs, colr, cpal }
        | StoredPayload::ColrV1 { glyphs, colr, cpal } => {
            colr_payload_depends_on_foreground(glyphs, colr, cpal).unwrap_or(false)
        }
    }
}

/// Walk a `glyf` simple-glyph outline and rasterise it as an A8 alpha
/// mask sized to fit `pixel_size`. The atlas-bound caller uploads
/// the bytes straight into the grayscale atlas, same shape as the
/// swash/CT mono path produces.
fn rasterize_mono(glyf: &[u8], upm: u16, pixel_size: u16) -> Option<RasterizedPayload> {
    if pixel_size == 0 || upm == 0 {
        return None;
    }

    let outline = glyf_decode::decode(glyf).ok()?;
    let scale = pixel_size as f32 / upm as f32;

    let pix_w =
        raster_axis_pixels(i32::from(outline.x_max) - i32::from(outline.x_min), scale)?;
    let pix_h =
        raster_axis_pixels(i32::from(outline.y_max) - i32::from(outline.y_min), scale)?;

    // `glyf_decode::Outline::walk` flips Y so its output is Y-down
    // with origin at the top of the bbox (y=0 → top, y increases
    // downward to `y_max - y_min`). That matches tiny-skia's pixmap
    // convention exactly, so we feed walk's coords straight in
    // without further flipping. The COLR rasteriser un-flips because
    // its painter expects Y-up design units; this monochrome path
    // skips the painter and goes pixmap-direct.
    let cmds = outline.walk(1, 1.0);
    if cmds.is_empty() {
        return None;
    }
    let mut pb = PathBuilder::new();
    for cmd in &cmds {
        match *cmd {
            glyf_decode::PathCmd::MoveTo { x, y } => pb.move_to(x, y),
            glyf_decode::PathCmd::LineTo { x, y } => pb.line_to(x, y),
            glyf_decode::PathCmd::QuadTo { cx, cy, x, y } => pb.quad_to(cx, cy, x, y),
            glyf_decode::PathCmd::Close => pb.close(),
        }
    }
    let path = pb.finish()?;

    let mut pixmap = Pixmap::new(pix_w, pix_h)?;
    // X: shift so design `x_min` lands at pixel `pad`.
    // Y: walk already puts the bbox top at y=0, so a flat `pad`
    // offset places the top of the glyph one px below the pixmap
    // top edge.
    let ctm = Transform::from_row(
        scale,
        0.0,
        0.0,
        scale,
        RASTER_PAD - outline.x_min as f32 * scale,
        RASTER_PAD,
    );
    let mut paint = SkPaint::default();
    paint.set_color_rgba8(0xFF, 0xFF, 0xFF, 0xFF);
    paint.anti_alias = true;
    pixmap.fill_path(&path, &paint, FillRule::Winding, ctm, None);

    // Pixmap stores premultiplied RGBA; for an A8 mask we just take
    // the alpha channel (which equals R/G/B since we filled white).
    let data: Vec<u8> = pixmap.pixels().iter().map(|p| p.alpha()).collect();

    // Placement: `floor` left and `ceil` top to expand outward by a
    // sub-pixel and avoid clipping anti-aliased edges, matching the
    // COLR rasteriser's convention. The bitmap's top edge sits at
    // design-unit `y_max` in baseline-up convention.
    let left = (outline.x_min as f32 * scale - RASTER_PAD).floor() as i32;
    let top = (outline.y_max as f32 * scale + RASTER_PAD).ceil() as i32;

    Some(RasterizedPayload {
        data,
        width: pix_w as u16,
        height: pix_h as u16,
        left,
        top,
        is_color: false,
    })
}

/// Rasterise a COLR glyph to RGBA. Returns `None` when COLR/CPAL is
/// malformed, when no paintable bounds can be derived, or when tiny-skia
/// rejects a degenerate configuration (e.g. zero pixmap size).
pub(super) fn rasterize(
    glyphs: &[Vec<u8>],
    colr_bytes: &[u8],
    cpal_bytes: &[u8],
    upm: u16,
    pixel_size: u16,
    foreground: [u8; 4],
) -> Option<RasterizedPayload> {
    if pixel_size == 0 || upm == 0 {
        return None;
    }

    // ttf-parser's `colr::Table::parse` requires a non-empty CPAL
    // slice, even for v1 fonts that make no palette lookups. If the
    // container ships an empty CPAL (legal for v1-only paints), feed
    // the parser a zero-entry placeholder.
    let cpal_source: &[u8] = if cpal_bytes.is_empty() {
        &EMPTY_CPAL
    } else {
        cpal_bytes
    };
    let cpal = ttf_parser::cpal::Table::parse(cpal_source)?;
    let colr = ttf_parser::colr::Table::parse(cpal, colr_bytes)?;
    let outline_source = OutlineSource::GlyphProtocol(glyphs);
    let fg = RgbaColor::new(foreground[0], foreground[1], foreground[2], foreground[3]);
    let (base_gid, painted_bbox) =
        select_base_glyph(colr_bytes, outline_source, |gid, painter| {
            colr.paint(GlyphId(gid), 0, painter, &[], fg)
        })?;

    // Prefer the COLR ClipBox — authoritative per the OpenType spec.
    // Without one, derive bounds by walking the paint graph so empty
    // wrapper base glyphs can still render their layer glyphs. Fall
    // back to the base glyph's `glyf` bbox for simpler fonts that carry
    // geometry on the base glyph. Pad 1 px each side so anti-aliased
    // layer edges that drift slightly past the declared bbox (common in
    // hand-authored fonts) aren't clipped.
    //
    // Widened to i32 immediately because a saturated ClipBox (e.g.
    // `x_min = i16::MIN`, `x_max = i16::MAX`) would overflow on the
    // `x_max - x_min` subtraction below if kept as i16 — wrapping in
    // release and panicking in debug.
    let (x_min, y_min, x_max, y_max): (i32, i32, i32, i32) =
        match colr.clip_box(GlyphId(base_gid), &[]) {
            Some(cb) => (
                cb.x_min.floor() as i32,
                cb.y_min.floor() as i32,
                cb.x_max.ceil() as i32,
                cb.y_max.ceil() as i32,
            ),
            None => painted_bbox.or_else(|| {
                glyphs
                    .get(base_gid as usize)
                    .and_then(|bytes| glyf_bbox(bytes))
                    .map(|(a, b, c, d)| (a as i32, b as i32, c as i32, d as i32))
            })?,
        };
    let scale = pixel_size as f32 / upm as f32;
    rasterize_colr_paint(
        (x_min, y_min, x_max, y_max),
        scale,
        foreground,
        outline_source,
        |raster, fg| colr.paint(GlyphId(base_gid), 0, raster, &[], fg),
    )
}

fn rasterize_face_glyph(
    face: &ttf_parser::Face<'_>,
    glyph_id: GlyphId,
    pixel_size: u16,
    foreground: [u8; 4],
) -> Option<RasterizedPayload> {
    if pixel_size == 0 || !face.is_color_glyph(glyph_id) {
        return None;
    }

    let colr = face.tables().colr?;
    if color_glyph_depends_on_foreground(face, glyph_id)? {
        return None;
    }
    let bbox = match colr.clip_box(glyph_id, &[]) {
        Some(cb) => (
            cb.x_min.floor() as i32,
            cb.y_min.floor() as i32,
            cb.x_max.ceil() as i32,
            cb.y_max.ceil() as i32,
        ),
        None => painted_colr_bbox(OutlineSource::FontFace(face), |painter| {
            let fg = RgbaColor::new(
                foreground[0],
                foreground[1],
                foreground[2],
                foreground[3],
            );
            face.paint_color_glyph(glyph_id, 0, fg, painter)
        })
        .or_else(|| {
            face.glyph_bounding_box(glyph_id)
                .map(|bb| bbox_to_i32(rect_to_bbox(bb)))
        })?,
    };
    let upm = face.units_per_em();
    if upm == 0 {
        return None;
    }
    let scale = visual_capped_font_colr_scale(bbox, upm, pixel_size);

    rasterize_colr_paint(
        bbox,
        scale,
        foreground,
        OutlineSource::FontFace(face),
        |raster, fg| face.paint_color_glyph(glyph_id, 0, fg, raster),
    )
}

fn visual_capped_font_colr_scale(
    (x_min, y_min, x_max, y_max): (i32, i32, i32, i32),
    upm: u16,
    pixel_size: u16,
) -> f32 {
    let base_scale = pixel_size as f32 / upm as f32;
    let Some(max_axis_units) = x_max
        .checked_sub(x_min)
        .and_then(|width| y_max.checked_sub(y_min).map(|height| width.max(height)))
    else {
        return base_scale;
    };
    if max_axis_units <= i32::from(upm) {
        return base_scale;
    }

    let cap = pixel_size as f32 / max_axis_units as f32;
    base_scale.min(cap)
}

fn rasterize_colr_paint<'a>(
    (x_min, y_min, x_max, y_max): (i32, i32, i32, i32),
    scale: f32,
    foreground: [u8; 4],
    outlines: OutlineSource<'a>,
    paint: impl FnOnce(&mut ColorRaster<'a>, RgbaColor) -> Option<()>,
) -> Option<RasterizedPayload> {
    let (pix_w, pix_h) = raster_size((x_min, y_min, x_max, y_max), scale)?;

    let base_pixmap = Pixmap::new(pix_w, pix_h)?;
    let base_ctm = Transform::from_row(
        scale,
        0.0,
        0.0,
        -scale,
        -(x_min as f32) * scale + RASTER_PAD,
        (y_max as f32) * scale + RASTER_PAD,
    );

    let mut raster = ColorRaster {
        layers: vec![Layer {
            pixmap: base_pixmap,
            mode: CompositeMode::SourceOver,
        }],
        transforms: vec![base_ctm],
        clips: vec![None],
        current_path: None,
        outlines,
    };

    let fg = RgbaColor::new(foreground[0], foreground[1], foreground[2], foreground[3]);
    paint(&mut raster, fg)?;

    debug_assert_eq!(raster.layers.len(), 1, "layer stack should drain");
    let final_pixmap = raster.layers.pop().unwrap().pixmap;

    Some(RasterizedPayload {
        data: pixmap_to_rgba(&final_pixmap),
        width: pix_w as u16,
        height: pix_h as u16,
        left: (x_min as f32 * scale - RASTER_PAD).floor() as i32,
        top: (y_max as f32 * scale + RASTER_PAD).ceil() as i32,
        is_color: true,
    })
}

fn raster_size(
    (x_min, y_min, x_max, y_max): (i32, i32, i32, i32),
    scale: f32,
) -> Option<(u32, u32)> {
    let width_units = x_max.checked_sub(x_min)?;
    let height_units = y_max.checked_sub(y_min)?;
    Some((
        raster_axis_pixels(width_units, scale)?,
        raster_axis_pixels(height_units, scale)?,
    ))
}

fn raster_axis_pixels(units: i32, scale: f32) -> Option<u32> {
    if units < 0 || !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let pixels = (units as f32 * scale).ceil() + RASTER_PAD * 2.0;
    if !pixels.is_finite() || pixels < 1.0 || pixels > MAX_RASTER_SIDE as f32 {
        return None;
    }
    Some(pixels as u32)
}

fn color_glyph_depends_on_foreground(
    face: &ttf_parser::Face<'_>,
    glyph_id: GlyphId,
) -> Option<bool> {
    let mut first = ForegroundProbePainter::default();
    let mut second = ForegroundProbePainter::default();
    face.paint_color_glyph(glyph_id, 0, RgbaColor::new(13, 37, 59, 251), &mut first)?;
    face.paint_color_glyph(glyph_id, 0, RgbaColor::new(197, 113, 29, 251), &mut second)?;
    Some(first.colors != second.colors)
}

fn colr_payload_depends_on_foreground(
    glyphs: &[Vec<u8>],
    colr_bytes: &[u8],
    cpal_bytes: &[u8],
) -> Option<bool> {
    let cpal_source: &[u8] = if cpal_bytes.is_empty() {
        &EMPTY_CPAL
    } else {
        cpal_bytes
    };
    let cpal = ttf_parser::cpal::Table::parse(cpal_source)?;
    let colr = ttf_parser::colr::Table::parse(cpal, colr_bytes)?;
    let fg = RgbaColor::new(13, 37, 59, 251);
    let (base_gid, _) = select_base_glyph(
        colr_bytes,
        OutlineSource::GlyphProtocol(glyphs),
        |gid, painter| colr.paint(GlyphId(gid), 0, painter, &[], fg),
    )?;
    let mut first = ForegroundProbePainter::default();
    let mut second = ForegroundProbePainter::default();
    colr.paint(
        GlyphId(base_gid),
        0,
        &mut first,
        &[],
        RgbaColor::new(13, 37, 59, 251),
    )?;
    colr.paint(
        GlyphId(base_gid),
        0,
        &mut second,
        &[],
        RgbaColor::new(197, 113, 29, 251),
    )?;
    Some(first.colors != second.colors)
}

#[derive(Default)]
struct ForegroundProbePainter {
    colors: Vec<[u8; 4]>,
}

impl ForegroundProbePainter {
    fn push_color(&mut self, color: RgbaColor) {
        self.colors
            .push([color.red, color.green, color.blue, color.alpha]);
    }

    fn push_stops(&mut self, stops: impl Iterator<Item = ttf_parser::colr::ColorStop>) {
        for stop in stops {
            self.push_color(stop.color);
        }
    }
}

impl<'a> Painter<'a> for ForegroundProbePainter {
    fn outline_glyph(&mut self, _glyph_id: GlyphId) {}

    fn paint(&mut self, paint: Paint<'a>) {
        match paint {
            Paint::Solid(color) => self.push_color(color),
            Paint::LinearGradient(lg) => self.push_stops(lg.stops(0, &[])),
            Paint::RadialGradient(rg) => self.push_stops(rg.stops(0, &[])),
            Paint::SweepGradient(sg) => self.push_stops(sg.stops(0, &[])),
        }
    }

    fn push_clip(&mut self) {}

    fn push_clip_box(&mut self, _clipbox: ClipBox) {}

    fn pop_clip(&mut self) {}

    fn push_layer(&mut self, _mode: CompositeMode) {}

    fn pop_layer(&mut self) {}

    fn push_transform(&mut self, _transform: TtfTransform) {}

    fn pop_transform(&mut self) {}
}

fn face_from_swash_font(font: swash::FontRef<'_>) -> Option<ttf_parser::Face<'_>> {
    let colr = font.table(swash_tag(b"COLR"))?;
    let cpal = font.table(swash_tag(b"CPAL")).unwrap_or(&EMPTY_CPAL);
    let cpal = if cpal.is_empty() { &EMPTY_CPAL } else { cpal };
    let raw_tables = ttf_parser::RawFaceTables {
        head: font.table(swash_tag(b"head"))?,
        hhea: font.table(swash_tag(b"hhea"))?,
        maxp: font.table(swash_tag(b"maxp"))?,
        bdat: font.table(swash_tag(b"bdat")),
        bloc: font.table(swash_tag(b"bloc")),
        cbdt: font.table(swash_tag(b"CBDT")),
        cblc: font.table(swash_tag(b"CBLC")),
        cmap: font.table(swash_tag(b"cmap")),
        colr: Some(colr),
        cpal: Some(cpal),
        cff: font.table(swash_tag(b"CFF ")),
        ebdt: font.table(swash_tag(b"EBDT")),
        eblc: font.table(swash_tag(b"EBLC")),
        glyf: font.table(swash_tag(b"glyf")),
        hmtx: font.table(swash_tag(b"hmtx")),
        kern: font.table(swash_tag(b"kern")),
        loca: font.table(swash_tag(b"loca")),
        name: font.table(swash_tag(b"name")),
        os2: font.table(swash_tag(b"OS/2")),
        post: font.table(swash_tag(b"post")),
        sbix: font.table(swash_tag(b"sbix")),
        stat: font.table(swash_tag(b"STAT")),
        svg: font.table(swash_tag(b"SVG ")),
        vhea: font.table(swash_tag(b"vhea")),
        vmtx: font.table(swash_tag(b"vmtx")),
        vorg: font.table(swash_tag(b"VORG")),
        gdef: font.table(swash_tag(b"GDEF")),
        gpos: font.table(swash_tag(b"GPOS")),
        gsub: font.table(swash_tag(b"GSUB")),
        math: font.table(swash_tag(b"MATH")),
        ankr: font.table(swash_tag(b"ankr")),
        feat: font.table(swash_tag(b"feat")),
        kerx: font.table(swash_tag(b"kerx")),
        morx: font.table(swash_tag(b"morx")),
        trak: font.table(swash_tag(b"trak")),
        avar: font.table(swash_tag(b"avar")),
        cff2: font.table(swash_tag(b"CFF2")),
        fvar: font.table(swash_tag(b"fvar")),
        gvar: font.table(swash_tag(b"gvar")),
        hvar: font.table(swash_tag(b"HVAR")),
        mvar: font.table(swash_tag(b"MVAR")),
        vvar: font.table(swash_tag(b"VVAR")),
        ..ttf_parser::RawFaceTables::default()
    };
    ttf_parser::Face::from_raw_tables(raw_tables).ok()
}

#[inline]
fn swash_tag(tag: &[u8; 4]) -> swash::Tag {
    u32::from_be_bytes(*tag)
}

fn bbox_to_i32(
    (x_min, y_min, x_max, y_max): (f32, f32, f32, f32),
) -> (i32, i32, i32, i32) {
    (
        x_min.floor() as i32,
        y_min.floor() as i32,
        x_max.ceil() as i32,
        y_max.ceil() as i32,
    )
}

fn rect_to_bbox(bb: ttf_parser::Rect) -> (f32, f32, f32, f32) {
    (
        bb.x_min as f32,
        bb.y_min as f32,
        bb.x_max as f32,
        bb.y_max as f32,
    )
}

fn union_bbox(
    a: Option<(f32, f32, f32, f32)>,
    b: (f32, f32, f32, f32),
) -> Option<(f32, f32, f32, f32)> {
    Some(match a {
        Some((ax0, ay0, ax1, ay1)) => {
            (ax0.min(b.0), ay0.min(b.1), ax1.max(b.2), ay1.max(b.3))
        }
        None => b,
    })
}

fn intersect_bbox(
    a: (f32, f32, f32, f32),
    b: (f32, f32, f32, f32),
) -> Option<(f32, f32, f32, f32)> {
    let x_min = a.0.max(b.0);
    let y_min = a.1.max(b.1);
    let x_max = a.2.min(b.2);
    let y_max = a.3.min(b.3);
    (x_min < x_max && y_min < y_max).then_some((x_min, y_min, x_max, y_max))
}

#[derive(Clone, Copy)]
enum OutlineSource<'a> {
    GlyphProtocol(&'a [Vec<u8>]),
    FontFace(&'a ttf_parser::Face<'a>),
}

impl OutlineSource<'_> {
    fn bbox(self, glyph_id: GlyphId) -> Option<(f32, f32, f32, f32)> {
        match self {
            OutlineSource::GlyphProtocol(glyphs) => glyphs
                .get(glyph_id.0 as usize)
                .filter(|bytes| !bytes.is_empty())
                .and_then(|bytes| glyf_bbox(bytes))
                .map(|(x_min, y_min, x_max, y_max)| {
                    (x_min as f32, y_min as f32, x_max as f32, y_max as f32)
                }),
            OutlineSource::FontFace(face) => {
                face.glyph_bounding_box(glyph_id).map(rect_to_bbox)
            }
        }
    }
}

fn valid_bbox(bbox: (f32, f32, f32, f32)) -> Option<(f32, f32, f32, f32)> {
    (bbox.0 < bbox.2 && bbox.1 < bbox.3).then_some(bbox)
}

fn transform_bbox(
    bbox: (f32, f32, f32, f32),
    transform: Transform,
) -> Option<(f32, f32, f32, f32)> {
    let (x_min, y_min, x_max, y_max) = bbox;
    let mut points = [
        Point::from_xy(x_min, y_min),
        Point::from_xy(x_min, y_max),
        Point::from_xy(x_max, y_min),
        Point::from_xy(x_max, y_max),
    ];
    transform.map_points(&mut points);
    let mut out = None;
    for point in points {
        if point.x.is_finite() && point.y.is_finite() {
            out = union_bbox(out, (point.x, point.y, point.x, point.y));
        }
    }
    out
}

fn painted_colr_bbox<'a>(
    outlines: OutlineSource<'a>,
    paint: impl FnOnce(&mut BoundsPainter<'a>) -> Option<()>,
) -> Option<(i32, i32, i32, i32)> {
    let mut painter = BoundsPainter {
        outlines,
        transforms: vec![Transform::identity()],
        clips: vec![BoundsClip::Unbounded],
        current_bbox: None,
        painted_bbox: None,
    };
    paint(&mut painter)?;
    painter.painted_bbox.map(bbox_to_i32)
}

fn select_base_glyph<'a>(
    colr: &[u8],
    outlines: OutlineSource<'a>,
    mut paint: impl FnMut(u16, &mut BoundsPainter<'a>) -> Option<()>,
) -> Option<(u16, Option<(i32, i32, i32, i32)>)> {
    let base_glyphs = base_glyph_ids(colr)?;
    let mut first = None;
    for gid in base_glyphs {
        first.get_or_insert(gid);
        let bbox = painted_colr_bbox(outlines, |painter| paint(gid, painter));
        if bbox.is_some() {
            return Some((gid, bbox));
        }
    }
    first.map(|gid| (gid, None))
}

struct BoundsPainter<'a> {
    outlines: OutlineSource<'a>,
    transforms: Vec<Transform>,
    clips: Vec<BoundsClip>,
    current_bbox: Option<(f32, f32, f32, f32)>,
    painted_bbox: Option<(f32, f32, f32, f32)>,
}

#[derive(Clone, Copy)]
enum BoundsClip {
    Unbounded,
    Empty,
    Bbox((f32, f32, f32, f32)),
}

impl BoundsPainter<'_> {
    fn top_ctm(&self) -> Transform {
        *self.transforms.last().unwrap_or(&Transform::identity())
    }

    fn top_clip(&self) -> BoundsClip {
        *self.clips.last().unwrap_or(&BoundsClip::Unbounded)
    }

    fn push_clip_bbox(&mut self, bbox: Option<(f32, f32, f32, f32)>) {
        let clip = match (self.top_clip(), bbox) {
            (BoundsClip::Empty, _) | (_, None) => BoundsClip::Empty,
            (BoundsClip::Unbounded, Some(bbox)) => BoundsClip::Bbox(bbox),
            (BoundsClip::Bbox(parent), Some(bbox)) => intersect_bbox(parent, bbox)
                .map(BoundsClip::Bbox)
                .unwrap_or(BoundsClip::Empty),
        };
        self.clips.push(clip);
    }

    fn clipped_bbox(&self, bbox: (f32, f32, f32, f32)) -> Option<(f32, f32, f32, f32)> {
        match self.top_clip() {
            BoundsClip::Unbounded => Some(bbox),
            BoundsClip::Empty => None,
            BoundsClip::Bbox(clip) => intersect_bbox(bbox, clip),
        }
    }
}

impl<'a> Painter<'a> for BoundsPainter<'a> {
    fn outline_glyph(&mut self, glyph_id: GlyphId) {
        self.current_bbox = self.outlines.bbox(glyph_id);
    }

    fn paint(&mut self, _paint: Paint<'a>) {
        let Some(bbox) = self.current_bbox else {
            return;
        };
        if let Some(transformed) =
            transform_bbox(bbox, self.top_ctm()).and_then(|bbox| self.clipped_bbox(bbox))
        {
            self.painted_bbox = union_bbox(self.painted_bbox, transformed);
        }
    }

    fn push_clip(&mut self) {
        let transformed = self
            .current_bbox
            .and_then(|bbox| transform_bbox(bbox, self.top_ctm()));
        self.push_clip_bbox(transformed);
    }

    fn push_clip_box(&mut self, clipbox: ClipBox) {
        let bbox = (clipbox.x_min, clipbox.y_min, clipbox.x_max, clipbox.y_max);
        self.push_clip_bbox(
            valid_bbox(bbox).and_then(|bbox| transform_bbox(bbox, self.top_ctm())),
        );
    }

    fn pop_clip(&mut self) {
        if self.clips.len() > 1 {
            self.clips.pop();
        }
    }

    fn push_layer(&mut self, _mode: CompositeMode) {}

    fn pop_layer(&mut self) {}

    fn push_transform(&mut self, transform: TtfTransform) {
        let t = Transform::from_row(
            transform.a,
            transform.b,
            transform.c,
            transform.d,
            transform.e,
            transform.f,
        );
        let ctm = self.top_ctm().pre_concat(t);
        self.transforms.push(ctm);
    }

    fn pop_transform(&mut self) {
        if self.transforms.len() > 1 {
            self.transforms.pop();
        }
    }
}

struct Layer {
    pixmap: Pixmap,
    mode: CompositeMode,
}

struct ColorRaster<'a> {
    layers: Vec<Layer>,
    transforms: Vec<Transform>,
    clips: Vec<Option<Mask>>,
    current_path: Option<Path>,
    outlines: OutlineSource<'a>,
}

impl ColorRaster<'_> {
    fn top_ctm(&self) -> Transform {
        *self.transforms.last().unwrap_or(&Transform::identity())
    }

    fn top_clip(&self) -> Option<&Mask> {
        self.clips.last().and_then(|c| c.as_ref())
    }

    fn top_pixmap(&mut self) -> &mut Pixmap {
        &mut self.layers.last_mut().unwrap().pixmap
    }

    fn fill_current(&mut self, paint: SkPaint) {
        let Some(path) = self.current_path.clone() else {
            return;
        };
        let ctm = self.top_ctm();
        // Clone the clip mask so we can borrow the pixmap mutably.
        let clip = self.top_clip().cloned();
        let pixmap = self.top_pixmap();
        pixmap.fill_path(&path, &paint, FillRule::Winding, ctm, clip.as_ref());
    }
}

impl<'a> Painter<'a> for ColorRaster<'a> {
    fn outline_glyph(&mut self, glyph_id: GlyphId) {
        self.current_path = match self.outlines {
            OutlineSource::GlyphProtocol(glyphs) => {
                glyphs.get(glyph_id.0 as usize).and_then(|bytes| {
                    (!bytes.is_empty()).then(|| build_path(bytes)).flatten()
                })
            }
            OutlineSource::FontFace(face) => build_face_path(face, glyph_id),
        };
    }

    fn paint(&mut self, paint: Paint<'a>) {
        match paint {
            Paint::Solid(color) => {
                let p = SkPaint {
                    shader: Shader::SolidColor(rgba_to_color(color)),
                    anti_alias: true,
                    ..SkPaint::default()
                };
                self.fill_current(p);
            }
            Paint::LinearGradient(lg) => {
                if let Some(shader) = linear_gradient_shader(&lg) {
                    let p = SkPaint {
                        shader,
                        anti_alias: true,
                        ..SkPaint::default()
                    };
                    self.fill_current(p);
                }
            }
            Paint::RadialGradient(rg) => {
                if let Some(shader) = radial_gradient_shader(&rg) {
                    let p = SkPaint {
                        shader,
                        anti_alias: true,
                        ..SkPaint::default()
                    };
                    self.fill_current(p);
                }
            }
            Paint::SweepGradient(sg) => {
                // Sweep gradients don't map to tiny-skia (no sweep
                // shader). Degrade to the first stop's solid colour.
                // Nabla doesn't use sweeps; extending later means
                // writing a custom per-pixel shader.
                if let Some(first) = sg.stops(0, &[]).next() {
                    let p = SkPaint {
                        shader: Shader::SolidColor(rgba_to_color(first.color)),
                        anti_alias: true,
                        ..SkPaint::default()
                    };
                    self.fill_current(p);
                }
            }
        }
    }

    fn push_clip(&mut self) {
        let ctm = self.top_ctm();
        let parent = self.top_clip().cloned();
        let (pw, ph) = {
            let p = self.top_pixmap();
            (p.width(), p.height())
        };
        let Some(mut mask) = Mask::new(pw, ph) else {
            self.clips.push(parent);
            return;
        };
        if let Some(path) = self.current_path.clone() {
            mask.fill_path(&path, FillRule::Winding, true, ctm);
        }
        if let Some(par) = parent {
            intersect_masks(&mut mask, &par);
        }
        self.clips.push(Some(mask));
    }

    fn push_clip_box(&mut self, clipbox: ClipBox) {
        let parent = self.top_clip().cloned();
        let (pw, ph) = {
            let p = self.top_pixmap();
            (p.width(), p.height())
        };
        let Some(rect) =
            Rect::from_ltrb(clipbox.x_min, clipbox.y_min, clipbox.x_max, clipbox.y_max)
        else {
            let Some(mask) = Mask::new(pw, ph) else {
                self.clips.push(parent);
                return;
            };
            self.clips.push(Some(mask));
            return;
        };
        let path = PathBuilder::from_rect(rect);
        let ctm = self.top_ctm();
        let Some(mut mask) = Mask::new(pw, ph) else {
            self.clips.push(parent);
            return;
        };
        mask.fill_path(&path, FillRule::Winding, true, ctm);
        if let Some(par) = parent {
            intersect_masks(&mut mask, &par);
        }
        self.clips.push(Some(mask));
    }

    fn pop_clip(&mut self) {
        if self.clips.len() > 1 {
            self.clips.pop();
        }
    }

    fn push_layer(&mut self, mode: CompositeMode) {
        let (w, h) = {
            let base = self.top_pixmap();
            (base.width(), base.height())
        };
        let Some(pixmap) = Pixmap::new(w, h) else {
            // Out of memory — push a token entry so pop_layer stays
            // balanced. Drawing will fail silently until the pop.
            self.layers.push(Layer {
                pixmap: Pixmap::new(1, 1).unwrap(),
                mode,
            });
            self.clips.push(self.top_clip().cloned());
            return;
        };
        self.layers.push(Layer { pixmap, mode });
        // Layers inherit the enclosing clip. Every push_layer is
        // paired with a pop_layer; we push a matching clip entry so
        // the stack heights stay in lock-step.
        self.clips.push(self.top_clip().cloned());
    }

    fn pop_layer(&mut self) {
        if self.layers.len() <= 1 {
            return;
        }
        let top = self.layers.pop().unwrap();
        let layer_clip = if self.clips.len() > 1 {
            self.clips.pop().flatten()
        } else {
            None
        };
        let blend = composite_mode_to_blend(top.mode);
        let Some(parent) = self.layers.last_mut() else {
            // Stack imbalance — should be unreachable given
            // ttf-parser's own push/pop pairing.
            return;
        };
        parent.pixmap.draw_pixmap(
            0,
            0,
            top.pixmap.as_ref(),
            &PixmapPaint {
                opacity: 1.0,
                blend_mode: blend,
                quality: tiny_skia::FilterQuality::Nearest,
            },
            Transform::identity(),
            layer_clip.as_ref(),
        );
    }

    fn push_transform(&mut self, transform: TtfTransform) {
        let t = Transform::from_row(
            transform.a,
            transform.b,
            transform.c,
            transform.d,
            transform.e,
            transform.f,
        );
        let ctm = self.top_ctm().pre_concat(t);
        self.transforms.push(ctm);
    }

    fn pop_transform(&mut self) {
        if self.transforms.len() > 1 {
            self.transforms.pop();
        }
    }
}

fn read_u16_be(bytes: &[u8], offset: usize) -> Option<u16> {
    let chunk: [u8; 2] = bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?;
    Some(u16::from_be_bytes(chunk))
}

fn read_u32_be(bytes: &[u8], offset: usize) -> Option<u32> {
    let chunk: [u8; 4] = bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_be_bytes(chunk))
}

fn base_glyph_ids(colr: &[u8]) -> Option<Vec<u16>> {
    let version = read_u16_be(colr, 0)?;
    let mut gids = Vec::new();

    // v1 BaseGlyphList: u32 numRecords, then records of
    // { u16 glyphID, u32 paintOffset } = 6 bytes each.
    if version >= 1 {
        let v1_off = read_u32_be(colr, 14).map(|offset| offset as usize);
        if let Some(v1_off) = v1_off.filter(|&offset| offset != 0) {
            let num_records = read_u32_be(colr, v1_off).map(|count| count as usize);
            let records_start = v1_off.checked_add(4);
            if let (Some(num_records), Some(records_start)) = (num_records, records_start)
            {
                for i in 0..num_records {
                    let Some(rec_off) = i
                        .checked_mul(6)
                        .and_then(|delta| records_start.checked_add(delta))
                    else {
                        break;
                    };
                    let Some(record_end) = rec_off.checked_add(6) else {
                        break;
                    };
                    if record_end > colr.len() {
                        break;
                    }
                    let Some(gid) = read_u16_be(colr, rec_off) else {
                        break;
                    };
                    gids.push(gid);
                }
            }
        }
    }

    // v0 BaseGlyphRecord array: { u16 glyphID, u16 firstLayer, u16 numLayers } = 6 B.
    let num_v0 = read_u16_be(colr, 2)? as usize;
    let v0_off = read_u32_be(colr, 4)? as usize;
    if num_v0 != 0 && v0_off != 0 {
        for i in 0..num_v0 {
            let Some(rec_off) =
                i.checked_mul(6).and_then(|delta| v0_off.checked_add(delta))
            else {
                break;
            };
            let Some(record_end) = rec_off.checked_add(6) else {
                break;
            };
            if record_end > colr.len() {
                break;
            }
            let Some(gid) = read_u16_be(colr, rec_off) else {
                break;
            };
            gids.push(gid);
        }
    }
    (!gids.is_empty()).then_some(gids)
}

/// Parse the COLR header's base-glyph records and return the first
/// one whose outline slot in `glyphs` is non-empty.
///
/// Naive "take record 0" doesn't work: fontTools sorts `BaseGlyphList`
/// by glyphID and keeps a `BaseGlyphPaintRecord` for `.notdef` (GID
/// 0), which has an empty outline after subsetting. We need to skip
/// past those empty slots and find the first record that actually
/// has ink. Prefers v1's `BaseGlyphList`, falls back to v0's
/// `BaseGlyphRecord` array.
#[cfg(test)]
fn first_base_glyph_id(colr: &[u8], glyphs: &[Vec<u8>]) -> Option<u16> {
    let base_glyphs = base_glyph_ids(colr)?;
    let first = *base_glyphs.first()?;
    let is_non_empty =
        |gid: u16| -> bool { glyphs.get(gid as usize).is_some_and(|g| !g.is_empty()) };
    base_glyphs
        .into_iter()
        .find(|&gid| is_non_empty(gid))
        .or(Some(first))
}

fn glyf_bbox(bytes: &[u8]) -> Option<(i16, i16, i16, i16)> {
    if bytes.len() < 10 {
        return None;
    }
    let xmin = i16::from_be_bytes([bytes[2], bytes[3]]);
    let ymin = i16::from_be_bytes([bytes[4], bytes[5]]);
    let xmax = i16::from_be_bytes([bytes[6], bytes[7]]);
    let ymax = i16::from_be_bytes([bytes[8], bytes[9]]);
    Some((xmin, ymin, xmax, ymax))
}

/// Decode a glyf simple-glyph record into an unscaled, Y-up
/// design-unit `Path`. The painter's CTM is responsible for the
/// scale + Y-flip at draw time.
///
/// `glyf_decode::Outline::walk` already flips Y so its output sits
/// in Y-down origin-at-y_max space. For the COLR painter we want
/// pristine design-unit Y-up coordinates (so paint-graph transforms
/// compose correctly), so we un-flip walk's output by subtracting
/// from `y_max`. Equivalent to a dedicated y-preserving walker, but
/// reuses `walk`'s existing implied-on-curve handling.
fn build_path(bytes: &[u8]) -> Option<Path> {
    let outline = glyf_decode::decode(bytes).ok()?;
    let y_max = outline.y_max as f32;
    // `walk(upm=1, size=1.0)` gives us an identity scale, so every
    // coord out is `design_x, y_max - design_y`. Un-flip Y below.
    let cmds = outline.walk(1, 1.0);
    if cmds.is_empty() {
        return None;
    }
    let unflip = |y: f32| y_max - y;
    let mut pb = PathBuilder::new();
    for cmd in &cmds {
        match *cmd {
            glyf_decode::PathCmd::MoveTo { x, y } => pb.move_to(x, unflip(y)),
            glyf_decode::PathCmd::LineTo { x, y } => pb.line_to(x, unflip(y)),
            glyf_decode::PathCmd::QuadTo { cx, cy, x, y } => {
                pb.quad_to(cx, unflip(cy), x, unflip(y))
            }
            glyf_decode::PathCmd::Close => pb.close(),
        }
    }
    pb.finish()
}

fn build_face_path(face: &ttf_parser::Face<'_>, glyph_id: GlyphId) -> Option<Path> {
    let mut builder = FacePathBuilder {
        path: PathBuilder::new(),
    };
    face.outline_glyph(glyph_id, &mut builder)?;
    builder.path.finish()
}

struct FacePathBuilder {
    path: PathBuilder,
}

impl OutlineBuilder for FacePathBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to(x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.path.quad_to(x1, y1, x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.path.cubic_to(x1, y1, x2, y2, x, y);
    }

    fn close(&mut self) {
        self.path.close();
    }
}

/// tiny-skia Pixmap pixels are premultiplied. The v4 grid color atlas
/// expects premultiplied RGBA — its Metal/wgpu/vulkan pipelines all
/// configure source-blend = `One`, dest-blend = `OneMinusSourceAlpha`,
/// matching the system-emoji rasteriser's premultiplied output. So pass
/// the bytes through verbatim. (The previous PR plumbed COLR glyphs
/// through the rich-text image-cache, whose pipeline used `SourceAlpha +
/// OneMinusSourceAlpha` and therefore wanted straight alpha — that path
/// no longer exists in main.)
fn pixmap_to_rgba(pixmap: &Pixmap) -> Vec<u8> {
    let pixels = pixmap.pixels();
    let mut out = Vec::with_capacity(pixels.len() * 4);
    for p in pixels {
        out.push(p.red());
        out.push(p.green());
        out.push(p.blue());
        out.push(p.alpha());
    }
    out
}

fn rgba_to_color(c: RgbaColor) -> Color {
    Color::from_rgba8(c.red, c.green, c.blue, c.alpha)
}

/// Collect + sort COLR stops. Returns the raw `(offset, color)`
/// pairs so the caller can still pull the first stop's colour for
/// single-stop degeneracy (tiny-skia's `GradientStop` fields are
/// `pub(crate)` and don't expose the colour back).
fn collect_stops(
    iter: impl Iterator<Item = ttf_parser::colr::ColorStop>,
) -> Vec<(f32, Color)> {
    let mut stops: Vec<(f32, Color)> = iter
        .map(|s| (s.stop_offset, rgba_to_color(s.color)))
        .collect();
    stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    stops
}

fn stops_to_tiny_skia(stops: &[(f32, Color)]) -> Vec<GradientStop> {
    stops
        .iter()
        .map(|&(o, c)| GradientStop::new(o, c))
        .collect()
}

fn extend_to_spread(e: GradientExtend) -> SpreadMode {
    match e {
        GradientExtend::Pad => SpreadMode::Pad,
        GradientExtend::Repeat => SpreadMode::Repeat,
        GradientExtend::Reflect => SpreadMode::Reflect,
    }
}

/// Project COLR's 3-point linear-gradient form to the 2-point form
/// tiny-skia wants. Returns `P3`, the point on the perpendicular to
/// `P0→P2` through `P0` that corresponds to `P1`.
///
/// Two equivalent formulations of the same geometry are in the wild:
///
/// - **skrifa/nanoemoji**: `P3 = P0 + project(P1 - P0, perp(P2 - P0))`
///   — project onto the perpendicular axis, add back to P0.
/// - **This impl**: `P3 = P1 - t * (P2 - P0)` where
///   `t = ((P1 - P0) · (P2 - P0)) / |P2 - P0|²`
///   — remove the parallel component from `(P1 - P0)` and add P0.
///
/// Algebraically these give the same point: subtracting the parallel
/// component of `(P1 - P0)` leaves its perpendicular component, and
/// `P0 + perp_component = P1 - parallel_component`. The FreeType
/// COLRv1 reference implementation uses the second formulation.
///
/// Returns `None` if `P0 == P2` (degenerate axis with no direction).
fn project_p3(p0: (f32, f32), p1: (f32, f32), p2: (f32, f32)) -> Option<(f32, f32)> {
    let dx = p2.0 - p0.0;
    let dy = p2.1 - p0.1;
    let len_sq = dx * dx + dy * dy;
    if !len_sq.is_finite() || len_sq < 1e-6 {
        return None;
    }
    let bx = p1.0 - p0.0;
    let by = p1.1 - p0.1;
    let t = (bx * dx + by * dy) / len_sq;
    Some((p1.0 - t * dx, p1.1 - t * dy))
}

/// Build a tiny-skia linear-gradient shader for a COLR linear paint.
/// The 3-point → 2-point projection happens via [`project_p3`].
///
/// Stop normalisation (extending the P0-P3 line when stops sit
/// outside `[0, 1]`) is NOT performed: tiny-skia clamps stop offsets
/// to `[0, 1]`, so a gradient with stops at e.g. `-0.2`..`1.2`
/// renders truncated at the boundaries. Nabla's stops sit within
/// `[0, 1]` so this hasn't bitten the demo. Handling wide stops
/// would mean moving `P0` and `P3` outward by the offset overhang
/// and rescaling stops to fit `[0, 1]`; skrifa's traversal.rs has
/// the full math.
fn linear_gradient_shader(lg: &TtfLinear<'_>) -> Option<Shader<'static>> {
    let p0 = (lg.x0, lg.y0);
    let p1 = (lg.x1, lg.y1);
    let p2 = (lg.x2, lg.y2);
    let (p3x, p3y) = project_p3(p0, p1, p2)?;

    let stops = collect_stops(lg.stops(0, &[]));
    if stops.len() < 2 {
        return stops.into_iter().next().map(|(_, c)| Shader::SolidColor(c));
    }

    LinearGradient::new(
        Point::from_xy(p0.0, p0.1),
        Point::from_xy(p3x, p3y),
        stops_to_tiny_skia(&stops),
        extend_to_spread(lg.extend),
        Transform::identity(),
    )
}

fn radial_gradient_shader(rg: &TtfRadial<'_>) -> Option<Shader<'static>> {
    let stops = collect_stops(rg.stops(0, &[]));
    if stops.len() < 2 {
        return stops.into_iter().next().map(|(_, c)| Shader::SolidColor(c));
    }
    RadialGradient::new(
        Point::from_xy(rg.x0, rg.y0),
        rg.r0.max(0.0),
        Point::from_xy(rg.x1, rg.y1),
        rg.r1.max(0.1),
        stops_to_tiny_skia(&stops),
        extend_to_spread(rg.extend),
        Transform::identity(),
    )
}

fn composite_mode_to_blend(mode: CompositeMode) -> BlendMode {
    use CompositeMode::*;
    match mode {
        Clear => BlendMode::Clear,
        Source => BlendMode::Source,
        Destination => BlendMode::Destination,
        SourceOver => BlendMode::SourceOver,
        DestinationOver => BlendMode::DestinationOver,
        SourceIn => BlendMode::SourceIn,
        DestinationIn => BlendMode::DestinationIn,
        SourceOut => BlendMode::SourceOut,
        DestinationOut => BlendMode::DestinationOut,
        SourceAtop => BlendMode::SourceAtop,
        DestinationAtop => BlendMode::DestinationAtop,
        Xor => BlendMode::Xor,
        Plus => BlendMode::Plus,
        Screen => BlendMode::Screen,
        Overlay => BlendMode::Overlay,
        Darken => BlendMode::Darken,
        Lighten => BlendMode::Lighten,
        ColorDodge => BlendMode::ColorDodge,
        ColorBurn => BlendMode::ColorBurn,
        HardLight => BlendMode::HardLight,
        SoftLight => BlendMode::SoftLight,
        Difference => BlendMode::Difference,
        Exclusion => BlendMode::Exclusion,
        Multiply => BlendMode::Multiply,
        Hue => BlendMode::Hue,
        Saturation => BlendMode::Saturation,
        Color => BlendMode::Color,
        Luminosity => BlendMode::Luminosity,
    }
}

/// Intersect two 8-bit alpha masks in place: `dst = dst ∩ src`.
/// Used when pushing nested clips — the new clip region is the
/// logical intersection of the outer and inner paths. Both masks
/// share dimensions by construction (we always build them from
/// the current pixmap's size).
fn intersect_masks(dst: &mut Mask, src: &Mask) {
    if dst.width() != src.width() || dst.height() != src.height() {
        return;
    }
    let dst_bytes = dst.data_mut();
    let src_bytes = src.data();
    for (d, &s) in dst_bytes.iter_mut().zip(src_bytes.iter()) {
        *d = ((*d as u16 * s as u16) / 255) as u8;
    }
}

#[cfg(test)]
mod tests {
    // Test lane: default

    use super::*;

    /// Helper: vector dot product in 2D.
    fn dot(a: (f32, f32), b: (f32, f32)) -> f32 {
        a.0 * b.0 + a.1 * b.1
    }

    /// Build a `glyf` simple-glyph whose bbox is the full `em_top × em_top`
    /// square but whose inked region is just the top `strip_height` rows
    /// (design Y from `em_top - strip_height` to `em_top`). The bbox in
    /// the glyf header is authoritative — `glyf_decode` reads it directly
    /// — so the rasterised pixmap will be em_top×em_top pixels with only
    /// the top strip filled. That's what lets the test distinguish "top"
    /// from "bottom" of the bitmap.
    fn glyf_top_strip(em_top: i16, strip_height: i16) -> Vec<u8> {
        // glyf simple-glyph layout per OpenType:
        //   i16 numberOfContours
        //   i16 xMin, yMin, xMax, yMax  (authoritative bbox — NOT derived from points)
        //   u16 endPtsOfContours[numContours]
        //   u16 instructionLength
        //   u8  flags[numPoints]
        //   coords (deltas, big-endian i16 when not using shorts)
        let strip_bottom = em_top - strip_height;
        let mut v = Vec::new();
        v.extend_from_slice(&1i16.to_be_bytes()); // numberOfContours
                                                  // Declare bbox as the full em — not just the inked strip — so
                                                  // the rasterised pixmap has empty space below the strip we
                                                  // can sample as "bottom".
        v.extend_from_slice(&0i16.to_be_bytes()); // xMin
        v.extend_from_slice(&0i16.to_be_bytes()); // yMin
        v.extend_from_slice(&em_top.to_be_bytes()); // xMax
        v.extend_from_slice(&em_top.to_be_bytes()); // yMax
        v.extend_from_slice(&3u16.to_be_bytes()); // endPtsOfContours[0] = 3 (4 points)
        v.extend_from_slice(&0u16.to_be_bytes()); // instructionLength = 0
        v.extend_from_slice(&[0x01; 4]); // 4 flags, all on-curve, full i16 deltas
                                         // Points walk the rectangle [0, strip_bottom] → [em_top, strip_bottom]
                                         // → [em_top, em_top] → [0, em_top]. Deltas from previous point
                                         // (first delta is from origin (0,0)).
        let xs = [0i16, em_top, 0, -em_top];
        let ys = [strip_bottom, 0, strip_height, 0];
        for x in &xs {
            v.extend_from_slice(&x.to_be_bytes());
        }
        for y in &ys {
            v.extend_from_slice(&y.to_be_bytes());
        }
        v
    }

    fn build_head() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&1u16.to_be_bytes()); // majorVersion
        v.extend_from_slice(&0u16.to_be_bytes()); // minorVersion
        v.extend_from_slice(&0x0001_0000u32.to_be_bytes()); // fontRevision
        v.extend_from_slice(&0u32.to_be_bytes()); // checkSumAdjustment, patched later
        v.extend_from_slice(&0x5F0F_3CF5u32.to_be_bytes()); // magicNumber
        v.extend_from_slice(&0u16.to_be_bytes()); // flags
        v.extend_from_slice(&100u16.to_be_bytes()); // unitsPerEm
        v.extend_from_slice(&0i64.to_be_bytes()); // created
        v.extend_from_slice(&0i64.to_be_bytes()); // modified
        v.extend_from_slice(&0i16.to_be_bytes()); // xMin
        v.extend_from_slice(&0i16.to_be_bytes()); // yMin
        v.extend_from_slice(&100i16.to_be_bytes()); // xMax
        v.extend_from_slice(&100i16.to_be_bytes()); // yMax
        v.extend_from_slice(&0u16.to_be_bytes()); // macStyle
        v.extend_from_slice(&8u16.to_be_bytes()); // lowestRecPPEM
        v.extend_from_slice(&2i16.to_be_bytes()); // fontDirectionHint
        v.extend_from_slice(&0i16.to_be_bytes()); // indexToLocFormat = short
        v.extend_from_slice(&0i16.to_be_bytes()); // glyphDataFormat
        v
    }

    fn build_hhea() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&1u16.to_be_bytes()); // majorVersion
        v.extend_from_slice(&0u16.to_be_bytes()); // minorVersion
        v.extend_from_slice(&100i16.to_be_bytes()); // ascender
        v.extend_from_slice(&0i16.to_be_bytes()); // descender
        v.extend_from_slice(&0i16.to_be_bytes()); // lineGap
        v.extend_from_slice(&100u16.to_be_bytes()); // advanceWidthMax
        v.extend_from_slice(&0i16.to_be_bytes()); // minLeftSideBearing
        v.extend_from_slice(&0i16.to_be_bytes()); // minRightSideBearing
        v.extend_from_slice(&100i16.to_be_bytes()); // xMaxExtent
        v.extend_from_slice(&1i16.to_be_bytes()); // caretSlopeRise
        v.extend_from_slice(&0i16.to_be_bytes()); // caretSlopeRun
        v.extend_from_slice(&0i16.to_be_bytes()); // caretOffset
        for _ in 0..4 {
            v.extend_from_slice(&0i16.to_be_bytes());
        }
        v.extend_from_slice(&0i16.to_be_bytes()); // metricDataFormat
        v.extend_from_slice(&3u16.to_be_bytes()); // numberOfHMetrics
        v
    }

    fn build_maxp() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&0x0001_0000u32.to_be_bytes()); // version
        v.extend_from_slice(&3u16.to_be_bytes()); // numGlyphs
        v.extend_from_slice(&4u16.to_be_bytes()); // maxPoints
        v.extend_from_slice(&1u16.to_be_bytes()); // maxContours
        for value in [0u16, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0] {
            v.extend_from_slice(&value.to_be_bytes());
        }
        v
    }

    fn build_hmtx() -> Vec<u8> {
        let mut v = Vec::new();
        for _ in 0..3 {
            v.extend_from_slice(&100u16.to_be_bytes());
            v.extend_from_slice(&0i16.to_be_bytes());
        }
        v
    }

    fn build_loca(glyf_len: usize) -> Vec<u8> {
        let mut v = Vec::new();
        for offset in [0u16, 0, 0, (glyf_len / 2) as u16] {
            v.extend_from_slice(&offset.to_be_bytes());
        }
        v
    }

    fn build_colr_layer_only(version: u16) -> Vec<u8> {
        build_colr_layer_only_with_palette(version, 0)
    }

    fn build_colr_layer_only_with_palette(version: u16, palette_index: u16) -> Vec<u8> {
        let base_records_offset: u32 = if version == 0 { 14 } else { 34 };
        let layer_records_offset = base_records_offset + 6;
        let mut v = Vec::new();
        v.extend_from_slice(&version.to_be_bytes());
        v.extend_from_slice(&1u16.to_be_bytes()); // numBaseGlyphRecords
        v.extend_from_slice(&base_records_offset.to_be_bytes());
        v.extend_from_slice(&layer_records_offset.to_be_bytes());
        v.extend_from_slice(&1u16.to_be_bytes()); // numLayerRecords

        if version == 1 {
            // v1 header tail: BaseGlyphList at the end, no v1 layer
            // list, no clip list, no variation index map/store.
            v.extend_from_slice(&(layer_records_offset + 4).to_be_bytes());
            v.extend_from_slice(&0u32.to_be_bytes());
            v.extend_from_slice(&0u32.to_be_bytes());
            v.extend_from_slice(&0u32.to_be_bytes());
            v.extend_from_slice(&0u32.to_be_bytes());
        }

        v.extend_from_slice(&1u16.to_be_bytes()); // base glyph id
        v.extend_from_slice(&0u16.to_be_bytes()); // firstLayerIndex
        v.extend_from_slice(&1u16.to_be_bytes()); // numLayers
        v.extend_from_slice(&2u16.to_be_bytes()); // layer glyph id
        v.extend_from_slice(&palette_index.to_be_bytes()); // palette index

        if version == 1 {
            v.extend_from_slice(&0u32.to_be_bytes()); // empty v1 BaseGlyphList
        }

        v
    }

    fn build_colr_v0_notdef_then_layer_only(palette_index: u16) -> Vec<u8> {
        let base_records_offset = 14u32;
        let layer_records_offset = base_records_offset + 12;
        let mut v = Vec::new();
        v.extend_from_slice(&0u16.to_be_bytes()); // version
        v.extend_from_slice(&2u16.to_be_bytes()); // numBaseGlyphRecords
        v.extend_from_slice(&base_records_offset.to_be_bytes());
        v.extend_from_slice(&layer_records_offset.to_be_bytes());
        v.extend_from_slice(&2u16.to_be_bytes()); // numLayerRecords

        v.extend_from_slice(&0u16.to_be_bytes()); // .notdef base glyph id
        v.extend_from_slice(&0u16.to_be_bytes()); // firstLayerIndex
        v.extend_from_slice(&1u16.to_be_bytes()); // numLayers
        v.extend_from_slice(&1u16.to_be_bytes()); // real wrapper base glyph id
        v.extend_from_slice(&1u16.to_be_bytes()); // firstLayerIndex
        v.extend_from_slice(&1u16.to_be_bytes()); // numLayers

        v.extend_from_slice(&0u16.to_be_bytes()); // .notdef layer glyph id
        v.extend_from_slice(&0u16.to_be_bytes()); // fixed palette index
        v.extend_from_slice(&2u16.to_be_bytes()); // painted layer glyph id
        v.extend_from_slice(&palette_index.to_be_bytes());

        v
    }

    fn build_colr_v1_paint_glyph_no_clip() -> Vec<u8> {
        let base_list_offset = 34u32;
        let paint_offset = 10u32;
        let mut v = Vec::new();
        v.extend_from_slice(&1u16.to_be_bytes()); // version
        v.extend_from_slice(&0u16.to_be_bytes()); // numBaseGlyphRecords
        v.extend_from_slice(&0u32.to_be_bytes()); // baseGlyphRecordsOffset
        v.extend_from_slice(&0u32.to_be_bytes()); // layerRecordsOffset
        v.extend_from_slice(&0u16.to_be_bytes()); // numLayerRecords
        v.extend_from_slice(&base_list_offset.to_be_bytes());
        v.extend_from_slice(&0u32.to_be_bytes()); // layerListOffset
        v.extend_from_slice(&0u32.to_be_bytes()); // clipListOffset
        v.extend_from_slice(&0u32.to_be_bytes()); // varIndexMapOffset
        v.extend_from_slice(&0u32.to_be_bytes()); // itemVariationStoreOffset

        v.extend_from_slice(&1u32.to_be_bytes()); // BaseGlyphList count
        v.extend_from_slice(&1u16.to_be_bytes()); // base glyph id
        v.extend_from_slice(&paint_offset.to_be_bytes());

        v.push(10); // PaintGlyph
        v.extend_from_slice(&6u32.to_be_bytes()[1..]); // child paint offset
        v.extend_from_slice(&2u16.to_be_bytes()); // painted layer glyph id
        v.push(2); // PaintSolid
        v.extend_from_slice(&0u16.to_be_bytes()); // palette index
        v.extend_from_slice(&0x4000u16.to_be_bytes()); // alpha = 1.0
        v
    }

    fn build_colr_v1_missing_outer_clip_nested_paint_glyph() -> Vec<u8> {
        let base_list_offset = 34u32;
        let paint_offset = 10u32;
        let mut v = Vec::new();
        v.extend_from_slice(&1u16.to_be_bytes()); // version
        v.extend_from_slice(&0u16.to_be_bytes()); // numBaseGlyphRecords
        v.extend_from_slice(&0u32.to_be_bytes()); // baseGlyphRecordsOffset
        v.extend_from_slice(&0u32.to_be_bytes()); // layerRecordsOffset
        v.extend_from_slice(&0u16.to_be_bytes()); // numLayerRecords
        v.extend_from_slice(&base_list_offset.to_be_bytes());
        v.extend_from_slice(&0u32.to_be_bytes()); // layerListOffset
        v.extend_from_slice(&0u32.to_be_bytes()); // clipListOffset
        v.extend_from_slice(&0u32.to_be_bytes()); // varIndexMapOffset
        v.extend_from_slice(&0u32.to_be_bytes()); // itemVariationStoreOffset

        v.extend_from_slice(&1u32.to_be_bytes()); // BaseGlyphList count
        v.extend_from_slice(&1u16.to_be_bytes()); // base glyph id
        v.extend_from_slice(&paint_offset.to_be_bytes());

        v.push(10); // outer PaintGlyph
        v.extend_from_slice(&6u32.to_be_bytes()[1..]); // nested child paint offset
        v.extend_from_slice(&3u16.to_be_bytes()); // missing outer clip glyph id
        v.push(10); // nested PaintGlyph
        v.extend_from_slice(&6u32.to_be_bytes()[1..]); // solid child paint offset
        v.extend_from_slice(&2u16.to_be_bytes()); // existing nested glyph id
        v.push(2); // PaintSolid
        v.extend_from_slice(&0u16.to_be_bytes()); // palette index
        v.extend_from_slice(&0x4000u16.to_be_bytes()); // alpha = 1.0
        v
    }

    fn build_cpal_red() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&0u16.to_be_bytes()); // version
        v.extend_from_slice(&1u16.to_be_bytes()); // numPaletteEntries
        v.extend_from_slice(&1u16.to_be_bytes()); // numPalettes
        v.extend_from_slice(&1u16.to_be_bytes()); // numColorRecords
        v.extend_from_slice(&14u32.to_be_bytes()); // colorRecordsArrayOffset
        v.extend_from_slice(&0u16.to_be_bytes()); // colorRecordIndices[0]
        v.extend_from_slice(&[0x00, 0x00, 0xFF, 0xFF]); // BGRA red
        v
    }

    fn checksum(data: &[u8]) -> u32 {
        data.chunks(4).fold(0u32, |sum, chunk| {
            let mut padded = [0u8; 4];
            padded[..chunk.len()].copy_from_slice(chunk);
            sum.wrapping_add(u32::from_be_bytes(padded))
        })
    }

    fn pad4(v: &mut Vec<u8>) {
        while v.len() % 4 != 0 {
            v.push(0);
        }
    }

    fn build_sfnt(mut tables: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
        tables.sort_by_key(|(tag, _)| *tag);
        let num_tables = tables.len() as u16;
        let mut search_power = 1u16;
        let mut entry_selector = 0u16;
        while search_power.saturating_mul(2) <= num_tables {
            search_power *= 2;
            entry_selector += 1;
        }
        let search_range = search_power * 16;
        let range_shift = num_tables * 16 - search_range;

        let mut records = Vec::new();
        let mut offset = 12 + tables.len() * 16;
        for (tag, data) in &tables {
            records.push((*tag, checksum(data), offset as u32, data.len() as u32));
            offset += data.len();
            offset = (offset + 3) & !3;
        }

        let mut font = Vec::new();
        font.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        font.extend_from_slice(&num_tables.to_be_bytes());
        font.extend_from_slice(&search_range.to_be_bytes());
        font.extend_from_slice(&entry_selector.to_be_bytes());
        font.extend_from_slice(&range_shift.to_be_bytes());
        for (tag, sum, table_offset, len) in &records {
            font.extend_from_slice(tag);
            font.extend_from_slice(&sum.to_be_bytes());
            font.extend_from_slice(&table_offset.to_be_bytes());
            font.extend_from_slice(&len.to_be_bytes());
        }
        for (_, data) in &tables {
            font.extend_from_slice(data);
            pad4(&mut font);
        }

        let head_offset = records
            .iter()
            .find_map(|(tag, _, table_offset, _)| {
                (*tag == *b"head").then_some(*table_offset)
            })
            .expect("test font has head table") as usize;
        let adjustment = 0xB1B0_AFBAu32.wrapping_sub(checksum(&font));
        font[head_offset + 8..head_offset + 12]
            .copy_from_slice(&adjustment.to_be_bytes());
        font
    }

    fn build_minimal_font_with_colr_and_layer_glyf(
        colr: Vec<u8>,
        mut glyf: Vec<u8>,
    ) -> Vec<u8> {
        if glyf.len() % 2 != 0 {
            glyf.push(0);
        }

        build_sfnt(vec![
            (*b"COLR", colr),
            (*b"CPAL", build_cpal_red()),
            (*b"glyf", glyf.clone()),
            (*b"head", build_head()),
            (*b"hhea", build_hhea()),
            (*b"hmtx", build_hmtx()),
            (*b"loca", build_loca(glyf.len())),
            (*b"maxp", build_maxp()),
        ])
    }

    fn build_minimal_font_with_colr(colr: Vec<u8>) -> Vec<u8> {
        build_minimal_font_with_colr_and_layer_glyf(colr, glyf_top_strip(100, 100))
    }

    fn build_minimal_colr_font(colr_version: u16) -> Vec<u8> {
        build_minimal_font_with_colr(build_colr_layer_only(colr_version))
    }

    #[test]
    fn raster_axis_pixels_rejects_dimensions_that_cannot_roundtrip_to_payload() {
        // Regression: rasterized payload dimensions are u16. A plain
        // `as u16` cast silently truncated oversized tiny-skia pixmap
        // dimensions after allocation.
        assert!(raster_axis_pixels(u16::MAX as i32 + 1, 1.0).is_none());
    }

    #[test]
    fn rasterize_colr_paint_rejects_oversized_bounds_before_painting() {
        // Regression: malformed or transformed COLR bounds larger
        // than the payload/atlas can represent must fail before
        // painting, not allocate and return truncated dimensions.
        let glyphs: &[Vec<u8>] = &[];
        let raster = rasterize_colr_paint(
            (0, 0, u16::MAX as i32 + 1, 0),
            1.0,
            [255, 255, 255, 255],
            OutlineSource::GlyphProtocol(glyphs),
            |_, _| Some(()),
        );
        assert!(raster.is_none());
    }

    #[test]
    fn color_raster_empty_clip_blocks_later_outline_paint() {
        // Regression: PaintGlyph clips child paints to the current
        // outline. A missing outline is an empty clip, not "keep the
        // parent clip", otherwise nested child glyphs can leak ink.
        let glyphs = vec![glyf_top_strip(100, 100)];
        let mut raster = ColorRaster {
            layers: vec![Layer {
                pixmap: Pixmap::new(16, 16).expect("pixmap allocates"),
                mode: CompositeMode::SourceOver,
            }],
            transforms: vec![Transform::identity()],
            clips: vec![None],
            current_path: None,
            outlines: OutlineSource::GlyphProtocol(&glyphs),
        };

        raster.push_clip();
        raster.outline_glyph(GlyphId(0));
        raster.paint(Paint::Solid(RgbaColor::new(255, 0, 0, 255)));
        raster.pop_clip();

        assert!(
            raster.layers[0]
                .pixmap
                .pixels()
                .iter()
                .all(|pixel| pixel.alpha() == 0),
            "empty clip should suppress later child paint"
        );
    }

    #[test]
    fn color_raster_layer_composite_respects_inherited_clip() {
        // Regression: a pushed COLR layer inherits the current clip.
        // Destructive blend modes like Clear must keep that clip when
        // the layer is composited back, otherwise transparent source
        // pixels outside the clip can erase the whole destination.
        let glyphs: &[Vec<u8>] = &[];
        let mut base = Pixmap::new(6, 6).expect("pixmap allocates");
        base.fill(Color::from_rgba8(255, 0, 0, 255));
        let mut raster = ColorRaster {
            layers: vec![Layer {
                pixmap: base,
                mode: CompositeMode::SourceOver,
            }],
            transforms: vec![Transform::identity()],
            clips: vec![None],
            current_path: None,
            outlines: OutlineSource::GlyphProtocol(glyphs),
        };

        raster.push_clip_box(ClipBox {
            x_min: 1.0,
            y_min: 1.0,
            x_max: 4.0,
            y_max: 4.0,
        });
        raster.push_layer(CompositeMode::Clear);
        raster.pop_layer();
        raster.pop_clip();

        let outside = raster.layers[0].pixmap.pixel(0, 0).expect("outside pixel");
        let inside = raster.layers[0].pixmap.pixel(2, 2).expect("inside pixel");
        assert_eq!(outside.alpha(), 255, "outside clip should stay opaque");
        assert_eq!(inside.alpha(), 0, "inside clip should be cleared");
    }

    #[test]
    fn rasterize_payload_handles_colr_v0_layer_bbox_without_base_outline() {
        // Regression: Glyph Protocol COLR payloads can mirror normal
        // COLR fonts where the base glyph is an empty wrapper and all
        // ink lives in layer glyphs. Without the paint-graph bbox
        // prepass, no-ClipBox payloads fell back to the empty base
        // glyph bbox and produced tofu.
        let payload = crate::font::glyph_registry::StoredPayload::ColrV0 {
            glyphs: vec![vec![], vec![], glyf_top_strip(100, 100)],
            colr: build_colr_layer_only(0),
            cpal: build_cpal_red(),
        };

        let raster = rasterize_payload(&payload, 100, 32, [255, 255, 255, 255])
            .expect("COLR v0 layer-only Glyph Protocol payload rasterizes");

        assert!(raster.is_color);
        assert!(raster.data.chunks_exact(4).any(|p| p[3] > 0));
    }

    #[test]
    fn payload_depends_on_foreground_tracks_colr_palette_index() {
        // Regression: foreground-dependent Glyph Protocol color glyphs
        // need a foreground-aware atlas key, while fixed CPAL payloads
        // should keep sharing one pre-painted color atlas entry.
        let glyphs = || vec![vec![], vec![], glyf_top_strip(100, 100)];
        let fixed = crate::font::glyph_registry::StoredPayload::ColrV0 {
            glyphs: glyphs(),
            colr: build_colr_layer_only_with_palette(0, 0),
            cpal: build_cpal_red(),
        };
        let foreground = crate::font::glyph_registry::StoredPayload::ColrV0 {
            glyphs: glyphs(),
            colr: build_colr_layer_only_with_palette(0, u16::MAX),
            cpal: build_cpal_red(),
        };

        assert!(!payload_depends_on_foreground(&fixed));
        assert!(payload_depends_on_foreground(&foreground));
    }

    #[test]
    fn glyph_protocol_colr_skips_unpaintable_notdef_base_record() {
        // Regression: font-subset COLR payloads can retain a leading
        // empty .notdef base record before the registered glyph's empty
        // wrapper base. Base-glyph selection must walk the paint graph,
        // not only test the base glyph's own outline bytes.
        let payload = crate::font::glyph_registry::StoredPayload::ColrV0 {
            glyphs: vec![vec![], vec![], glyf_top_strip(100, 100)],
            colr: build_colr_v0_notdef_then_layer_only(u16::MAX),
            cpal: build_cpal_red(),
        };

        assert!(payload_depends_on_foreground(&payload));

        let raster = rasterize_payload(&payload, 100, 32, [7, 47, 113, 255])
            .expect("COLR payload selects the paintable base record");

        assert!(raster.is_color);
        assert!(raster.data.chunks_exact(4).any(|p| p[3] > 0));
    }

    #[test]
    fn rasterize_font_glyph_handles_colr_v0_layer_bbox_without_base_outline() {
        // Regression: normal-font COLR v0 glyphs can have an empty
        // base glyph and put all ink in layer glyphs. Without deriving
        // the bbox from layers, the new swash-font path returned None
        // before it reached ttf-parser's COLR painter.
        let font_data = build_minimal_colr_font(0);
        let font = swash::FontRef::from_index(&font_data, 0).expect("test font parses");
        let face = face_from_swash_font(font).expect("COLR face parses");
        assert!(face.glyph_bounding_box(GlyphId(1)).is_none());
        assert!(face.glyph_bounding_box(GlyphId(2)).is_some());
        assert!(!color_glyph_depends_on_foreground(&face, GlyphId(1)).unwrap());

        let raster = rasterize_font_glyph(font, 1, 32, [255, 255, 255, 255])
            .expect("COLR v0 layer-only glyph rasterizes");

        assert!(raster.is_color);
        assert!(raster.width >= 32);
        assert!(raster.height >= 32);
        let alpha_pixels = raster.data.chunks_exact(4).filter(|p| p[3] > 0).count();
        let red_pixels = raster
            .data
            .chunks_exact(4)
            .filter(|p| p[3] > 0 && p[0] > p[2])
            .count();
        assert!(alpha_pixels > 0, "raster should contain ink");
        assert!(red_pixels > 0, "raster should use CPAL red");
    }

    #[test]
    fn rasterize_font_glyph_caps_oversized_colr_bbox_to_visual_size() {
        // Regression: some packaged emoji fonts have COLR layer bounds
        // much larger than one em. Terminal emoji rasterization should
        // fit those glyphs back into the requested visual budget instead
        // of letting them dominate the row.
        let font_data = build_minimal_font_with_colr_and_layer_glyf(
            build_colr_layer_only(0),
            glyf_top_strip(300, 300),
        );
        let font = swash::FontRef::from_index(&font_data, 0).expect("test font parses");

        let raster = rasterize_font_glyph(font, 1, 32, [255, 255, 255, 255])
            .expect("oversized COLR glyph rasterizes");

        assert!(raster.is_color);
        assert!(
            raster.width <= 34 && raster.height <= 34,
            "32 px visual budget plus 1 px raster pad on each side should cap oversized bounds, got {}x{}",
            raster.width,
            raster.height
        );
        assert!(raster.data.chunks_exact(4).any(|p| p[3] > 0));
    }

    #[test]
    fn rasterize_font_glyph_skips_foreground_dependent_colr() {
        // Regression: normal-font COLR glyphs are cached by
        // font/glyph/size only. If a graph uses paletteIndex 0xFFFF,
        // pre-painting it into the color atlas would freeze the first
        // foreground color for every later cell using that glyph.
        let font_data =
            build_minimal_font_with_colr(build_colr_layer_only_with_palette(0, u16::MAX));
        let font = swash::FontRef::from_index(&font_data, 0).expect("test font parses");
        let face = face_from_swash_font(font).expect("COLR face parses");

        assert!(color_glyph_depends_on_foreground(&face, GlyphId(1)).unwrap());
        assert!(
            rasterize_font_glyph(font, 1, 32, [255, 255, 255, 255]).is_none(),
            "foreground-dependent COLR must fall back to the existing swash path"
        );
    }

    #[test]
    fn rasterize_font_glyph_handles_colr_v1_table_with_v0_layer_records() {
        // Regression: COLR v1 tables can still contain v0 base/layer
        // records. ttf-parser falls back to those when no v1 record
        // exists for the glyph, so the bbox fallback must accept
        // table version 1 instead of rejecting it as non-v0.
        let font_data = build_minimal_colr_font(1);
        let font = swash::FontRef::from_index(&font_data, 0).expect("test font parses");

        let raster = rasterize_font_glyph(font, 1, 32, [255, 255, 255, 255])
            .expect("COLR v1 table with v0 records rasterizes");

        assert!(raster.is_color);
        assert!(raster.data.chunks_exact(4).any(|p| p[3] > 0));
    }

    #[test]
    fn rasterize_font_glyph_respects_missing_paintglyph_clip_outline() {
        // Regression: the bounds prepass must follow PaintGlyph clip
        // semantics too. If the outer clip glyph has no outline, a
        // nested child PaintGlyph must not make the base glyph look
        // paintable.
        let font_data = build_minimal_font_with_colr(
            build_colr_v1_missing_outer_clip_nested_paint_glyph(),
        );
        let font = swash::FontRef::from_index(&font_data, 0).expect("test font parses");

        assert!(
            rasterize_font_glyph(font, 1, 32, [255, 255, 255, 255]).is_none(),
            "missing outer PaintGlyph outline should clip nested child paint"
        );
    }

    #[test]
    fn rasterize_font_glyph_handles_colr_v1_paint_glyph_without_clipbox() {
        // Regression: v1 PaintGlyph graphs do not require a top-level
        // ClipBox. The rasterizer must derive bounds by walking the
        // paint graph, otherwise an empty base outline makes the
        // paintable glyph look unrasterizable.
        let font_data = build_minimal_font_with_colr(build_colr_v1_paint_glyph_no_clip());
        let font = swash::FontRef::from_index(&font_data, 0).expect("test font parses");
        let face = face_from_swash_font(font).expect("COLR face parses");
        assert!(face.glyph_bounding_box(GlyphId(1)).is_none());
        assert!(face.glyph_bounding_box(GlyphId(2)).is_some());

        let raster = rasterize_font_glyph(font, 1, 32, [255, 255, 255, 255])
            .expect("COLR v1 PaintGlyph without ClipBox rasterizes");

        assert!(raster.is_color);
        assert!(raster.data.chunks_exact(4).any(|p| p[3] > 0));
    }

    #[test]
    fn rasterize_mono_top_pixels_filled_bottom_pixels_empty() {
        // Outline occupies only the top 25% of the em — after raster,
        // the top rows of the bitmap should be inked and the bottom
        // rows should be transparent. Catches the Y-flip bug we hit
        // earlier (where the strip rendered at the bottom instead).
        let upm = 100i16;
        let bytes = glyf_top_strip(upm, upm / 4);
        let r = rasterize_mono(&bytes, upm as u16, upm as u16)
            .expect("rasterize succeeds for valid simple glyph");

        let w = r.width as usize;
        let h = r.height as usize;
        assert!(w > 4 && h > 4, "bitmap should be larger than the padding");

        // Sample a row near the top (just below the 1-px pad) and a
        // row near the bottom. Use the centre column to avoid the
        // padding strip on the sides.
        let mid_x = w / 2;
        let top_y = 2;
        let bot_y = h - 2;
        let top_alpha = r.data[top_y * w + mid_x];
        let bot_alpha = r.data[bot_y * w + mid_x];

        assert!(
            top_alpha > 0,
            "top of bitmap should be inked (got alpha {top_alpha})"
        );
        assert!(
            bot_alpha == 0,
            "bottom of bitmap should be empty (got alpha {bot_alpha})"
        );
        assert!(!r.is_color, "glyf path produces an alpha mask");
    }

    #[test]
    fn rasterize_mono_rejects_zero_pixel_size() {
        let bytes = glyf_top_strip(100, 25);
        assert!(rasterize_mono(&bytes, 100, 0).is_none());
        assert!(rasterize_mono(&bytes, 0, 16).is_none());
    }

    #[test]
    fn project_p3_is_perpendicular_to_p0p2_axis_through_p0() {
        // For any well-formed input, (P3 - P0) must be perpendicular
        // to (P2 - P0). This is the defining property of the
        // projection — skrifa, FreeType, and nanoemoji all document
        // it as the `P0-P3 ⟂ P0-P2` constraint.
        let cases = [
            ((0.0, 0.0), (10.0, 5.0), (20.0, 0.0)),
            ((100.0, 100.0), (150.0, 200.0), (200.0, 100.0)),
            ((0.0, 0.0), (3.0, 4.0), (5.0, 0.0)),
            ((-50.0, 25.0), (0.0, 75.0), (50.0, 25.0)),
        ];
        for (p0, p1, p2) in cases {
            let (p3x, p3y) = project_p3(p0, p1, p2).unwrap();
            let p0p3 = (p3x - p0.0, p3y - p0.1);
            let p0p2 = (p2.0 - p0.0, p2.1 - p0.1);
            let d = dot(p0p3, p0p2);
            assert!(
                d.abs() < 1e-3,
                "P0P3 · P0P2 = {d} for p0={p0:?} p1={p1:?} p2={p2:?}",
            );
        }
    }

    #[test]
    fn project_p3_matches_skrifa_formulation() {
        // Cross-check: P3 = P0 + project(P1-P0, perp(P2-P0)) should
        // give the same result as our formulation. Skrifa computes
        // this way; we compute P1 - t*(P2-P0). Both land on the same
        // point mathematically.
        let p0 = (10.0, 20.0);
        let p1 = (50.0, 80.0);
        let p2 = (100.0, 20.0);

        // Skrifa-style: project (P1-P0) onto perpendicular of (P2-P0).
        let perp_x = p2.1 - p0.1; // (dy, -dx) rotation of P0→P2
        let perp_y = -(p2.0 - p0.0);
        let b = (p1.0 - p0.0, p1.1 - p0.1);
        let perp_len_sq = perp_x * perp_x + perp_y * perp_y;
        let k = (b.0 * perp_x + b.1 * perp_y) / perp_len_sq;
        let skrifa_p3 = (p0.0 + k * perp_x, p0.1 + k * perp_y);

        let (our_p3x, our_p3y) = project_p3(p0, p1, p2).unwrap();
        assert!((our_p3x - skrifa_p3.0).abs() < 1e-3);
        assert!((our_p3y - skrifa_p3.1).abs() < 1e-3);
    }

    #[test]
    fn project_p3_rejects_degenerate_axis() {
        // P0 == P2 means the color line has no direction. Must return
        // None so the gradient shader falls back to solid colour.
        assert!(project_p3((10.0, 20.0), (50.0, 50.0), (10.0, 20.0)).is_none());
        // Near-coincident (within epsilon) also rejected.
        assert!(
            project_p3((10.0, 20.0), (50.0, 50.0), (10.0 + 1e-4, 20.0 + 1e-4)).is_none()
        );
    }

    #[test]
    fn project_p3_p1_already_on_perpendicular_returns_p1() {
        // If P1 is already on the perpendicular through P0 (i.e. its
        // projection onto P0→P2 is at P0 itself), P3 should equal P1
        // exactly.
        let p0 = (0.0, 0.0);
        let p2 = (10.0, 0.0);
        let p1 = (0.0, 5.0); // perpendicular to x-axis at origin
        let (p3x, p3y) = project_p3(p0, p1, p2).unwrap();
        assert!((p3x - p1.0).abs() < 1e-6);
        assert!((p3y - p1.1).abs() < 1e-6);
    }

    #[test]
    fn glyf_bbox_reads_signed_bbox() {
        // numContours=1 (0x0001), x_min=-100, y_min=-200, x_max=300, y_max=700.
        let bytes = [
            0x00, 0x01, // numContours
            0xFF, 0x9C, // -100
            0xFF, 0x38, // -200
            0x01, 0x2C, // 300
            0x02, 0xBC, // 700
        ];
        assert_eq!(glyf_bbox(&bytes), Some((-100, -200, 300, 700)));
    }

    #[test]
    fn glyf_bbox_rejects_short_input() {
        assert_eq!(glyf_bbox(&[]), None);
        assert_eq!(glyf_bbox(&[0; 9]), None);
    }

    /// Build a minimal COLR v1 header + BaseGlyphList payload.
    /// `base_glyph_ids` becomes the list of GlyphIDs written as
    /// BaseGlyphPaintRecord entries, in order.
    fn build_colr_v1(base_glyph_ids: &[u16]) -> Vec<u8> {
        let mut out = Vec::new();
        // Header: version=1, num_v0=0, v0_off=0, layer_off=0, num_layers=0.
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());
        // base_glyph_list_offset — points right after the v1 header
        // (34 bytes total: 14 v0 + 4 (v1_base) + 4 (v1_layer) +
        // 4 (v1_clip) + 4 (varindex) + 4 (variationstore)).
        let list_off: u32 = 34;
        out.extend_from_slice(&list_off.to_be_bytes());
        // layer_list_offset, clip_list_offset, var_index_map_offset,
        // item_variation_store_offset — all 0, unused.
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes());
        assert_eq!(out.len(), list_off as usize);
        // BaseGlyphList: num_records: u32, then (u16 gid, u32 paint_off).
        out.extend_from_slice(&(base_glyph_ids.len() as u32).to_be_bytes());
        for &gid in base_glyph_ids {
            out.extend_from_slice(&gid.to_be_bytes());
            out.extend_from_slice(&0u32.to_be_bytes());
        }
        out
    }

    #[test]
    fn first_base_glyph_id_picks_first_non_empty() {
        // GID 0 is empty (.notdef), GID 1 has outline bytes — the
        // subsetted-Nabla case. Must return 1, not 0.
        let colr = build_colr_v1(&[0, 1]);
        let glyphs: Vec<Vec<u8>> = vec![
            vec![],              // GID 0: empty
            vec![0xA, 0xB, 0xC], // GID 1: has bytes
        ];
        assert_eq!(first_base_glyph_id(&colr, &glyphs), Some(1));
    }

    #[test]
    fn first_base_glyph_id_honours_record_order() {
        // All GIDs non-empty → returns the first one.
        let colr = build_colr_v1(&[3, 1, 7]);
        let glyphs: Vec<Vec<u8>> = vec![vec![1]; 10]; // GID 0..9 all non-empty
        assert_eq!(first_base_glyph_id(&colr, &glyphs), Some(3));
    }

    #[test]
    fn first_base_glyph_id_falls_back_to_first_record_when_all_empty() {
        // Every record points at an empty outline (pathological case
        // where the subsetter kept placeholders only). Return the
        // first record's GID so the caller's bbox read bails cleanly
        // rather than panicking on an `expect_some`.
        let colr = build_colr_v1(&[5, 10]);
        let glyphs: Vec<Vec<u8>> = vec![vec![]; 20];
        assert_eq!(first_base_glyph_id(&colr, &glyphs), Some(5));
    }

    #[test]
    fn first_base_glyph_id_handles_empty_colr_table() {
        // < 8 bytes: nothing to parse.
        assert_eq!(first_base_glyph_id(&[], &[]), None);
        assert_eq!(first_base_glyph_id(&[0, 0, 0, 0, 0, 0, 0, 0], &[]), None);
    }

    #[test]
    fn base_glyph_ids_falls_back_to_v0_when_v1_list_offset_is_invalid() {
        // Defends: COLR v1 tables can still carry v0 records. Bad v1
        // BaseGlyphList metadata should not hide valid v0 base records.
        let mut colr = build_colr_layer_only(1);
        colr[14..18].copy_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(base_glyph_ids(&colr), Some(vec![1]));
    }

    #[test]
    fn composite_mode_to_blend_covers_every_variant() {
        // Every CompositeMode variant from ttf-parser's COLR spec
        // (§Format 32 Paint​Composite) must map to some tiny-skia
        // BlendMode. Exhaustive enum match catches a missing arm at
        // compile time, but this test also guarantees the common
        // `SourceOver` → `SourceOver` pairing — the one the layer
        // stack falls back on when nothing special is in play.
        use CompositeMode::*;
        assert_eq!(composite_mode_to_blend(SourceOver), BlendMode::SourceOver);
        assert_eq!(composite_mode_to_blend(Clear), BlendMode::Clear);
        assert_eq!(composite_mode_to_blend(Xor), BlendMode::Xor);
        assert_eq!(composite_mode_to_blend(Plus), BlendMode::Plus);
        assert_eq!(composite_mode_to_blend(Multiply), BlendMode::Multiply);
        assert_eq!(composite_mode_to_blend(Luminosity), BlendMode::Luminosity);
    }

    #[test]
    fn extend_to_spread_maps_all_three_modes() {
        assert_eq!(extend_to_spread(GradientExtend::Pad), SpreadMode::Pad);
        assert_eq!(extend_to_spread(GradientExtend::Repeat), SpreadMode::Repeat);
        assert_eq!(
            extend_to_spread(GradientExtend::Reflect),
            SpreadMode::Reflect
        );
    }

    #[test]
    fn intersect_masks_multiplies_alpha_channels() {
        let mut dst = Mask::new(2, 2).unwrap();
        let mut src = Mask::new(2, 2).unwrap();
        // Manually set the alpha bytes: dst = 255,128,64,0; src = 128,255,128,255.
        dst.data_mut().copy_from_slice(&[255, 128, 64, 0]);
        src.data_mut().copy_from_slice(&[128, 255, 128, 255]);

        intersect_masks(&mut dst, &src);

        // (255 * 128) / 255 = 128
        // (128 * 255) / 255 = 128
        // (64  * 128) / 255 = 32  (integer division)
        // (0   * 255) / 255 = 0
        assert_eq!(dst.data(), &[128, 128, 32, 0]);
    }

    #[test]
    fn intersect_masks_ignores_mismatched_sizes() {
        // Precondition: we only call intersect_masks on masks with
        // the same dimensions. If that ever fails, we leave dst as-is
        // rather than panicking.
        let mut dst = Mask::new(2, 2).unwrap();
        dst.data_mut().copy_from_slice(&[0x55; 4]);
        let src = Mask::new(4, 2).unwrap();
        intersect_masks(&mut dst, &src);
        assert_eq!(dst.data(), &[0x55; 4]);
    }
}
