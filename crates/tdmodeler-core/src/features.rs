//! Feature operations: booleans, transforms, extrude/revolve, patterns, shell
//! (对标 3DOne 特征造型 / 组合编辑 / 基础编辑).

use manifold_rust::cross_section::CrossSection;
use manifold_rust::linalg::{Mat3x4, Vec2, Vec3};
use manifold_rust::manifold::Manifold;

use crate::solid::Solid;

// ---- 组合编辑: 布尔 ----

pub fn union(a: &Solid, b: &Solid) -> Solid {
    Solid::from_manifold(a.manifold.union(&b.manifold))
}
pub fn difference(a: &Solid, b: &Solid) -> Solid {
    Solid::from_manifold(a.manifold.difference(&b.manifold))
}
pub fn intersection(a: &Solid, b: &Solid) -> Solid {
    Solid::from_manifold(a.manifold.intersection(&b.manifold))
}

// ---- 基础编辑: 变换 ----

pub fn translate(s: &Solid, x: f64, y: f64, z: f64) -> Solid {
    Solid::from_manifold(s.manifold.translate(Vec3::new(x, y, z)))
}
pub fn rotate(s: &Solid, x_deg: f64, y_deg: f64, z_deg: f64) -> Solid {
    Solid::from_manifold(s.manifold.rotate(x_deg, y_deg, z_deg))
}
pub fn scale(s: &Solid, x: f64, y: f64, z: f64) -> Solid {
    Solid::from_manifold(s.manifold.scale(Vec3::new(x, y, z)))
}
pub fn mirror(s: &Solid, nx: f64, ny: f64, nz: f64) -> Solid {
    Solid::from_manifold(s.manifold.mirror(Vec3::new(nx, ny, nz)))
}

// ---- 特征造型: 拉伸 / 旋转 ----

/// Extrude a 2D profile into a solid.
/// `height` is the extrusion distance, `twist_deg` rotates the top face,
/// `scale_top` scales the top face (for tapered extrusions).
pub fn extrude(cs: &CrossSection, height: f64, twist_deg: f64, scale_top: (f64, f64)) -> Solid {
    Solid::from_manifold(Manifold::extrude(
        &cs.to_polygons(),
        height,
        0,
        twist_deg,
        Vec2::new(scale_top.0, scale_top.1),
    ))
}

/// Revolve a 2D profile around the axis by `degrees` (360 = full solid of revolution).
pub fn revolve(cs: &CrossSection, segments: i32, degrees: f64) -> Solid {
    Solid::from_manifold(Manifold::revolve(&cs.to_polygons(), segments, degrees))
}

/// Linear sweep: extrude a 2D profile and orient it so the extrusion axis
/// points along `dir` (arbitrary 3D direction) for `length` units.
/// This is the straight-sweep primitive used by 扫掠.
pub fn sweep_linear(cs: &CrossSection, dir: [f64; 3], length: f64) -> Solid {
    let m = Manifold::extrude(&cs.to_polygons(), length, 0, 0.0, Vec2::new(1.0, 1.0));

    let n = [
        dir[0],
        dir[1],
        dir[2],
    ];
    let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if nl < 1e-9 {
        return Solid::from_manifold(m);
    }
    let n = [n[0] / nl, n[1] / nl, n[2] / nl];

    // Build an orthonormal basis whose third axis is `n`.
    let up = if n[2].abs() < 0.99 {
        [0.0, 0.0, 1.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let t = [
        up[1] * n[2] - up[2] * n[1],
        up[2] * n[0] - up[0] * n[2],
        up[0] * n[1] - up[1] * n[0],
    ];
    let tl = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
    let t = [t[0] / tl, t[1] / tl, t[2] / tl];
    let b = [
        n[1] * t[2] - n[2] * t[1],
        n[2] * t[0] - n[0] * t[2],
        n[0] * t[1] - n[1] * t[0],
    ];

    let mat = Mat3x4 {
        x: Vec3::new(t[0], t[1], t[2]),
        y: Vec3::new(b[0], b[1], b[2]),
        z: Vec3::new(n[0], n[1], n[2]),
        w: Vec3::new(0.0, 0.0, 0.0),
    };
    Solid::from_manifold(m.transform(&mat))
}

// ---- 阵列 ----

pub fn linear_pattern(s: &Solid, dx: f64, dy: f64, dz: f64, count: usize) -> Solid {
    let mut acc = s.clone();
    for i in 1..count {
        let f = i as f64;
        let c = translate(s, dx * f, dy * f, dz * f);
        acc = union(&acc, &c);
    }
    acc
}

/// Circular pattern: `count` copies placed on a ring of `radius` around the Z axis.
pub fn circular_pattern(s: &Solid, radius: f64, count: usize) -> Solid {
    let mut acc = s.clone();
    for i in 1..count {
        let ang = 360.0 * (i as f64) / (count as f64);
        let c = rotate(&translate(s, radius, 0.0, 0.0), 0.0, 0.0, ang);
        acc = union(&acc, &c);
    }
    acc
}

// ---- 实体分割: 平面裁切 ----

/// Split a solid by an infinite plane `normal · x = offset` (实体分割).
/// Returns `(negative_side, positive_side)` keyed by signed distance to the plane.
pub fn split_by_plane(s: &Solid, normal: [f64; 3], offset: f64) -> (Solid, Solid) {
    let n = Vec3::new(normal[0], normal[1], normal[2]);
    let (a, b) = s.manifold.split_by_plane(n, offset);
    (Solid::from_manifold(a), Solid::from_manifold(b))
}

// ---- 特殊功能: 抽壳 (approximate constant-thickness shell) ----

/// Hollow out a solid, keeping `thickness` of material on each face. This is an
/// inward uniform-scale approximation; true offset shells are a P2 item.
pub fn shell(s: &Solid, thickness: f64) -> Solid {
    let inner = scale(s, 1.0 - thickness, 1.0 - thickness, 1.0 - thickness);
    difference(s, &inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sketch;
    use crate::solid;

    #[test]
    fn extrude_rectangle_volume() {
        let cs = sketch::rectangle(2.0, 3.0, true);
        let e = extrude(&cs, 4.0, 0.0, (1.0, 1.0));
        assert!((e.volume() - 24.0).abs() < 1e-4, "vol={}", e.volume());
        assert!(e.is_valid());
    }

    #[test]
    fn revolve_circle_is_cylinder() {
        let cs = sketch::circle(1.0, 64);
        let r = revolve(&cs, 64, 360.0);
        // a revolved circle of radius 1 by 360° is a sphere
        let expected = 4.0 / 3.0 * std::f64::consts::PI;
        assert!((r.volume() - expected).abs() / expected < 0.02);
    }

    #[test]
    fn sweep_linear_along_z_is_extrude() {
        let cs = sketch::rectangle(2.0, 2.0, true);
        let s = sweep_linear(&cs, [0.0, 0.0, 1.0], 5.0);
        assert!((s.volume() - 20.0).abs() < 1e-4, "vol={}", s.volume());
        assert!(s.is_valid());
    }

    #[test]
    fn sweep_linear_along_x_has_correct_extent() {
        let cs = sketch::circle(1.0, 48);
        let s = sweep_linear(&cs, [1.0, 0.0, 0.0], 10.0);
        let (mn, mx) = s.bounding_box();
        assert!((mx[0] - mn[0] - 10.0).abs() < 1e-4);
        // circular cross-section in YZ plane
        assert!((mx[1] - mn[1] - 2.0).abs() < 1e-3);
        assert!((mx[2] - mn[2] - 2.0).abs() < 1e-3);
        assert!(s.is_valid());
    }

    #[test]
    fn split_by_plane_halves_a_cube() {
        let cube = solid::box_(2.0, 2.0, 2.0, true); // spans x in [-1, 1]
        let (neg, pos) = split_by_plane(&cube, [1.0, 0.0, 0.0], 0.0);
        assert!(neg.is_valid());
        assert!(pos.is_valid());
        assert!((neg.volume() - 4.0).abs() < 1e-3, "neg={}", neg.volume());
        assert!((pos.volume() - 4.0).abs() < 1e-3, "pos={}", pos.volume());
    }

    #[test]
    fn union_volume_bounds() {
        let a = solid::box_(2.0, 2.0, 2.0, true);
        let b = translate(&a, 1.0, 0.0, 0.0); // partial overlap
        let u = union(&a, &b);
        assert!(u.volume() <= a.volume() + b.volume() + 1e-6);
        assert!(u.volume() > a.volume());
    }

    #[test]
    fn difference_removes_material() {
        let cube = solid::box_(4.0, 4.0, 4.0, true);
        let cutter = solid::cylinder(10.0, 1.0, 1.0, 32);
        let res = difference(&cube, &cutter);
        assert!(res.is_valid());
        assert!(res.volume() < cube.volume());
        assert!(res.volume() > 0.0);
    }

    #[test]
    fn linear_pattern_count() {
        let unit = solid::box_(1.0, 1.0, 1.0, true);
        let pat = linear_pattern(&unit, 2.0, 0.0, 0.0, 3);
        assert!((pat.volume() - 3.0).abs() < 1e-6);
    }

    #[test]
    fn shell_reduces_volume() {
        let cube = solid::box_(4.0, 4.0, 4.0, true);
        let hollow = shell(&cube, 0.25);
        assert!(hollow.is_valid());
        assert!(hollow.volume() < cube.volume());
        assert!(hollow.volume() > 0.0);
    }
}
