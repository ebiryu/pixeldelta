/// Largest value [`color_delta`] can return, reached between opaque red and
/// opaque cyan. Thresholds are expressed as a fraction of this value.
pub const MAX_COLOR_DELTA: f32 = 35215.0;

// RGB to YIQ, the transform pixelmatch and odiff use.
const R_TO_Y: f32 = 0.298_895_3;
const G_TO_Y: f32 = 0.586_622_5;
const B_TO_Y: f32 = 0.114_482_23;
const R_TO_I: f32 = 0.595_977_99;
const G_TO_I: f32 = -0.274_176_1;
const B_TO_I: f32 = -0.321_801_9;
const R_TO_Q: f32 = 0.211_470_17;
const G_TO_Q: f32 = -0.522_617_1;
const B_TO_Q: f32 = 0.311_146_94;

// Weights placing most of the distance on luminance.
const Y_WEIGHT: f32 = 0.5053;
const I_WEIGHT: f32 = 0.299;
const Q_WEIGHT: f32 = 0.1957;

// The two shades the background alternates between, and the byte strides at
// which the green and blue channels switch between them. The strides are the
// golden ratio and its square, which no image content lines up with.
const BACKGROUND_DARK: f32 = 48.0;
const BACKGROUND_RANGE: f32 = 159.0;
const RED_STRIDE: f64 = 1.0;
const GREEN_STRIDE: f64 = 1.618_033_988_749_895;
const BLUE_STRIDE: f64 = 2.618_033_988_749_895;

/// Background a semi-transparent pixel at byte `offset` is composited onto.
///
/// A single background color hides differences in content close to that color,
/// so each channel of the background alternates between a dark and a light
/// shade along the buffer. The red channel switches every byte, so for the
/// four-byte-aligned offsets of whole pixels it stays on the dark shade.
fn checkerboard_background(offset: usize) -> [f32; 3] {
    let position = offset as f64;
    let shade = |stride: f64| {
        let cell = (position / stride) as u64;
        BACKGROUND_DARK + BACKGROUND_RANGE * (cell % 2) as f32
    };
    [shade(RED_STRIDE), shade(GREEN_STRIDE), shade(BLUE_STRIDE)]
}

/// Perceptual distance between two RGBA pixels in the YIQ color space.
///
/// `offset` is the byte offset of the pixel within its image, which selects the
/// background semi-transparent pixels are composited onto. Pixels that are
/// opaque in both images do not depend on it.
///
/// The result is non-negative and bounded by [`MAX_COLOR_DELTA`].
pub fn color_delta(a: &[u8; 4], b: &[u8; 4], offset: usize) -> f32 {
    if a == b {
        return 0.0;
    }

    let [dr, dg, db] = channel_deltas(a, b, offset);

    let y = dr * R_TO_Y + dg * G_TO_Y + db * B_TO_Y;
    let i = dr * R_TO_I + dg * G_TO_I + db * B_TO_I;
    let q = dr * R_TO_Q + dg * G_TO_Q + db * B_TO_Q;

    Y_WEIGHT * y * y + I_WEIGHT * i * i + Q_WEIGHT * q * q
}

/// Brightness difference between two RGBA pixels, positive when `a` is the
/// brighter one.
///
/// This is the luminance term of [`color_delta`] on its own, kept signed. The
/// anti-aliasing detector needs the direction of the difference, which the
/// squared distance discards.
pub fn brightness_delta(a: &[u8; 4], b: &[u8; 4], offset: usize) -> f32 {
    if a == b {
        return 0.0;
    }

    let [dr, dg, db] = channel_deltas(a, b, offset);

    dr * R_TO_Y + dg * G_TO_Y + db * B_TO_Y
}

/// Per-channel differences the YIQ transform is applied to.
///
/// Opaque pixels contribute their raw channel differences. If either pixel is
/// semi-transparent, both are composited onto the background at `offset` first.
fn channel_deltas(a: &[u8; 4], b: &[u8; 4], offset: usize) -> [f32; 3] {
    if a[3] == 255 && b[3] == 255 {
        return [
            f32::from(a[0]) - f32::from(b[0]),
            f32::from(a[1]) - f32::from(b[1]),
            f32::from(a[2]) - f32::from(b[2]),
        ];
    }

    let alpha_a = f32::from(a[3]);
    let alpha_b = f32::from(b[3]);
    let da = alpha_a - alpha_b;
    let background = checkerboard_background(offset);
    let mut deltas = [0.0; 3];
    for (channel, delta) in deltas.iter_mut().enumerate() {
        *delta = (f32::from(a[channel]) * alpha_a
            - f32::from(b[channel]) * alpha_b
            - background[channel] * da)
            / 255.0;
    }
    deltas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_pixels_have_no_delta() {
        assert_eq!(color_delta(&[10, 20, 30, 255], &[10, 20, 30, 255], 0), 0.0);
    }

    #[test]
    fn red_and_cyan_reach_the_maximum() {
        let delta = color_delta(&[255, 0, 0, 255], &[0, 255, 255, 255], 0);
        assert!(
            (delta - MAX_COLOR_DELTA).abs() < 1.0,
            "delta {delta} is not close to {MAX_COLOR_DELTA}"
        );
    }

    #[test]
    fn no_pair_exceeds_the_maximum() {
        // Blending moves each channel difference within the same range it has
        // for opaque pixels, so the bound has to hold on both paths.
        for alpha in [0u8, 1, 64, 128, 254, 255] {
            for offset in [0usize, 4, 8, 12] {
                for r in (0..=255).step_by(15) {
                    for g in (0..=255).step_by(15) {
                        for b in (0..=255).step_by(15) {
                            let delta = color_delta(
                                &[r, g, b, alpha],
                                &[255 - r, 255 - g, 255 - b, 255],
                                offset,
                            );
                            assert!(
                                delta <= MAX_COLOR_DELTA,
                                "delta {delta} for {r},{g},{b} at alpha {alpha}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn delta_is_symmetric() {
        let a = [200, 30, 90, 255];
        let b = [12, 240, 33, 255];
        assert_eq!(color_delta(&a, &b, 0), color_delta(&b, &a, 0));
    }

    #[test]
    fn fully_transparent_pixels_are_equal_whatever_their_color() {
        assert_eq!(color_delta(&[255, 0, 0, 0], &[0, 0, 255, 0], 0), 0.0);
    }

    #[test]
    fn half_transparent_white_differs_from_opaque_white() {
        // On a white background both pixels resolve to white and the
        // difference disappears. The checkerboard background keeps it.
        let delta = color_delta(&[255, 255, 255, 128], &[255, 255, 255, 255], 0);
        assert!(delta > 0.0, "delta {delta} lost the alpha difference");
    }

    #[test]
    fn the_background_alternates_between_two_shades() {
        // Same pixel pair at two offsets that land on different background
        // shades in the blue channel.
        let pair = ([0u8, 0, 255, 128], [0u8, 0, 255, 255]);
        let dark = color_delta(&pair.0, &pair.1, 0);
        let light = color_delta(&pair.0, &pair.1, 12);
        assert_ne!(dark, light);
    }

    #[test]
    fn opaque_pixels_ignore_the_offset() {
        let a = [12, 200, 40, 255];
        let b = [200, 12, 40, 255];
        assert_eq!(color_delta(&a, &b, 0), color_delta(&a, &b, 4096));
    }

    #[test]
    fn luminance_dominates_chroma() {
        let base = [128, 128, 128, 255];
        // Same luminance, different chroma.
        let chroma = color_delta(&base, &[100, 140, 140, 255], 0);
        // Different luminance, comparable RGB distance.
        let luma = color_delta(&base, &[100, 100, 100, 255], 0);
        assert!(
            luma > chroma,
            "luminance delta {luma} should exceed chroma delta {chroma}"
        );
    }
}
