//! Measurement utilities (对标 3DOne 测量: 距离、尺寸、包围盒).
//!
//! These operate on [`Solid`] and are pure functions, so they are fully
//! unit-testable without a GPU.

use crate::solid::Solid;

/// Axis-aligned size of a solid: `[width, depth, height]`.
pub fn dimensions(s: &Solid) -> [f64; 3] {
    let (mn, mx) = s.bounding_box();
    [mx[0] - mn[0], mx[1] - mn[1], mx[2] - mn[2]]
}

/// Geometric center of the bounding box.
pub fn center(s: &Solid) -> [f64; 3] {
    let (mn, mx) = s.bounding_box();
    [(mn[0] + mx[0]) / 2.0, (mn[1] + mx[1]) / 2.0, (mn[2] + mx[2]) / 2.0]
}

/// Euclidean distance between two points.
pub fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// Smallest gap between two solids, searching up to `search_length`.
/// Returns `search_length` if no closer approach is found within range.
pub fn min_gap(a: &Solid, b: &Solid, search_length: f64) -> f64 {
    a.manifold.min_gap(&b.manifold, search_length)
}

/// Volume of a solid (delegates to the kernel).
pub fn volume(s: &Solid) -> f64 {
    s.volume()
}

/// Surface area of a solid (delegates to the kernel).
pub fn surface_area(s: &Solid) -> f64 {
    s.surface_area()
}

/// Convenience bundle of statistics for UI / export summaries.
pub struct Stats {
    pub dimensions: [f64; 3],
    pub center: [f64; 3],
    pub volume: f64,
    pub surface_area: f64,
    pub triangles: usize,
}

pub fn stats(s: &Solid) -> Stats {
    Stats {
        dimensions: dimensions(s),
        center: center(s),
        volume: s.volume(),
        surface_area: s.surface_area(),
        triangles: s.num_tri(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solid;

    #[test]
    fn box_dimensions_and_center() {
        let b = solid::box_(4.0, 6.0, 8.0, true);
        assert!((dimensions(&b)[0] - 4.0).abs() < 1e-6);
        assert!((dimensions(&b)[1] - 6.0).abs() < 1e-6);
        assert!((dimensions(&b)[2] - 8.0).abs() < 1e-6);
        let c = center(&b);
        assert!(distance(c, [0.0, 0.0, 0.0]) < 1e-6);
    }

    #[test]
    fn min_gap_between_separated_solids() {
        let a = solid::box_(2.0, 2.0, 2.0, true);
        let b = crate::features::translate(&a, 5.0, 0.0, 0.0);
        // boxes span x in [-1,1] and [4,6]; gap should be 3.0
        let g = min_gap(&a, &b, 100.0);
        assert!((g - 3.0).abs() < 1e-3, "gap={g}");
    }

    #[test]
    fn overlapping_solids_have_zero_gap() {
        let a = solid::box_(2.0, 2.0, 2.0, true);
        let b = solid::box_(2.0, 2.0, 2.0, true);
        let g = min_gap(&a, &b, 100.0);
        assert!(g < 1e-3, "gap={g}");
    }
}
