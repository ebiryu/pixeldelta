//! Grouping differing pixels into connected clusters.
//!
//! A caller that only sees a diff pixel count cannot tell one moved button
//! from a change spread across the screen. Clustering answers that: it walks
//! the diff bitmap the scan leaves behind and returns one rectangle per
//! connected group of differing pixels.

use crate::region::Rect;

/// A connected group of differing pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cluster {
    /// The smallest rectangle covering the group, in the coordinate system of
    /// [`Rect`].
    pub bounds: Rect,
    /// Number of differing pixels in the group.
    pub diff_pixels: u64,
    /// Displacement to where the group's content sits in the other image, when
    /// the layout-shift search ran.
    pub displacement: Option<(i32, i32)>,
    /// Structural similarity to the displaced content, when it was measured.
    pub ssim: Option<f64>,
}

/// Groups the set pixels of `bitmap` into eight-connected clusters.
///
/// `bitmap` holds one entry per pixel, row major, with a true entry for a
/// differing pixel. The walk clears each entry it visits, so the buffer is
/// left empty. Clusters come out in the raster order of the first pixel found
/// in each.
pub fn clusters(bitmap: &mut [bool], width: u32, height: u32) -> Vec<Cluster> {
    let stride = width as usize;
    let mut out = Vec::new();
    let mut stack = Vec::new();

    for seed in 0..bitmap.len() {
        if !bitmap[seed] {
            continue;
        }

        // Flood fill the group the seed belongs to, clearing each pixel as it
        // joins so it is counted once and the buffer ends up empty.
        bitmap[seed] = false;
        stack.push(seed);
        let (mut min_x, mut min_y) = (u32::MAX, u32::MAX);
        let (mut max_x, mut max_y) = (0u32, 0u32);
        let mut diff_pixels = 0u64;

        while let Some(index) = stack.pop() {
            let x = (index % stride) as u32;
            let y = (index / stride) as u32;
            diff_pixels += 1;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);

            let left = x.saturating_sub(1);
            let right = (x + 1).min(width - 1);
            let top = y.saturating_sub(1);
            let bottom = (y + 1).min(height - 1);
            for ny in top..=bottom {
                let row = ny as usize * stride;
                for nx in left..=right {
                    let neighbor = row + nx as usize;
                    if bitmap[neighbor] {
                        bitmap[neighbor] = false;
                        stack.push(neighbor);
                    }
                }
            }
        }

        out.push(Cluster {
            bounds: Rect {
                x: min_x,
                y: min_y,
                width: max_x - min_x + 1,
                height: max_y - min_y + 1,
            },
            diff_pixels,
            displacement: None,
            ssim: None,
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a bitmap from a picture where `#` marks a differing pixel.
    fn bitmap(rows: &[&str]) -> (Vec<bool>, u32, u32) {
        let height = rows.len() as u32;
        let width = rows.first().map_or(0, |row| row.len()) as u32;
        let mut cells = Vec::with_capacity((width * height) as usize);
        for row in rows {
            assert_eq!(row.len() as u32, width, "rows differ in width");
            cells.extend(row.bytes().map(|byte| byte == b'#'));
        }
        (cells, width, height)
    }

    fn rect(x: u32, y: u32, width: u32, height: u32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn an_empty_bitmap_has_no_clusters() {
        let (mut cells, w, h) = bitmap(&["...", "...", "..."]);
        assert!(clusters(&mut cells, w, h).is_empty());
    }

    #[test]
    fn a_single_pixel_is_a_one_by_one_cluster() {
        let (mut cells, w, h) = bitmap(&["...", ".#.", "..."]);
        let found = clusters(&mut cells, w, h);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].bounds, rect(1, 1, 1, 1));
        assert_eq!(found[0].diff_pixels, 1);
    }

    #[test]
    fn diagonally_touching_pixels_are_one_cluster() {
        // Eight-connectivity joins the diagonal; four-connectivity would not.
        let (mut cells, w, h) = bitmap(&["#..", ".#.", "..#"]);
        let found = clusters(&mut cells, w, h);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].bounds, rect(0, 0, 3, 3));
        assert_eq!(found[0].diff_pixels, 3);
    }

    #[test]
    fn a_gap_separates_two_clusters() {
        let (mut cells, w, h) = bitmap(&["#.#", "#.#", "..."]);
        let found = clusters(&mut cells, w, h);
        assert_eq!(found.len(), 2);
        // Raster order: the left column is found first.
        assert_eq!(found[0].bounds, rect(0, 0, 1, 2));
        assert_eq!(found[0].diff_pixels, 2);
        assert_eq!(found[1].bounds, rect(2, 0, 1, 2));
        assert_eq!(found[1].diff_pixels, 2);
    }

    #[test]
    fn a_filled_block_is_one_cluster_covering_its_bounds() {
        let (mut cells, w, h) = bitmap(&[".....", ".###.", ".###.", "....."]);
        let found = clusters(&mut cells, w, h);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].bounds, rect(1, 1, 3, 2));
        assert_eq!(found[0].diff_pixels, 6);
    }

    #[test]
    fn the_walk_empties_the_bitmap() {
        let (mut cells, w, h) = bitmap(&["##", "##"]);
        clusters(&mut cells, w, h);
        assert!(cells.iter().all(|&set| !set), "visited pixels stay set");
    }
}
