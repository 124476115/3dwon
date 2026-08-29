//! Bridge between triangle meshes and the `manifold` kernel (feature `manifold`).

use manifold_rust::manifold::Manifold;
use manifold_rust::types::MeshGL;

use tdmodeler_core::Solid;
use tdmodeler_mesh::TriangleMesh;

use crate::IoError;
use crate::stl;

/// Build a [`Solid`] from a [`TriangleMesh`] by uploading it to the kernel.
pub fn solid_from_mesh(mesh: &TriangleMesh) -> Solid {
    let mut gl = MeshGL::default();
    gl.num_prop = 3;
    let mut props = Vec::with_capacity(mesh.positions.len() * 3);
    for p in &mesh.positions {
        props.extend_from_slice(&[p[0], p[1], p[2]]);
    }
    gl.vert_properties = props;
    gl.tri_verts = mesh.indices.clone();
    Solid::from_manifold(Manifold::from_mesh_gl(&gl))
}

/// Read a binary STL (bytes) and convert it into a kernel [`Solid`].
pub fn solid_from_stl_bytes(bytes: &[u8]) -> Result<Solid, IoError> {
    let mesh = stl::from_binary_stl_bytes(bytes)?;
    Ok(solid_from_mesh(&mesh))
}

pub fn solid_from_stl_file(path: &std::path::Path) -> Result<Solid, IoError> {
    let bytes = std::fs::read(path)?;
    solid_from_stl_bytes(&bytes)
}

/// Tessellate a [`Solid`] into a [`TriangleMesh`] (for rendering / export).
pub fn mesh_from_solid(solid: &Solid) -> TriangleMesh {
    solid.to_mesh()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stl;
    use tdmodeler_core::{features, sketch, solid};

    #[test]
    fn stl_to_solid_to_stl_round_trip() {
        // Build a plate with a circular hole, export to STL, reimport as Solid.
        let plate = sketch::rectangle(20.0, 20.0, true);
        let hole = sketch::circle(4.0, 48);
        let profile = sketch::difference(&plate, &hole);
        let body = features::extrude(&profile, 2.0, 0.0, (1.0, 1.0));
        let mesh = body.to_mesh();
        let stl_bytes = stl::to_binary_stl_bytes(&mesh);

        let solid2 = solid_from_stl_bytes(&stl_bytes).unwrap();
        assert!(solid2.is_valid(), "reimported solid should be manifold");
        let mesh2 = solid2.to_mesh();
        assert!(mesh2.is_watertight(), "reimported mesh should be watertight");
        assert!(mesh2.num_tri() > 0);
    }

    #[test]
    fn imported_solid_preserves_volume() {
        let cube = solid::box_(4.0, 4.0, 4.0, true);
        let mesh = cube.to_mesh();
        let bytes = stl::to_binary_stl_bytes(&mesh);
        let solid2 = solid_from_stl_bytes(&bytes).unwrap();
        let expected = cube.volume();
        let got = solid2.volume();
        assert!((got - expected).abs() / expected < 0.02, "vol {got} vs {expected}");
    }
}
