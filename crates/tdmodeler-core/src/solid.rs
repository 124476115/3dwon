//! Solid bodies and primitive constructors.

use manifold_rust::cross_section::CrossSection;
use manifold_rust::linalg::{Vec2, Vec3};
use manifold_rust::manifold::Manifold;
use manifold_rust::types::{Error, MeshGL};

use tdmodeler_mesh::TriangleMesh;

/// A solid body backed by the manifold kernel.
#[derive(Clone)]
pub struct Solid {
    pub manifold: Manifold,
}

impl std::fmt::Debug for Solid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Solid(verts={}, tris={})", self.to_mesh().num_vert(), self.to_mesh().num_tri())
    }
}

impl Solid {
    pub fn from_manifold(m: Manifold) -> Self {
        Self { manifold: m }
    }

    /// Tessellate into a render/export-ready triangle mesh (positions only;
    /// normals are recomputed by [`TriangleMesh`]).
    pub fn to_mesh(&self) -> TriangleMesh {
        let gl: MeshGL = self.manifold.get_mesh_gl(0);
        let nv = gl.num_vert();
        let mut positions = Vec::with_capacity(nv);
        for v in 0..nv {
            let p = gl.get_vert_pos(v);
            positions.push([p[0], p[1], p[2]]);
        }
        let indices: Vec<u32> = gl.tri_verts.iter().map(|&i| i as u32).collect();
        TriangleMesh::new(positions, indices)
    }

    pub fn volume(&self) -> f64 {
        self.manifold.volume()
    }
    pub fn surface_area(&self) -> f64 {
        self.manifold.surface_area()
    }
    pub fn num_tri(&self) -> usize {
        self.manifold.num_tri()
    }
    pub fn num_vert(&self) -> usize {
        self.manifold.num_vert()
    }
    pub fn is_empty(&self) -> bool {
        self.manifold.is_empty()
    }
    /// True when the kernel reports no error and the mesh has no self-intersections.
    pub fn is_valid(&self) -> bool {
        self.manifold.status() == Error::NoError && !self.manifold.has_self_intersections()
    }
    pub fn bounding_box(&self) -> ([f64; 3], [f64; 3]) {
        let b = self.manifold.bounding_box();
        (
            [b.min.x, b.min.y, b.min.z],
            [b.max.x, b.max.y, b.max.z],
        )
    }

    /// Build a solid from a render/import mesh by interpreting its triangles as
    /// a (possibly non-manifold) surface and healing it (STL 工程: 修复/导入).
    pub fn from_mesh(mesh: &TriangleMesh) -> Solid {
        let gl = MeshGL {
            num_prop: 3,
            vert_properties: mesh
                .positions
                .iter()
                .flat_map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
                .collect(),
            tri_verts: mesh.indices.iter().map(|&i| i as u32).collect(),
            merge_from_vert: vec![],
            merge_to_vert: vec![],
            run_index: vec![],
            run_original_id: vec![],
            run_transform: vec![],
            face_id: vec![],
            halfedge_tangent: vec![],
            run_flags: vec![],
            tolerance: 0.0,
        };
        Solid::from_manifold(Manifold::from_mesh_gl_robust(&gl))
    }
}

// ---- STL 工程: 分离 / 修复 ----

/// Split a solid into its connected components (分离分割 disconnected shells).
pub fn decompose(s: &Solid) -> Vec<Solid> {
    s.manifold
        .decompose()
        .into_iter()
        .map(Solid::from_manifold)
        .collect()
}

/// Attempt to repair inconsistent triangle orientations (修复法向).
pub fn repair(s: &Solid) -> Solid {
    Solid::from_manifold(s.manifold.repair_orientation())
}

// ---- Primitive constructors (基本实体) ----

pub fn box_(w: f64, h: f64, d: f64, center: bool) -> Solid {
    Solid::from_manifold(Manifold::cube(Vec3::new(w, h, d), center))
}

pub fn sphere(r: f64, segments: i32) -> Solid {
    Solid::from_manifold(Manifold::sphere(r, segments))
}

/// Cylinder along +Z, base at z=0, height `height`.
pub fn cylinder(height: f64, r_low: f64, r_high: f64, segments: i32) -> Solid {
    Solid::from_manifold(Manifold::cylinder(height, r_low, r_high, segments))
}

pub fn cone(height: f64, radius: f64, segments: i32) -> Solid {
    Solid::from_manifold(Manifold::cylinder(height, 0.0, radius, segments))
}

pub fn ellipsoid(rx: f64, ry: f64, rz: f64, segments: i32) -> Solid {
    let s = Manifold::sphere(1.0, segments);
    Solid::from_manifold(s.scale(Vec3::new(rx, ry, rz)))
}

/// Torus: a tube of radius `minor` swept around a circle of radius `major`.
pub fn torus(major: f64, minor: f64, segments: i32) -> Solid {
    let cs = CrossSection::circle(minor, segments).translate(Vec2::new(major, 0.0));
    Solid::from_manifold(Manifold::revolve(&cs.to_polygons(), segments, 360.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::{translate, union};

    #[test]
    fn box_volume() {
        let b = box_(2.0, 3.0, 4.0, true);
        assert!((b.volume() - 24.0).abs() < 1e-6);
        assert!(b.is_valid());
    }

    #[test]
    fn sphere_volume_approx() {
        let r = 1.0;
        let s = sphere(r, 128);
        let expected = 4.0 / 3.0 * std::f64::consts::PI * r * r * r;
        assert!((s.volume() - expected).abs() / expected < 0.02);
        assert!(s.is_valid());
    }

    #[test]
    fn torus_is_valid_solid() {
        let t = torus(5.0, 1.0, 48);
        assert!(t.is_valid());
        assert!(t.num_tri() > 100);
        // volume of torus = 2*pi^2*R*r^2
        let expected = 2.0 * std::f64::consts::PI * std::f64::consts::PI * 5.0 * 1.0 * 1.0;
        assert!((t.volume() - expected).abs() / expected < 0.03);
    }

    #[test]
    fn ellipsoid_axes() {
        let e = ellipsoid(2.0, 1.0, 1.0, 64);
        assert!(e.is_valid());
        let (mn, mx) = e.bounding_box();
        assert!((mx[0] - mn[0] - 4.0).abs() < 1e-6);
    }

    #[test]
    fn from_mesh_roundtrip_preserves_volume() {
        let b = box_(2.0, 2.0, 2.0, true);
        let mesh = b.to_mesh();
        let s = Solid::from_mesh(&mesh);
        assert!(s.is_valid());
        assert!((s.volume() - 8.0).abs() < 1e-3, "vol={}", s.volume());
    }

    #[test]
    fn decompose_separates_disjoint_bodies() {
        let a = box_(2.0, 2.0, 2.0, true);
        let b = translate(&a, 50.0, 0.0, 0.0);
        let u = union(&a, &b);
        let parts = decompose(&u);
        assert_eq!(parts.len(), 2, "expected 2 connected components");
        let total: f64 = parts.iter().map(|p| p.volume()).sum();
        assert!((total - 16.0).abs() < 1e-3, "total={total}");
    }

    #[test]
    fn repair_keeps_valid_solid() {
        let b = box_(3.0, 3.0, 3.0, true);
        let r = repair(&b);
        assert!(r.is_valid());
        assert!((r.volume() - 27.0).abs() < 1e-3);
    }
}
