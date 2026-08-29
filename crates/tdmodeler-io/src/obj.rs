//! Wavefront OBJ format.

use std::path::Path;

use tdmodeler_mesh::TriangleMesh;
use crate::IoError;

pub fn write_obj(mesh: &TriangleMesh, path: &Path) -> Result<(), IoError> {
    std::fs::write(path, to_obj_string(mesh))?;
    Ok(())
}

pub fn to_obj_string(mesh: &TriangleMesh) -> String {
    let mut s = String::from("# TDModeler export\n");
    for p in &mesh.positions {
        s.push_str(&format!("v {} {} {}\n", p[0], p[1], p[2]));
    }
    for n in &mesh.normals {
        s.push_str(&format!("vn {} {} {}\n", n[0], n[1], n[2]));
    }
    for t in 0..mesh.num_tri() {
        let a = mesh.indices[3 * t] + 1;
        let b = mesh.indices[3 * t + 1] + 1;
        let c = mesh.indices[3 * t + 2] + 1;
        s.push_str(&format!("f {}//{} {}//{} {}//{}\n", a, a, b, b, c, c));
    }
    s
}

pub fn from_obj_string(text: &str) -> Result<TriangleMesh, IoError> {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut faces: Vec<Vec<u32>> = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                let x = it.next().and_then(|s| s.parse::<f32>().ok()).ok_or(IoError::Parse)?;
                let y = it.next().and_then(|s| s.parse::<f32>().ok()).ok_or(IoError::Parse)?;
                let z = it.next().and_then(|s| s.parse::<f32>().ok()).ok_or(IoError::Parse)?;
                positions.push([x, y, z]);
            }
            Some("f") => {
                let mut face = Vec::new();
                for tok in it {
                    let v = tok.split('/').next().unwrap_or(tok);
                    let parsed = v.parse::<i64>().map_err(|_| IoError::Parse)?;
                    let idx = if parsed < 0 {
                        (positions.len() as i64 + parsed) as u32
                    } else {
                        (parsed - 1) as u32
                    };
                    face.push(idx);
                }
                if !face.is_empty() {
                    faces.push(face);
                }
            }
            _ => {}
        }
    }
    let mut indices = Vec::new();
    for face in &faces {
        if face.len() < 3 {
            return Err(IoError::Parse);
        }
        for k in 1..face.len() - 1 {
            indices.push(face[0]);
            indices.push(face[k]);
            indices.push(face[k + 1]);
        }
    }
    Ok(TriangleMesh::new(positions, indices))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tdmodeler_mesh::unit_cube;

    #[test]
    fn obj_round_trip() {
        let m = unit_cube();
        let s = to_obj_string(&m);
        let back = from_obj_string(&s).unwrap();
        assert_eq!(back.num_tri(), m.num_tri());
        assert!((back.volume() - m.volume()).abs() < 1e-5);
    }
}
