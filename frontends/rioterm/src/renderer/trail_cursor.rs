// This file was heavily inspired by neovide implementation.

use rio_backend::sugarloaf::Sugarloaf;
use std::time::Instant;

/// Animation duration for long jumps (seconds).
const ANIMATION_LENGTH: f32 = 0.15;

/// Animation duration for short (≤2 cell horizontal) movements.
const SHORT_ANIMATION_LENGTH: f32 = 0.04;

/// Cursor jumps above this cell distance are treated as redraw warps.
/// Full-screen TUIs often move the terminal cursor between distant paint
/// regions while scrolling; animating those jumps draws huge trail quads.
const WARP_MOVE_MAX_CELLS: f32 = 32.0;

/// One-row vertical editor/file-manager movement may include a large horizontal
/// column correction on short or empty lines. Keep that visual warp.
const ONE_ROW_WARP_MAX_VERTICAL_CELLS: f32 = 1.001;

/// Large same-column vertical relocations are usually transient terminal UI
/// redraw positions, such as shells repainting an input line. Do not trail
/// those from/to the left edge.
const VERTICAL_RELOCATION_MAX_CELLS: f32 = 6.0;
const VERTICAL_RELOCATION_MAX_HORIZONTAL_CELLS: f32 = 1.001;

/// Nearby terminal-cursor movement should stay visually responsive. This
/// includes editor/file-manager vertical `j`/`k` movement, not only typing.
const SHORT_MOVE_MAX_CELLS: f32 = 2.001;

/// Trail size 0.0–1.0.
/// 1.0 = max stretch (leading edge jumps instantly,
/// trailing edge lags most).
const TRAIL_SIZE: f32 = 1.0;
const DEPTH: f32 = 0.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrailCursorSnapshot {
    /// Animated cursor top-left in physical pixels.
    pub x: f32,
    /// Animated cursor top-left in physical pixels.
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrailCursorShaderState {
    /// Terminal cursor destination in physical pixels, top-left origin.
    pub destination_rect: TrailCursorSnapshot,
    /// Bounding box of the animated Rio trail corners in physical pixels.
    pub animated_rect: TrailCursorSnapshot,
    /// Animated trail corners in physical pixels, ordered top-left,
    /// top-right, bottom-right, bottom-left.
    pub corners: [[f32; 2]; 4],
    pub animating: bool,
}

#[derive(Clone)]
struct Spring {
    position: f32,
    velocity: f32,
}

impl Spring {
    #[inline]
    fn new() -> Self {
        Self {
            position: 0.0,
            velocity: 0.0,
        }
    }

    #[inline]
    fn reset(&mut self) {
        self.position = 0.0;
        self.velocity = 0.0;
    }

    /// Advance by variable `dt`. Returns `true` while still moving.
    #[inline]
    fn update(&mut self, dt: f32, animation_length: f32) -> bool {
        if animation_length <= dt {
            self.reset();
            return false;
        }
        if self.position == 0.0 {
            return false;
        }

        // Critically-damped spring (zeta = 1.0).
        // omega chosen so destination is reached within ~2% tolerance in
        // `animation_length` time.
        let omega = 4.0 / animation_length;

        // Analytical solution for critically-damped harmonic oscillation.
        let a = self.position;
        let b = a * omega + self.velocity;
        let c = (-omega * dt).exp();

        self.position = (a + b * dt) * c;
        self.velocity = c * (-a * omega - b * dt * omega + b);

        if self.position.abs() < 0.01 {
            self.reset();
            false
        } else {
            true
        }
    }
}

#[derive(Clone)]
struct Corner {
    spring_x: Spring,
    spring_y: Spring,
    /// Current animated pixel position.
    x: f32,
    y: f32,
    /// Offset relative to cursor center (shape-aware).
    rel_x: f32,
    rel_y: f32,
    prev_dest_x: f32,
    prev_dest_y: f32,
    anim_length: f32,
}

impl Corner {
    fn new(rel_x: f32, rel_y: f32) -> Self {
        Self {
            spring_x: Spring::new(),
            spring_y: Spring::new(),
            x: 0.0,
            y: 0.0,
            rel_x,
            rel_y,
            prev_dest_x: -1e6,
            prev_dest_y: -1e6,
            anim_length: 0.0,
        }
    }

    #[inline]
    fn destination(
        &self,
        center_x: f32,
        center_y: f32,
        cell_w: f32,
        cell_h: f32,
    ) -> (f32, f32) {
        (
            center_x + self.rel_x * cell_w,
            center_y + self.rel_y * cell_h,
        )
    }

    #[inline]
    fn update(
        &mut self,
        center_x: f32,
        center_y: f32,
        cell_w: f32,
        cell_h: f32,
        dt: f32,
        immediate_movement: bool,
    ) -> bool {
        let (dest_x, dest_y) = self.destination(center_x, center_y, cell_w, cell_h);

        if (dest_x - self.prev_dest_x).abs() > 0.01
            || (dest_y - self.prev_dest_y).abs() > 0.01
        {
            self.spring_x.position = dest_x - self.x;
            self.spring_y.position = dest_y - self.y;
            self.prev_dest_x = dest_x;
            self.prev_dest_y = dest_y;
        }

        // Teleport: snap to destination without animating.
        if immediate_movement {
            self.x = dest_x;
            self.y = dest_y;
            self.spring_x.reset();
            self.spring_y.reset();
            return false;
        }

        let mut animating = self.spring_x.update(dt, self.anim_length);
        animating |= self.spring_y.update(dt, self.anim_length);
        self.x = dest_x - self.spring_x.position;
        self.y = dest_y - self.spring_y.position;

        animating
    }

    /// Direction alignment: dot product of the corner's relative direction
    /// with the travel direction.  Higher = more aligned with movement =
    /// "leading".  Matches neovide's `calculate_direction_alignment`.
    #[inline]
    fn direction_alignment(
        &self,
        center_x: f32,
        center_y: f32,
        cell_w: f32,
        cell_h: f32,
    ) -> f32 {
        let (dest_x, dest_y) = self.destination(center_x, center_y, cell_w, cell_h);

        // Corner's relative direction (normalized).
        let rel_len = (self.rel_x * self.rel_x + self.rel_y * self.rel_y)
            .sqrt()
            .max(1e-6);
        let corner_dir_x = self.rel_x / rel_len;
        let corner_dir_y = self.rel_y / rel_len;

        // Travel direction (from current animated pos to destination).
        let dx = dest_x - self.x;
        let dy = dest_y - self.y;
        let travel_len = (dx * dx + dy * dy).sqrt().max(1e-6);

        (dx / travel_len) * corner_dir_x + (dy / travel_len) * corner_dir_y
    }
}

pub struct TrailCursor {
    /// Four corners: [top-left, top-right, bottom-right, bottom-left].
    corners: [Corner; 4],
    last_frame: Instant,
    /// Current destination center (physical pixels).
    dest_cx: f32,
    dest_cy: f32,
    /// Previous destination center, used to detect jumps.
    prev_dest_cx: f32,
    prev_dest_cy: f32,
    /// Center before the current jump — preserved so `compute_jump` can
    /// measure travel distance (since `set_destination` overwrites
    /// `prev_dest` before `animate` runs).
    jump_from_cx: f32,
    jump_from_cy: f32,
    /// One-shot flag: set when destination changes, consumed in `animate`.
    jumped: bool,
    /// One-shot flag: snap the next jump instead of drawing a trail.
    snap_next_jump: bool,
    /// True until the first real destination is set — first frame teleports.
    first_frame: bool,
    route_id: Option<usize>,
    animating: bool,
}

impl TrailCursor {
    pub fn new() -> Self {
        Self {
            corners: [
                Corner::new(-0.5, -0.5), // top-left
                Corner::new(0.5, -0.5),  // top-right
                Corner::new(0.5, 0.5),   // bottom-right
                Corner::new(-0.5, 0.5),  // bottom-left
            ],
            last_frame: Instant::now(),
            dest_cx: 0.0,
            dest_cy: 0.0,
            prev_dest_cx: -1e6,
            prev_dest_cy: -1e6,
            jump_from_cx: -1e6,
            jump_from_cy: -1e6,
            jumped: false,
            snap_next_jump: false,
            first_frame: true,
            route_id: None,
            animating: false,
        }
    }

    pub fn set_route(&mut self, route_id: usize) {
        if self.route_id != Some(route_id) {
            self.route_id = Some(route_id);
            self.first_frame = true;
        }
    }

    pub fn reset(&mut self) {
        for corner in &mut self.corners {
            corner.spring_x.reset();
            corner.spring_y.reset();
            corner.prev_dest_x = -1e6;
            corner.prev_dest_y = -1e6;
            corner.x = 0.0;
            corner.y = 0.0;
        }
        self.prev_dest_cx = -1e6;
        self.prev_dest_cy = -1e6;
        self.jump_from_cx = -1e6;
        self.jump_from_cy = -1e6;
        self.jumped = false;
        self.snap_next_jump = false;
        self.first_frame = true;
        self.animating = false;
    }

    /// Update the cursor destination.  Called once per frame **before**
    /// `animate()`.  Sets the `jumped` flag when the destination changes
    /// (matching neovide's `update_cursor_destination`).
    pub fn set_destination(
        &mut self,
        cursor_x: f32,
        cursor_y: f32,
        cell_width: f32,
        cell_height: f32,
    ) {
        // Center of cursor cell.
        let cx = cursor_x + cell_width * 0.5;
        let cy = cursor_y + cell_height * 0.5;
        self.dest_cx = cx;
        self.dest_cy = cy;

        // Detect a jump (destination changed).
        if (cx - self.prev_dest_cx).abs() > 0.01 || (cy - self.prev_dest_cy).abs() > 0.01
        {
            self.jump_from_cx = self.prev_dest_cx;
            self.jump_from_cy = self.prev_dest_cy;
            self.snap_next_jump = cursor_jump_should_snap(
                self.jump_from_cx,
                self.jump_from_cy,
                cx,
                cy,
                cell_width,
                cell_height,
            );
            self.prev_dest_cx = cx;
            self.prev_dest_cy = cy;
            self.jumped = true;
        }
    }

    /// Run animation for one frame.  Called once per frame **after**
    /// `set_destination()`.  If `jumped` is set, computes corner ranking
    /// and assigns animation lengths exactly once per jump (matching
    /// neovide's `animate`).
    pub fn animate(&mut self, cell_width: f32, cell_height: f32) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        let cx = self.dest_cx;
        let cy = self.dest_cy;

        // First frame: teleport all corners to destination without
        // animation (matches neovide's `immediate_movement`).
        let immediate = self.first_frame || self.snap_next_jump;
        if self.first_frame {
            self.first_frame = false;
        }
        self.snap_next_jump = false;

        // On jump: compute ranking and set animation lengths (one-shot).
        if self.jumped && !immediate {
            self.compute_jump(cx, cy, cell_width, cell_height);
        }
        self.jumped = false;

        // Spring update every frame (matching neovide).
        let mut still_animating = false;
        for corner in &mut self.corners {
            if corner.update(cx, cy, cell_width, cell_height, dt, immediate) {
                still_animating = true;
            }
        }

        self.animating = still_animating;
    }

    /// Compute corner direction-alignment ranking and assign animation
    /// lengths.  Called exactly once per cursor jump (matching neovide's
    /// `Corner::jump` called from the `if self.jumped` block).
    fn compute_jump(&mut self, cx: f32, cy: f32, cell_width: f32, cell_height: f32) {
        // Compute jump vector in cell units for short-movement detection.
        // `jump_from` is the center *before* this jump was detected.
        let jump_x = if cell_width > 0.0 {
            ((cx - self.jump_from_cx) / cell_width).abs()
        } else {
            0.0
        };
        let jump_y = if cell_height > 0.0 {
            ((cy - self.jump_from_cy) / cell_height).abs()
        } else {
            0.0
        };
        let is_short = cursor_jump_distance_cells(jump_x, jump_y) <= SHORT_MOVE_MAX_CELLS;

        if is_short {
            let t = ANIMATION_LENGTH.min(SHORT_ANIMATION_LENGTH);
            for c in &mut self.corners {
                c.anim_length = t;
            }
            return;
        }

        // Direction-alignment ranking (neovide-style).
        let mut alignments: [(usize, f32); 4] = [
            (
                0,
                self.corners[0].direction_alignment(cx, cy, cell_width, cell_height),
            ),
            (
                1,
                self.corners[1].direction_alignment(cx, cy, cell_width, cell_height),
            ),
            (
                2,
                self.corners[2].direction_alignment(cx, cy, cell_width, cell_height),
            ),
            (
                3,
                self.corners[3].direction_alignment(cx, cy, cell_width, cell_height),
            ),
        ];

        // Sort ascending: lowest alignment = most trailing.
        alignments.sort_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.0.cmp(&b.0))
        });

        // Build per-corner rank array.
        let mut ranks = [0usize; 4];
        for (rank, &(corner_idx, _)) in alignments.iter().enumerate() {
            ranks[corner_idx] = rank;
        }

        let leading = ANIMATION_LENGTH * (1.0 - TRAIL_SIZE).clamp(0.0, 1.0);
        let trailing = ANIMATION_LENGTH;
        let mid = (leading + trailing) / 2.0;

        for (i, corner) in self.corners.iter_mut().enumerate() {
            corner.anim_length = match ranks[i] {
                0 => trailing,
                1 => mid,
                _ => leading,
            };
        }
    }

    /// Draw the cursor trail as a single convex quad spanned by the four
    /// animated corners — emitted as two triangles through the existing
    /// `DrawCmd::Vertices` pipeline. Matches neovide's approach of
    /// `PathBuilder::move_to(TL).line_to(TR).line_to(BR).line_to(BL).close()`
    /// into a single `draw_path`. The old scanline fill (up to 640 rects
    /// per frame) was a workaround for `sugarloaf.rect` being axis-aligned
    /// only; `sugarloaf.triangle` already accepts arbitrary vertex
    /// positions, so one fan covers the same pixels in one draw call.
    pub fn draw(
        &self,
        sugarloaf: &mut Sugarloaf,
        scale_factor: f32,
        cursor_color: [f32; 4],
    ) {
        if !self.animating {
            return;
        }

        let inv = 1.0 / scale_factor;

        // Corner positions in *logical* pixels (sugarloaf.triangle scales
        // by scale_factor internally). Ordered TL, TR, BR, BL — same
        // winding as neovide's path builder.
        let pts: [(f32, f32); 4] = [
            (self.corners[0].x * inv, self.corners[0].y * inv),
            (self.corners[1].x * inv, self.corners[1].y * inv),
            (self.corners[2].x * inv, self.corners[2].y * inv),
            (self.corners[3].x * inv, self.corners[3].y * inv),
        ];

        // Fan from TL: (TL, TR, BR) + (TL, BR, BL). Two triangles share
        // TL and BR, so the shared diagonal seam is hidden inside the
        // convex hull — same as any triangle-fan tessellation.
        sugarloaf.triangle(
            pts[0].0,
            pts[0].1,
            pts[1].0,
            pts[1].1,
            pts[2].0,
            pts[2].1,
            DEPTH,
            cursor_color,
        );
        sugarloaf.triangle(
            pts[0].0,
            pts[0].1,
            pts[2].0,
            pts[2].1,
            pts[3].0,
            pts[3].1,
            DEPTH,
            cursor_color,
        );
    }

    /// `true` while the spring corners haven't settled *visibly*.
    #[inline]
    pub fn is_animating(&self) -> bool {
        self.animating
    }

    #[inline]
    pub fn shader_state(
        &self,
        cell_width: f32,
        cell_height: f32,
    ) -> Option<TrailCursorShaderState> {
        if self.first_frame || cell_width <= 0.0 || cell_height <= 0.0 {
            return None;
        }

        let animated_rect = self.animated_rect_from_corners()?;
        let destination_rect = TrailCursorSnapshot {
            x: self.dest_cx - cell_width * 0.5,
            y: self.dest_cy - cell_height * 0.5,
            width: cell_width,
            height: cell_height,
        };
        let corners =
            std::array::from_fn(|idx| [self.corners[idx].x, self.corners[idx].y]);

        Some(TrailCursorShaderState {
            destination_rect,
            animated_rect,
            corners,
            animating: self.animating,
        })
    }

    fn animated_rect_from_corners(&self) -> Option<TrailCursorSnapshot> {
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for corner in &self.corners {
            min_x = min_x.min(corner.x);
            min_y = min_y.min(corner.y);
            max_x = max_x.max(corner.x);
            max_y = max_y.max(corner.y);
        }

        let width = max_x - min_x;
        let height = max_y - min_y;
        if !min_x.is_finite()
            || !min_y.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
        {
            return None;
        }

        Some(TrailCursorSnapshot {
            x: min_x,
            y: min_y,
            width,
            height,
        })
    }
}

#[inline]
fn cursor_jump_distance_cells(jump_x: f32, jump_y: f32) -> f32 {
    jump_x.hypot(jump_y)
}

#[inline]
fn cursor_jump_should_snap(
    from_cx: f32,
    from_cy: f32,
    to_cx: f32,
    to_cy: f32,
    cell_width: f32,
    cell_height: f32,
) -> bool {
    if from_cx < -999_999.0 || from_cy < -999_999.0 {
        return true;
    }

    let jump_x = if cell_width > 0.0 {
        ((to_cx - from_cx) / cell_width).abs()
    } else {
        0.0
    };
    let jump_y = if cell_height > 0.0 {
        ((to_cy - from_cy) / cell_height).abs()
    } else {
        0.0
    };

    if jump_y <= ONE_ROW_WARP_MAX_VERTICAL_CELLS {
        return jump_x > WARP_MOVE_MAX_CELLS;
    }

    if jump_x <= VERTICAL_RELOCATION_MAX_HORIZONTAL_CELLS
        && jump_y > VERTICAL_RELOCATION_MAX_CELLS
    {
        return true;
    }

    cursor_jump_distance_cells(jump_x, jump_y) > WARP_MOVE_MAX_CELLS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_cursor(cursor: &mut TrailCursor, cell_width: f32, cell_height: f32) {
        cursor.set_destination(0.0, 0.0, cell_width, cell_height);
        cursor.animate(cell_width, cell_height);
    }

    #[test]
    fn shader_state_exposes_destination_rect_and_ordered_corners() {
        let cell_width = 10.0;
        let cell_height = 20.0;
        let mut cursor = TrailCursor::new();

        cursor.set_destination(30.0, 40.0, cell_width, cell_height);
        cursor.animate(cell_width, cell_height);

        assert_eq!(
            cursor.shader_state(cell_width, cell_height),
            Some(TrailCursorShaderState {
                destination_rect: TrailCursorSnapshot {
                    x: 30.0,
                    y: 40.0,
                    width: cell_width,
                    height: cell_height,
                },
                animated_rect: TrailCursorSnapshot {
                    x: 30.0,
                    y: 40.0,
                    width: cell_width,
                    height: cell_height,
                },
                corners: [[30.0, 40.0], [40.0, 40.0], [40.0, 60.0], [30.0, 60.0],],
                animating: false,
            })
        );
    }

    #[test]
    fn one_row_vertical_move_uses_short_animation() {
        let cell_width = 10.0;
        let cell_height = 20.0;
        let mut cursor = TrailCursor::new();
        seed_cursor(&mut cursor, cell_width, cell_height);

        cursor.set_destination(0.0, cell_height, cell_width, cell_height);
        cursor.animate(cell_width, cell_height);

        for corner in &cursor.corners {
            assert_eq!(corner.anim_length, SHORT_ANIMATION_LENGTH);
        }
        assert!(cursor.is_animating());
    }

    #[test]
    fn large_cursor_jump_snaps_without_trail_animation() {
        let cell_width = 10.0;
        let cell_height = 20.0;
        let mut cursor = TrailCursor::new();
        seed_cursor(&mut cursor, cell_width, cell_height);

        cursor.set_destination(0.0, cell_height * 40.0, cell_width, cell_height);
        cursor.animate(cell_width, cell_height);

        assert!(!cursor.is_animating());
        assert_eq!(cursor.corners[0].x, 0.0);
        assert_eq!(cursor.corners[0].y, cell_height * 40.0);
        assert_eq!(cursor.corners[2].x, cell_width);
        assert_eq!(cursor.corners[2].y, cell_height * 41.0);
    }

    #[test]
    fn moderate_one_row_column_move_keeps_trail_animation() {
        let cell_width = 10.0;
        let cell_height = 20.0;
        let mut cursor = TrailCursor::new();
        seed_cursor(&mut cursor, cell_width, cell_height);

        cursor.set_destination(cell_width * 8.0, cell_height, cell_width, cell_height);
        cursor.animate(cell_width, cell_height);

        assert!(cursor.is_animating());
        assert!(cursor
            .corners
            .iter()
            .any(|corner| corner.anim_length == ANIMATION_LENGTH));
    }

    #[test]
    fn extreme_one_row_column_warp_snaps_without_trail_animation() {
        let cell_width = 10.0;
        let cell_height = 20.0;
        let mut cursor = TrailCursor::new();
        seed_cursor(&mut cursor, cell_width, cell_height);

        cursor.set_destination(cell_width * 40.0, cell_height, cell_width, cell_height);
        cursor.animate(cell_width, cell_height);

        assert!(!cursor.is_animating());
        assert_eq!(cursor.corners[0].x, cell_width * 40.0);
        assert_eq!(cursor.corners[0].y, cell_height);
        assert_eq!(cursor.corners[2].x, cell_width * 41.0);
        assert_eq!(cursor.corners[2].y, cell_height * 2.0);
    }

    #[test]
    fn reset_clears_animation_and_makes_next_destination_snap() {
        let cell_width = 10.0;
        let cell_height = 20.0;
        let mut cursor = TrailCursor::new();
        seed_cursor(&mut cursor, cell_width, cell_height);

        cursor.set_destination(cell_width * 2.0, 0.0, cell_width, cell_height);
        cursor.animate(cell_width, cell_height);
        assert!(cursor.is_animating());

        cursor.reset();
        assert!(!cursor.is_animating());

        cursor.set_destination(cell_width * 4.0, cell_height, cell_width, cell_height);
        cursor.animate(cell_width, cell_height);
        assert!(!cursor.is_animating());
        assert_eq!(cursor.corners[0].x, cell_width * 4.0);
        assert_eq!(cursor.corners[0].y, cell_height);
        assert_eq!(cursor.corners[2].x, cell_width * 5.0);
        assert_eq!(cursor.corners[2].y, cell_height * 2.0);
    }

    #[test]
    fn route_change_snaps_without_trailing_between_panels() {
        let cell_width = 10.0;
        let cell_height = 20.0;
        let mut cursor = TrailCursor::new();
        cursor.set_route(1);
        seed_cursor(&mut cursor, cell_width, cell_height);

        cursor.set_route(2);
        cursor.set_destination(cell_width * 5.0, 0.0, cell_width, cell_height);
        cursor.animate(cell_width, cell_height);

        assert!(!cursor.is_animating());
        assert_eq!(cursor.corners[0].x, cell_width * 5.0);
        assert_eq!(cursor.corners[0].y, 0.0);
        assert_eq!(cursor.corners[2].x, cell_width * 6.0);
        assert_eq!(cursor.corners[2].y, cell_height);
    }

    #[test]
    fn long_same_column_vertical_relocation_snaps_without_trail_animation() {
        let cell_width = 10.0;
        let cell_height = 20.0;
        let mut cursor = TrailCursor::new();
        seed_cursor(&mut cursor, cell_width, cell_height);

        cursor.set_destination(0.0, cell_height * 10.0, cell_width, cell_height);
        cursor.animate(cell_width, cell_height);

        assert!(!cursor.is_animating());
        assert_eq!(cursor.corners[0].x, 0.0);
        assert_eq!(cursor.corners[0].y, cell_height * 10.0);
        assert_eq!(cursor.corners[2].x, cell_width);
        assert_eq!(cursor.corners[2].y, cell_height * 11.0);
    }

    #[test]
    fn diagonal_redraw_jump_exceeds_snap_threshold() {
        assert!(cursor_jump_should_snap(5.0, 10.0, 405.0, 810.0, 10.0, 20.0));
        assert!(!cursor_jump_should_snap(5.0, 10.0, 5.0, 30.0, 10.0, 20.0));
        assert!(cursor_jump_should_snap(5.0, 10.0, 405.0, 30.0, 10.0, 20.0));
        assert!(cursor_jump_should_snap(5.0, 10.0, 5.0, 210.0, 10.0, 20.0));
    }
}
