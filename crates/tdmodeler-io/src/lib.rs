//! Import/export filters for `tdmodeler`.
//!
//! - [`stl`]: binary and ASCII STL (primary 3D-print / slicing format)
//! - [`obj`]: Wavefront OBJ
//! - [`amf_3mf`]: 3MF (zip + XML, slicer-compatible)
//! - [`bridge`] (feature `manifold`): convert a [`TriangleMesh`] to/from the
//!   [`tdmodeler_core::Solid`] kernel type.

pub mod stl;
pub mod obj;
pub mod amf_3mf;

#[cfg(feature = "manifold")]
pub mod bridge;

use tdmodeler_mesh::TriangleMesh;

#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("truncated or invalid mesh file")]
    Truncated,
    #[error("parse error in mesh file")]
    Parse,
}

/// Convenience: load a mesh from a path, dispatching on extension.
pub fn load_mesh(path: &std::path::Path) -> Result<TriangleMesh, IoError> {
    let data = std::fs::read(path)?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "stl" => {
            // ASCII STL starts with "solid"; binary otherwise.
            if data.len() > 5 && &data[0..5].to_ascii_lowercase() == b"solid" {
                stl::from_ascii_stl_string(&String::from_utf8_lossy(&data))
            } else {
                stl::from_binary_stl_bytes(&data)
            }
        }
        "obj" => obj::from_obj_string(&String::from_utf8_lossy(&data)),
        _ => Err(IoError::Parse),
    }
}
