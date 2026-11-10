//! Per-pixel blending on straight-alpha RGBA8.

/// Source-over blend of `color` with additional `coverage` (0..=1) onto `dst`.
pub(crate) fn blend_over(dst: &mut [u8], color: [u8; 4], coverage: f32) {
    let sa = f32::from(color[3]) / 255.0 * coverage;
    if sa <= 0.0 {
        return;
    }
    let da = f32::from(dst[3]) / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        dst.fill(0);
        return;
    }
    for i in 0..3 {
        let s = f32::from(color[i]);
        let d = f32::from(dst[i]);
        dst[i] = to_u8((s * sa + d * da * (1.0 - sa)) / out_a);
    }
    dst[3] = to_u8(out_a * 255.0);
}

/// Reduces the alpha of `dst` by `coverage` (0..=1). Fully erased pixels
/// become transparent black so the buffer content stays canonical.
pub(crate) fn erase(dst: &mut [u8], coverage: f32) {
    if coverage <= 0.0 {
        return;
    }
    let a = to_u8(f32::from(dst[3]) * (1.0 - coverage));
    if a == 0 {
        dst.fill(0);
    } else {
        dst[3] = a;
    }
}

fn to_u8(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_over_replaces() {
        let mut px = [10, 20, 30, 255];
        blend_over(&mut px, [200, 100, 50, 255], 1.0);
        assert_eq!(px, [200, 100, 50, 255]);
    }

    #[test]
    fn over_transparent_keeps_source_color() {
        let mut px = [0, 0, 0, 0];
        blend_over(&mut px, [255, 0, 0, 255], 0.5);
        assert_eq!(px, [255, 0, 0, 128]);
    }

    #[test]
    fn half_coverage_mixes_with_opaque_background() {
        let mut px = [255, 255, 255, 255];
        blend_over(&mut px, [0, 0, 0, 255], 0.5);
        assert_eq!(px, [128, 128, 128, 255]);
    }

    #[test]
    fn zero_coverage_is_noop() {
        let mut px = [1, 2, 3, 4];
        blend_over(&mut px, [255, 255, 255, 255], 0.0);
        erase(&mut px, 0.0);
        assert_eq!(px, [1, 2, 3, 4]);
    }

    #[test]
    fn erase_full_and_partial() {
        let mut px = [9, 9, 9, 255];
        erase(&mut px, 0.5);
        assert_eq!(px, [9, 9, 9, 128]);
        erase(&mut px, 1.0);
        assert_eq!(px, [0, 0, 0, 0]);
    }
}
