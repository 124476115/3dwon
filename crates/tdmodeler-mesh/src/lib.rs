//! `tdmodeler-mesh`: dependency-free triangle-mesh core used by IO, core and render.
//!
//! Provides a plain [`TriangleMesh`] plus f32 vector math used for normals,
//! bounding boxes and volume (divergence theorem).

pub mod math;

use crate::math::*;
use std::collections::HashMap;

/// A triangle mesh in indexed form. Positions are shared; normals are
/// per-vertex (smoothly averaged) and recomputed whenever the mesh changes.
#[derive(Debug, Clone)]
pub struct TriangleMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

impl TriangleMesh {
    pub fn new(positions: Vec<[f32; 3]>, indices: Vec<u32>) -> Self {
        let mut m = Self {
            positions,
            normals: Vec::new(),
            indices,
        };
        m.compute_normals();
        m
    }

    /// Number of triangles (indices.len() / 3).
    pub fn num_tri(&self) -> usize {
        self.indices.len() / 3
    }

    /// Number of unique vertices.
    pub fn num_vert(&self) -> usize {
        self.positions.len()
    }

    /// Recompute smooth per-vertex normals from triangle winding (CCW = front).
    pub fn compute_normals(&mut self) {
        self.normals = vec![[0.0, 0.0, 0.0]; self.positions.len()];
        for t in 0..self.num_tri() {
            let i0 = self.indices[3 * t] as usize;
            let i1 = self.indices[3 * t + 1] as usize;
            let i2 = self.indices[3 * t + 2] as usize;
            let a = self.positions[i0];
            let b = self.positions[i1];
            let c = self.positions[i2];
            let n = cross(sub(b, a), sub(c, a));
            self.normals[i0] = add(self.normals[i0], n);
            self.normals[i1] = add(self.normals[i1], n);
            self.normals[i2] = add(self.normals[i2], n);
        }
        for n in &mut self.normals {
            *n = normalize(*n);
        }
    }

    /// Axis-aligned bounding box `(min, max)`.
    pub fn bounding_box(&self) -> ([f32; 3], [f32; 3]) {
        if self.positions.is_empty() {
            return ([0.0; 3], [0.0; 3]);
        }
        let mut min = self.positions[0];
        let mut max = self.positions[0];
        for p in &self.positions {
            for i in 0..3 {
                min[i] = min[i].min(p[i]);
                max[i] = max[i].max(p[i]);
            }
        }
        (min, max)
    }

    /// Signed volume via the divergence theorem; absolute value for a closed mesh.
    pub fn volume(&self) -> f64 {
        let mut v = 0.0f64;
        for t in 0..self.num_tri() {
            let a = self.positions[self.indices[3 * t] as usize];
            let b = self.positions[self.indices[3 * t + 1] as usize];
            let c = self.positions[self.indices[3 * t + 2] as usize];
            let bxc = cross(b, c);
            v += a[0] as f64 * bxc[0] as f64
                + a[1] as f64 * bxc[1] as f64
                + a[2] as f64 * bxc[2] as f64;
        }
        (v / 6.0).abs()
    }

    /// A mesh is watertight when every undirected edge is shared by exactly two
    /// triangles. This is the main quality gate for printable STL.
    pub fn is_watertight(&self) -> bool {
        if self.indices.is_empty() {
            return false;
        }
        let mut edge_count: HashMap<(u32, u32), u32> = HashMap::new();
        for t in 0..self.num_tri() {
            let idx = [
                self.indices[3 * t],
                self.indices[3 * t + 1],
                self.indices[3 * t + 2],
            ];
            for k in 0..3 {
                let (u, v) = (idx[k], idx[(k + 1) % 3]);
                let key = if u < v { (u, v) } else { (v, u) };
                *edge_count.entry(key).or_insert(0) += 1;
            }
        }
        // every edge exactly twice, and total edges == 3 * tris / 2
        edge_count.values().all(|&c| c == 2)
    }

    /// Build an indexed mesh from a flat list of triangle vertices, welding
    /// coincident vertices (within ~1e-5) so the result is manifold/watertight
    /// when the source geometry is closed. Used by the STL/OBJ readers.
    pub fn from_positions(positions: &[[f32; 3]]) -> Self {
        let mut map: HashMap<[i32; 3], u32> = HashMap::new();
        let mut pos: Vec<[f32; 3]> = Vec::new();
        let mut indices: Vec<u32> = Vec::with_capacity(positions.len());
        let scale = 1e5;
        for &p in positions {
            let key = [
                (p[0] * scale).round() as i32,
                (p[1] * scale).round() as i32,
                (p[2] * scale).round() as i32,
            ];
            let idx = *map.entry(key).or_insert_with(|| {
                let i = pos.len() as u32;
                pos.push(p);
                i
            });
            indices.push(idx);
        }
        Self::new(pos, indices)
    }

    /// Merge this mesh with another by concatenating buffers (no welding).
    pub fn merge(&mut self, other: &TriangleMesh) {
        let offset = self.positions.len() as u32;
        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);
        self.indices
            .extend(other.indices.iter().map(|i| i + offset));
    }
}

/// Build a unit cube (side 1, corner at origin) as a watertight mesh.
pub fn unit_cube() -> TriangleMesh {
    let p = |x: f32, y: f32, z: f32| [x, y, z];
    let positions = vec![
        p(0.0, 0.0, 0.0),
        p(1.0, 0.0, 0.0),
        p(1.0, 1.0, 0.0),
        p(0.0, 1.0, 0.0),
        p(0.0, 0.0, 1.0),
        p(1.0, 0.0, 1.0),
        p(1.0, 1.0, 1.0),
        p(0.0, 1.0, 1.0),
    ];
    // 12 triangles, CCW outward
    let indices = vec![
        0, 2, 1, 0, 3, 2, // bottom (-z) -> actually +z up; keep consistent
        4, 5, 6, 4, 6, 7, // top
        0, 1, 5, 0, 5, 4, // front (y=0)
        2, 3, 7, 2, 7, 6, // back (y=1)
        1, 2, 6, 1, 6, 5, // right (x=1)
        3, 0, 4, 3, 4, 7, // left (x=0)
    ];
    TriangleMesh::new(positions, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_volume_is_one() {
        let c = unit_cube();
        assert!((c.volume() - 1.0).abs() < 1e-6, "volume={}", c.volume());
    }

    #[test]
    fn cube_is_watertight() {
        assert!(unit_cube().is_watertight());
    }

    #[test]
    fn cube_bounding_box() {
        let c = unit_cube();
        let (min, max) = c.bounding_box();
        assert_eq!(min, [0.0, 0.0, 0.0]);
        assert_eq!(max, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn cube_normals_unit_length() {
        let c = unit_cube();
        for n in &c.normals {
            assert!((length(*n) - 1.0).abs() < 1e-5, "normal={:?}", n);
        }
    }

    #[test]
    fn open_mesh_not_watertight() {
        // a single triangle is not closed
        let m = TriangleMesh::new(vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], vec![0, 1, 2]);
        assert!(!m.is_watertight());
    }

    #[test]
    fn merge_preserves_geometry() {
        let mut a = unit_cube();
        let b = unit_cube();
        let vol_before = a.volume();
        a.merge(&b);
        assert!((a.volume() - 2.0 * vol_before).abs() < 1e-6);
    }
}
