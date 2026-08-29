//! 2D sketch primitives and operations, producing a `CrossSection` that can be
//! extruded or revolved into a solid (对标 3DOne 草图模块).

use manifold_rust::cross_section::CrossSection;
use manifold_rust::linalg::Vec2;

/// Axis-aligned rectangle, optionally centered at the origin.
pub fn rectangle(w: f64, h: f64, center: bool) -> CrossSection {
    CrossSection::square_vec2(Vec2::new(w, h), center)
}

/// Circle of given radius.
pub fn circle(r: f64, segments: i32) -> CrossSection {
    CrossSection::circle(r, segments)
}

/// Regular polygon (n sides) centered at origin.
pub fn regular_polygon(radius: f64, sides: usize) -> CrossSection {
    let pts: Vec<Vec2> = (0..sides)
        .map(|i| {
            let a = 2.0 * std::f64::consts::PI * (i as f64) / (sides as f64);
            Vec2::new(radius * a.cos(), radius * a.sin())
        })
        .collect();
    CrossSection::from_polygon_with_fill_rule(pts, 1)
}

/// Arbitrary polygon from 2D points (CCW outer contour).
pub fn polygon(points: &[[f64; 2]]) -> CrossSection {
    let pts: Vec<Vec2> = points.iter().map(|p| Vec2::new(p[0], p[1])).collect();
    CrossSection::from_polygon_with_fill_rule(pts, 1)
}

/// Subtract `b` from `a` (used to cut holes in a profile).
pub fn difference(a: &CrossSection, b: &CrossSection) -> CrossSection {
    a.difference(b)
}

/// Union of two profiles.
pub fn union(a: &CrossSection, b: &CrossSection) -> CrossSection {
    a.union(b)
}

/// Offset a profile by `delta` (positive = outward).
pub fn offset(cs: &CrossSection, delta: f64) -> CrossSection {
    cs.offset(delta)
}

/// Ellipse profile centered at the origin.
pub fn ellipse(rx: f64, ry: f64, segments: i32) -> CrossSection {
    CrossSection::circle(1.0, segments).scale(Vec2::new(rx, ry))
}

/// Sample points along a circular arc (open polyline). Angles are in degrees.
/// Combine with `line`/`polygon` to build profiles that contain arcs.
pub fn arc(cx: f64, cy: f64, r: f64, a0_deg: f64, a1_deg: f64, seg: usize) -> Vec<[f64; 2]> {
    let a0 = a0_deg.to_radians();
    let a1 = a1_deg.to_radians();
    let n = seg.max(1);
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let a = a0 + (a1 - a0) * t;
            [cx + r * a.cos(), cy + r * a.sin()]
        })
        .collect()
}

/// A straight line segment as a 2-point polyline.
pub fn line(a: [f64; 2], b: [f64; 2]) -> Vec<[f64; 2]> {
    vec![a, b]
}

/// Round the corners of a closed polyline by `radius` (sketch 圆角).
///
/// Each convex corner is replaced by a circular arc tangent to both incident
/// edges; the result is a valid filled polygon profile.
pub fn fillet(points: &[[f64; 2]], radius: f64) -> CrossSection {
    let n = points.len();
    if n < 3 {
        return polygon(points);
    }
    let r = radius.abs();
    let pi = std::f64::consts::PI;
    let mut out: Vec<Vec2> = Vec::with_capacity(n * 8);
    let steps = 10usize;
    for i in 0..n {
        let prev = points[(i + n - 1) % n];
        let curr = points[i];
        let next = points[(i + 1) % n];
        let a = [curr[0] - prev[0], curr[1] - prev[1]];
        let b = [next[0] - curr[0], next[1] - curr[1]];
        let la = (a[0] * a[0] + a[1] * a[1]).sqrt();
        let lb = (b[0] * b[0] + b[1] * b[1]).sqrt();
        if la < 1e-9 || lb < 1e-9 {
            out.push(Vec2::new(curr[0], curr[1]));
            continue;
        }
        let da = [a[0] / la, a[1] / la];
        let db = [b[0] / lb, b[1] / lb];
        let cosang = (da[0] * db[0] + da[1] * db[1]).clamp(-1.0, 1.0);
        let ang = cosang.acos();
        let half = ang / 2.0;
        if half < 1e-4 || half > pi / 2.0 - 1e-4 {
            out.push(Vec2::new(curr[0], curr[1]));
            continue;
        }
        let mut t = (r / half.tan()).min(la * 0.5).min(lb * 0.5);
        if t <= 1e-9 {
            out.push(Vec2::new(curr[0], curr[1]));
            continue;
        }
        let start = [curr[0] - da[0] * t, curr[1] - da[1] * t];
        let end = [curr[0] + db[0] * t, curr[1] + db[1] * t];
        // inward bisector points into the polygon interior (CCW winding)
        let bis = [-da[0] + db[0], -da[1] + db[1]];
        let bl = (bis[0] * bis[0] + bis[1] * bis[1]).sqrt();
        let bis = [bis[0] / bl, bis[1] / bl];
        let center = [
            curr[0] + bis[0] * (r / half.sin()),
            curr[1] + bis[1] * (r / half.sin()),
        ];
        let a0 = (start[1] - center[1]).atan2(start[0] - center[0]);
        let a1 = (end[1] - center[1]).atan2(end[0] - center[0]);
        let mut da_arc = a1 - a0;
        while da_arc > pi {
            da_arc -= 2.0 * pi;
        }
        while da_arc < -pi {
            da_arc += 2.0 * pi;
        }
        out.push(Vec2::new(start[0], start[1]));
        for s in 1..steps {
            let ang2 = a0 + da_arc * (s as f64) / (steps as f64);
            out.push(Vec2::new(
                center[0] + r * ang2.cos(),
                center[1] + r * ang2.sin(),
            ));
        }
        out.push(Vec2::new(end[0], end[1]));
    }
    CrossSection::from_polygon_with_fill_rule(out, 1)
}

/// Chamfer (bevel) the corners of a closed polyline by cutting `dist` from the
/// corner along each incident edge (sketch 倒角).
pub fn chamfer(points: &[[f64; 2]], dist: f64) -> CrossSection {
    let n = points.len();
    if n < 3 {
        return polygon(points);
    }
    let d = dist.abs();
    let mut out: Vec<Vec2> = Vec::with_capacity(n * 2);
    for i in 0..n {
        let prev = points[(i + n - 1) % n];
        let curr = points[i];
        let next = points[(i + 1) % n];
        let a = [curr[0] - prev[0], curr[1] - prev[1]];
        let b = [next[0] - curr[0], next[1] - curr[1]];
        let la = (a[0] * a[0] + a[1] * a[1]).sqrt();
        let lb = (b[0] * b[0] + b[1] * b[1]).sqrt();
        let t = d.min(la * 0.5).min(lb * 0.5);
        if t <= 1e-9 {
            out.push(Vec2::new(curr[0], curr[1]));
            continue;
        }
        let da = [a[0] / la, a[1] / la];
        let db = [b[0] / lb, b[1] / lb];
        let start = [curr[0] - da[0] * t, curr[1] - da[1] * t];
        let end = [curr[0] + db[0] * t, curr[1] + db[1] * t];
        out.push(Vec2::new(start[0], start[1]));
        out.push(Vec2::new(end[0], end[1]));
    }
    CrossSection::from_polygon_with_fill_rule(out, 1)
}

/// Area of a profile (used by tests and feature validation).
///
/// Uses the net signed area so that holes (inner contours wound opposite to
/// the outer contour) subtract rather than add. The manifold-rust `area()`
/// sums per-contour absolute areas and therefore double-counts holes.
pub fn area(cs: &CrossSection) -> f64 {
    let polys = cs.to_polygons();
    let mut total = 0.0;
    for p in &polys {
        let n = p.len();
        if n < 3 {
            continue;
        }
        let mut a = 0.0;
        for i in 0..n {
            let c = p[i];
            let d = p[(i + 1) % n];
            a += c.x * d.y - d.x * c.y;
        }
        total += a * 0.5;
    }
    total.abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_area() {
        let r = rectangle(4.0, 2.0, true);
        assert!((area(&r) - 8.0).abs() < 1e-6);
    }

    #[test]
    fn circle_area_approx() {
        let c = circle(1.0, 128);
        let expected = std::f64::consts::PI;
        assert!((area(&c) - expected).abs() / expected < 0.01);
    }

    #[test]
    fn rectangle_with_circular_hole() {
        let outer = rectangle(10.0, 10.0, true);
        let hole = circle(3.0, 64);
        let with_hole = difference(&outer, &hole);
        let expected = 100.0 - std::f64::consts::PI * 9.0;
        let expected = 100.0 - std::f64::consts::PI * 9.0;
        assert!((area(&with_hole) - expected).abs() / expected < 0.01);
    }

    #[test]
    fn polygon_is_ccw_filled() {
        // unit square CCW
        let sq = polygon(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        assert!((area(&sq) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ellipse_area_approx() {
        let e = ellipse(2.0, 1.0, 128);
        let expected = std::f64::consts::PI * 2.0 * 1.0;
        assert!((area(&e) - expected).abs() / expected < 0.02);
    }

    #[test]
    fn fillet_reduces_area() {
        let sq = [
            [-1.0, -1.0],
            [1.0, -1.0],
            [1.0, 1.0],
            [-1.0, 1.0],
        ];
        let base = polygon(&sq);
        let rounded = fillet(&sq, 0.3);
        assert!(area(&rounded) < area(&base));
        assert!(area(&rounded) > 0.0);
        // filleted square should still be ~ a unit-ish square with nibbled corners
        assert!((area(&rounded) - (4.0 - 4.0 * (1.0 - std::f64::consts::PI / 4.0) * 0.3_f64.powi(2))).abs() < 0.1);
    }

    #[test]
    fn chamfer_reduces_area() {
        let sq = [
            [-1.0, -1.0],
            [1.0, -1.0],
            [1.0, 1.0],
            [-1.0, 1.0],
        ];
        let base = polygon(&sq);
        let cut = chamfer(&sq, 0.2);
        assert!(area(&cut) < area(&base));
        assert!(area(&cut) > 0.0);
    }

    #[test]
    fn arc_points_lie_on_circle() {
        let pts = arc(0.0, 0.0, 2.0, 0.0, 90.0, 8);
        assert_eq!(pts.len(), 9);
        for p in &pts {
            let d = (p[0] * p[0] + p[1] * p[1]).sqrt();
            assert!((d - 2.0).abs() < 1e-9);
        }
    }

    #[test]
    fn arc_closed_makes_circle_profile() {
        let mut closed = arc(0.0, 0.0, 1.0, 0.0, 360.0, 64);
        closed.push(closed[0]);
        let cs = polygon(&closed);
        let expected = std::f64::consts::PI;
        assert!((area(&cs) - expected).abs() / expected < 0.02);
    }
}
