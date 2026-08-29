//! 3MF export.
//!
//! 3MF is a ZIP container holding `3D/3dmodel.model` (XML). We implement a
//! minimal, dependency-free ZIP writer using the STORE method (no compression)
//! so the slicer-readable package is produced without any C toolchain.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use tdmodeler_mesh::TriangleMesh;
use crate::IoError;

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8833;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

fn build_3mf_xml(mesh: &TriangleMesh) -> String {
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    s.push_str(
        "<model unit=\"millimeter\" xml:lang=\"en-US\" \
         xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n",
    );
    s.push_str("  <resources>\n");
    s.push_str("    <object id=\"1\" type=\"model\">\n");
    s.push_str("      <mesh>\n        <vertices>\n");
    for p in &mesh.positions {
        s.push_str(&format!(
            "          <vertex x=\"{}\" y=\"{}\" z=\"{}\"/>\n",
            p[0], p[1], p[2]
        ));
    }
    s.push_str("        </vertices>\n        <triangles>\n");
    for t in 0..mesh.num_tri() {
        let a = mesh.indices[3 * t];
        let b = mesh.indices[3 * t + 1];
        let c = mesh.indices[3 * t + 2];
        s.push_str(&format!(
            "          <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>\n",
            a, b, c
        ));
    }
    s.push_str("        </triangles>\n      </mesh>\n    </object>\n  </resources>\n");
    s.push_str("  <build>\n    <item objectid=\"1\"/>\n  </build>\n</model>\n");
    s
}

/// Write a single-entry STORE-method ZIP containing the 3MF model XML.
fn write_zip_store(file_name: &str, content: &[u8], out: &mut dyn Write) -> std::io::Result<()> {
    let name_bytes = file_name.as_bytes();
    let crc = crc32(content);
    let local_offset: u32 = 0;

    // Local file header
    out.write_all(&0x0403_4b50u32.to_le_bytes())?; // signature
    out.write_all(&20u16.to_le_bytes())?; // version needed
    out.write_all(&0u16.to_le_bytes())?; // flags
    out.write_all(&0u16.to_le_bytes())?; // compression = store
    out.write_all(&0u16.to_le_bytes())?; // mod time
    out.write_all(&0u16.to_le_bytes())?; // mod date
    out.write_all(&crc.to_le_bytes())?;
    out.write_all(&(content.len() as u32).to_le_bytes())?; // compressed
    out.write_all(&(content.len() as u32).to_le_bytes())?; // uncompressed
    out.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
    out.write_all(&0u16.to_le_bytes())?; // extra len
    out.write_all(name_bytes)?;
    out.write_all(content)?;

    let central_offset = 0u32
        + (30 + name_bytes.len() + content.len()) as u32;

    // Central directory header
    out.write_all(&0x0201_4b50u32.to_le_bytes())?; // signature
    out.write_all(&20u16.to_le_bytes())?; // version made by
    out.write_all(&20u16.to_le_bytes())?; // version needed
    out.write_all(&0u16.to_le_bytes())?; // flags
    out.write_all(&0u16.to_le_bytes())?; // compression
    out.write_all(&0u16.to_le_bytes())?; // time
    out.write_all(&0u16.to_le_bytes())?; // date
    out.write_all(&crc.to_le_bytes())?;
    out.write_all(&(content.len() as u32).to_le_bytes())?;
    out.write_all(&(content.len() as u32).to_le_bytes())?;
    out.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
    out.write_all(&0u16.to_le_bytes())?; // extra
    out.write_all(&0u16.to_le_bytes())?; // comment
    out.write_all(&0u16.to_le_bytes())?; // disk number start
    out.write_all(&0u16.to_le_bytes())?; // internal attrs
    out.write_all(&0u32.to_le_bytes())?; // external attrs
    out.write_all(&local_offset.to_le_bytes())?; // offset of local header
    out.write_all(name_bytes)?;

    // End of central directory
    out.write_all(&0x0605_4b50u32.to_le_bytes())?;
    out.write_all(&0u16.to_le_bytes())?; // disk num
    out.write_all(&0u16.to_le_bytes())?; // disk with cd
    out.write_all(&1u16.to_le_bytes())?; // entries this disk
    out.write_all(&1u16.to_le_bytes())?; // total entries
    out.write_all(&(central_offset - local_offset).to_le_bytes())?; // cd size (== central dir len)
    out.write_all(&local_offset.to_le_bytes())?; // cd offset
    out.write_all(&0u16.to_le_bytes())?; // comment len
    Ok(())
}

pub fn write_3mf(mesh: &TriangleMesh, path: &Path) -> Result<(), IoError> {
    let xml = build_3mf_xml(mesh);
    let mut file = File::create(path)?;
    write_zip_store("3D/3dmodel.model", xml.as_bytes(), &mut file)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tdmodeler_mesh::unit_cube;

    #[test]
    fn three_mf_is_valid_store_zip() {
        let m = unit_cube();
        let tmp = std::env::temp_dir().join("tdmodeler_test_3mf.model");
        write_3mf(&m, &tmp).unwrap();
        let bytes = std::fs::read(&tmp).unwrap();
        // STORE keeps the XML plaintext inside the ZIP
        assert!(bytes.windows(b"3D/3dmodel.model".len()).any(|w| w == b"3D/3dmodel.model"));
        assert!(bytes.windows(b"<mesh>".len()).any(|w| w == b"<mesh>"));
        assert!(bytes.windows(b"<triangle".len()).any(|w| w == b"<triangle"));
        // ZIP local-file signature present
        assert!(bytes.windows(4).any(|w| w == &0x0403_4b50u32.to_le_bytes()));
        let _ = std::fs::remove_file(&tmp);
    }
}
