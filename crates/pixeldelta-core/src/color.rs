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

/// Composites a pixel onto an opaque white background.
///
/// Transparency has to be resolved against a fixed background, otherwise two
/// fully transparent pixels of different color would register as different.
fn blend_on_white(pixel: &[u8; 4]) -> [f32; 3] {
    let alpha = f32::from(pixel[3]) / 255.0;
    [
        255.0 + (f32::from(pixel[0]) - 255.0) * alpha,
        255.0 + (f32::from(pixel[1]) - 255.0) * alpha,
        255.0 + (f32::from(pixel[2]) - 255.0) * alpha,
    ]
}

/// Perceptual distance between two RGBA pixels in the YIQ color space.
///
/// The result is non-negative and bounded by [`MAX_COLOR_DELTA`].
pub fn color_delta(a: &[u8; 4], b: &[u8; 4]) -> f32 {
    if a == b {
        return 0.0;
    }

    let [r1, g1, b1] = blend_on_white(a);
    let [r2, g2, b2] = blend_on_white(b);

    let y = (r1 - r2) * R_TO_Y + (g1 - g2) * G_TO_Y + (b1 - b2) * B_TO_Y;
    let i = (r1 - r2) * R_TO_I + (g1 - g2) * G_TO_I + (b1 - b2) * B_TO_I;
    let q = (r1 - r2) * R_TO_Q + (g1 - g2) * G_TO_Q + (b1 - b2) * B_TO_Q;

    Y_WEIGHT * y * y + I_WEIGHT * i * i + Q_WEIGHT * q * q
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_pixels_have_no_delta() {
        assert_eq!(color_delta(&[10, 20, 30, 255], &[10, 20, 30, 255]), 0.0);
    }

    #[test]
    fn red_and_cyan_reach_the_maximum() {
        let delta = color_delta(&[255, 0, 0, 255], &[0, 255, 255, 255]);
        assert!(
            (delta - MAX_COLOR_DELTA).abs() < 1.0,
            "delta {delta} is not close to {MAX_COLOR_DELTA}"
        );
    }

    #[test]
    fn no_pair_exceeds_the_maximum() {
        for r in (0..=255).step_by(15) {
            for g in (0..=255).step_by(15) {
                for b in (0..=255).step_by(15) {
                    let delta = color_delta(&[r, g, b, 255], &[255 - r, 255 - g, 255 - b, 255]);
                    assert!(delta <= MAX_COLOR_DELTA, "delta {delta} for {r},{g},{b}");
                }
            }
        }
    }

    #[test]
    fn delta_is_symmetric() {
        let a = [200, 30, 90, 255];
        let b = [12, 240, 33, 255];
        assert_eq!(color_delta(&a, &b), color_delta(&b, &a));
    }

    #[test]
    fn fully_transparent_pixels_are_equal_whatever_their_color() {
        assert_eq!(color_delta(&[255, 0, 0, 0], &[0, 0, 255, 0]), 0.0);
    }

    #[test]
    fn luminance_dominates_chroma() {
        let base = [128, 128, 128, 255];
        // Same luminance, different chroma.
        let chroma = color_delta(&base, &[100, 140, 140, 255]);
        // Different luminance, comparable RGB distance.
        let luma = color_delta(&base, &[100, 100, 100, 255]);
        assert!(
            luma > chroma,
            "luminance delta {luma} should exceed chroma delta {chroma}"
        );
    }
}
