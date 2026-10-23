//! Rectangular regions and the mask that excludes them from a comparison.

/// A rectangle in pixel coordinates, with the origin at the top left.
///
/// The region covers `[x, x + width)` horizontally and `[y, y + height)`
/// vertically, so a rectangle of zero width or height covers nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// The part of the rectangle that lies within a `width` x `height` image,
    /// or `None` when nothing of it does.
    fn clip(&self, width: u32, height: u32) -> Option<Self> {
        let right = self.x.saturating_add(self.width).min(width);
        let bottom = self.y.saturating_add(self.height).min(height);
        if self.x >= right || self.y >= bottom {
            return None;
        }
        Some(Self {
            x: self.x,
            y: self.y,
            width: right - self.x,
            height: bottom - self.y,
        })
    }

    fn contains(&self, x: u32, y: u32) -> bool {
        x >= self.x && x - self.x < self.width && y >= self.y && y - self.y < self.height
    }
}

/// The regions a comparison leaves out, clipped to the image.
pub struct Mask {
    regions: Vec<Rect>,
}

impl Mask {
    /// Clips `regions` to a `width` x `height` image, dropping those that fall
    /// outside it entirely.
    pub fn new(regions: &[Rect], width: u32, height: u32) -> Self {
        Self {
            regions: regions
                .iter()
                .filter_map(|region| region.clip(width, height))
                .collect(),
        }
    }

    /// Whether the pixel at (`x`, `y`) is left out of the comparison.
    pub fn excludes(&self, x: u32, y: u32) -> bool {
        self.regions.iter().any(|region| region.contains(x, y))
    }

    /// Whether any region covers part of row `y`.
    ///
    /// A row it does not reach has every pixel compared, which lets the scan
    /// count the row in one step instead of asking about each pixel.
    pub fn reaches_row(&self, y: u32) -> bool {
        self.regions
            .iter()
            .any(|region| y >= region.y && y - region.y < region.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_region_covers_its_top_left_corner_but_not_its_bottom_right() {
        let mask = Mask::new(
            &[Rect {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            }],
            10,
            10,
        );
        assert!(mask.excludes(1, 2));
        assert!(mask.excludes(3, 5));
        assert!(!mask.excludes(4, 5));
        assert!(!mask.excludes(3, 6));
        assert!(!mask.excludes(0, 2));
    }

    #[test]
    fn an_empty_region_covers_nothing() {
        let mask = Mask::new(
            &[Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 5,
            }],
            10,
            10,
        );
        assert!(!mask.excludes(0, 0));
    }

    #[test]
    fn a_region_reaching_past_the_image_keeps_the_part_inside() {
        let mask = Mask::new(
            &[Rect {
                x: 8,
                y: 8,
                width: u32::MAX,
                height: u32::MAX,
            }],
            10,
            10,
        );
        assert!(mask.excludes(9, 9));
        assert!(!mask.excludes(7, 9));
    }

    #[test]
    fn a_row_is_reached_only_between_the_top_and_bottom_of_a_region() {
        let mask = Mask::new(
            &[Rect {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            }],
            10,
            10,
        );
        assert!(!mask.reaches_row(1));
        assert!(mask.reaches_row(2));
        assert!(mask.reaches_row(5));
        assert!(!mask.reaches_row(6));
    }

    #[test]
    fn a_region_outside_the_image_is_dropped() {
        let mask = Mask::new(
            &[Rect {
                x: 20,
                y: 0,
                width: 5,
                height: 5,
            }],
            10,
            10,
        );
        assert!(mask.regions.is_empty());
    }
}
