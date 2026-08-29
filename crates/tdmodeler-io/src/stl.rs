//! STL format: binary (preferred for slicing) and ASCII.

use std::path::Path;

use tdmodeler_mesh::math::face_normal;
use tdmodeler_mesh::TriangleMesh;

use crate::IoError;

const HEADER_LEN: usize = 80;
const ATTR: u16 = 0;

pub fn to_binary_stl_bytes(mesh: &TriangleMesh) -> Vec<u8> {
    let mut buf = Vec::with_capacity(HEADER_LEN + 4 + mesh.num_tri() * 50);
    buf.extend_from_slice(&[0u8; HEADER_LEN]);
    buf.extend_from_slice(&(mesh.num_tri() as u32).to_le_bytes());
    for t in 0..mesh.num_tri() {
        let i0 = mesh.indices[3 * t] as usize;
        let i1 = mesh.indices[3 * t + 1] as usize;
        let i2 = mesh.indices[3 * t + 2] as usize;
        let n = face_normal(mesh.positions[i0], mesh.positions[i1], mesh.positions[i2]);
        for c in n {
            buf.extend_from_slice(&c.to_le_bytes());
        }
        for &idx in &[i0, i1, i2] {
            for c in mesh.positions[idx] {
                buf.extend_from_slice(&c.to_le_bytes());
            }
        }
        buf.extend_from_slice(&ATTR.to_le_bytes());
    }
    buf
}

pub fn write_binary_stl(mesh: &TriangleMesh, path: &Path) -> Result<(), IoError> {
    std::fs::write(path, to_binary_stl_bytes(mesh))?;
    Ok(())
}

pub fn from_binary_stl_bytes(bytes: &[u8]) -> Result<TriangleMesh, IoError> {
    if bytes.len() < HEADER_LEN + 4 {
        return Err(IoError::Truncated);
    }
    let tri_count =
        u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    let expected = HEADER_LEN + 4 + tri_count * 50;
    if bytes.len() < expected {
        return Err(IoError::Truncated);
    }
    let mut flat = Vec::with_capacity(tri_count * 3);
    let mut off = HEADER_LEN + 4;
    for _ in 0..tri_count {
        off += 12; // skip normal
        for _ in 0..3 {
            let mut v = [0f32; 3];
            for c in 0..3 {
                let b = [
                    bytes[off],
                    bytes[off + 1],
                    bytes[off + 2],
                    bytes[off + 3],
                ];
                v[c] = f32::from_le_bytes(b);
                off += 4;
            }
            flat.push(v);
        }
        off += 2; // attribute
    }
    Ok(TriangleMesh::from_positions(&flat))
}

pub fn to_ascii_stl_string(mesh: &TriangleMesh) -> String {
    let mut s = String::from("solid tdmodeler\n");
    for t in 0..mesh.num_tri() {
        let i0 = mesh.indices[3 * t] as usize;
        let i1 = mesh.indices[3 * t + 1] as usize;
        let i2 = mesh.indices[3 * t + 2] as usize;
        let n = face_normal(mesh.positions[i0], mesh.positions[i1], mesh.positions[i2]);
        s.push_str(&format!("facet normal {} {} {}\n", n[0], n[1], n[2]));
        s.push_str("  outer loop\n");
        for &idx in &[i0, i1, i2] {
            let p = mesh.positions[idx];
            s.push_str(&format!("    vertex {} {} {}\n", p[0], p[1], p[2]));
        }
        s.push_str("  endloop\nendfacet\n");
    }
    s.push_str("endsolid tdmodeler\n");
    s
}

pub fn from_ascii_stl_string(text: &str) -> Result<TriangleMesh, IoError> {
    let mut flat = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 && parts[0] == "vertex" {
            let x = parts[1].parse::<f32>().map_err(|_| IoError::Parse)?;
            let y = parts[2].parse::<f32>().map_err(|_| IoError::Parse)?;
            let z = parts[3].parse::<f32>().map_err(|_| IoError::Parse)?;
            flat.push([x, y, z]);
        }
    }
    if flat.len() % 3 != 0 {
        return Err(IoError::Parse);
    }
    Ok(TriangleMesh::from_positions(&flat))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tdmodeler_mesh::unit_cube;

    #[test]
    fn binary_round_trip_preserves_geometry() {
        let m = unit_cube();
        let bytes = to_binary_stl_bytes(&m);
        let back = from_binary_stl_bytes(&bytes).unwrap();
        assert_eq!(back.num_tri(), m.num_tri());
        assert!((back.volume() - m.volume()).abs() < 1e-5);
        assert!(back.is_watertight());
    }

    #[test]
    fn ascii_round_trip_preserves_geometry() {
        let m = unit_cube();
        let s = to_ascii_stl_string(&m);
        let back = from_ascii_stl_string(&s).unwrap();
        assert_eq!(back.num_tri(), m.num_tri());
        assert!((back.volume() - m.volume()).abs() < 1e-5);
    }

    #[test]
    fn binary_header_length() {
        let m = unit_cube();
        let bytes = to_binary_stl_bytes(&m);
        assert_eq!(u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]), 12);
    }
}
