use rio_backend::sugarloaf::GraphicId;

const ATLAS_IMAGE_NAMESPACE: u32 = 0x8000_0000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtlasCellSlice {
    pub width: f32,
    pub height: f32,
    pub source_rect: [f32; 4],
}

pub fn atlas_image_id(id: GraphicId) -> Option<u32> {
    let raw = u32::try_from(id.get()).ok()?;
    Some(raw | ATLAS_IMAGE_NAMESPACE)
}

pub fn atlas_cell_slice(
    offset_x: u16,
    offset_y: u16,
    image_width: u16,
    image_height: u16,
    cell_width: f32,
    cell_height: f32,
) -> Option<AtlasCellSlice> {
    if image_width == 0 || image_height == 0 || cell_width <= 0.0 || cell_height <= 0.0 {
        return None;
    }

    let image_width = image_width as f32;
    let image_height = image_height as f32;
    let offset_x = offset_x as f32;
    let offset_y = offset_y as f32;

    if offset_x >= image_width || offset_y >= image_height {
        return None;
    }

    let width = cell_width.min(image_width - offset_x);
    let height = cell_height.min(image_height - offset_y);

    if width <= 0.0 || height <= 0.0 {
        return None;
    }

    Some(AtlasCellSlice {
        width,
        height,
        source_rect: [
            offset_x / image_width,
            offset_y / image_height,
            width / image_width,
            height / image_height,
        ],
    })
}

#[cfg(test)]
// Test lane: default
mod tests {
    use super::*;

    #[test]
    // Defends: Atlas graphics share the renderer image store without colliding with common Kitty image ids.
    fn atlas_image_ids_use_reserved_namespace() {
        assert_eq!(atlas_image_id(GraphicId::new(7)), Some(0x8000_0007));
    }

    #[test]
    // Defends: Sixel/iTerm atlas cells render only the visible texture slice for each terminal cell.
    fn atlas_cell_slice_uses_cell_sized_source_rect() {
        let slice = atlas_cell_slice(10, 20, 120, 80, 9.0, 18.0).unwrap();

        assert_eq!(slice.width, 9.0);
        assert_eq!(slice.height, 18.0);
        assert_eq!(
            slice.source_rect,
            [10.0 / 120.0, 20.0 / 80.0, 9.0 / 120.0, 18.0 / 80.0,]
        );
    }

    #[test]
    // Defends: The last cell of an atlas graphic is clipped instead of sampling outside the texture.
    fn atlas_cell_slice_clips_final_cell() {
        let slice = atlas_cell_slice(116, 73, 120, 80, 9.0, 18.0).unwrap();

        assert_eq!(slice.width, 4.0);
        assert_eq!(slice.height, 7.0);
        assert_eq!(
            slice.source_rect,
            [116.0 / 120.0, 73.0 / 80.0, 4.0 / 120.0, 7.0 / 80.0]
        );
    }
}
