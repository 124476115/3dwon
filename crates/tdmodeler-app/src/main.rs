//! TDModeler command-line front-end.
//!
//! Provides a headless, scriptable modeling + slicing-export workflow (the same
//! pipeline 3DOne uses to produce STL/3MF for printers) plus, behind the `gui`
//! feature, an interactive `wgpu`/`egui` viewer.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};

use tdmodeler_core::features;
use tdmodeler_core::sketch;
use tdmodeler_core::solid;
use tdmodeler_core::solid::Solid;
use tdmodeler_mesh::TriangleMesh;
use tdmodeler_render::camera::OrbitCamera;

#[derive(Parser)]
#[command(name = "tdmodeler", about = "Cross-platform 3D modeling & slicing export")]
struct Cli {
    /// Launch the interactive viewer instead of running a command (requires the
    /// `gui` feature at build time).
    #[arg(long, global = true)]
    gui: bool,
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Build a demo part (base plate with holes + a peg) and export it.
    Demo {
        /// Output directory (files: model.stl, model.3mf, model.obj).
        #[arg(short, long, default_value = "out")]
        output: PathBuf,
    },
    /// Convert a mesh file to another format (stl/obj/3mf).
    Convert {
        input: PathBuf,
        output: PathBuf,
    },
    /// Print mesh statistics for a file.
    Info { input: PathBuf },
    /// Generate a cuboid.
    Box {
        #[arg(long, default_value_t = 10.0)]
        w: f64,
        #[arg(long, default_value_t = 10.0)]
        h: f64,
        #[arg(long, default_value_t = 10.0)]
        d: f64,
        #[arg(long, default_value_t = true)]
        center: bool,
        output: PathBuf,
    },
    /// Generate a sphere.
    Sphere {
        #[arg(long, default_value_t = 5.0)]
        r: f64,
        #[arg(long, default_value_t = 64)]
        seg: i32,
        output: PathBuf,
    },
    /// Generate a cylinder (along +Z).
    Cylinder {
        #[arg(long, default_value_t = 10.0)]
        h: f64,
        #[arg(long, default_value_t = 3.0)]
        r: f64,
        #[arg(long, default_value_t = 32)]
        seg: i32,
        output: PathBuf,
    },
    /// Generate a cone.
    Cone {
        #[arg(long, default_value_t = 10.0)]
        h: f64,
        #[arg(long, default_value_t = 3.0)]
        r: f64,
        #[arg(long, default_value_t = 32)]
        seg: i32,
        output: PathBuf,
    },
    /// Generate a torus.
    Torus {
        #[arg(long, default_value_t = 10.0)]
        major: f64,
        #[arg(long, default_value_t = 3.0)]
        minor: f64,
        #[arg(long, default_value_t = 48)]
        seg: i32,
        output: PathBuf,
    },
    /// Extrude a rectangle profile into a solid.
    Extrude {
        #[arg(long, default_value_t = 20.0)]
        w: f64,
        #[arg(long, default_value_t = 20.0)]
        h: f64,
        #[arg(long, default_value_t = 10.0)]
        depth: f64,
        output: PathBuf,
    },
    /// Revolve a circle of radius `r` (makes a sphere).
    Revolve {
        #[arg(long, default_value_t = 5.0)]
        r: f64,
        #[arg(long, default_value_t = 64)]
        seg: i32,
        output: PathBuf,
    },
    /// Generate an ellipsoid (a sphere scaled on each axis).
    Ellipsoid {
        #[arg(long, default_value_t = 5.0)]
        rx: f64,
        #[arg(long, default_value_t = 4.0)]
        ry: f64,
        #[arg(long, default_value_t = 3.0)]
        rz: f64,
        #[arg(long, default_value_t = 64)]
        seg: i32,
        output: PathBuf,
    },
    /// Sweep a 2D profile along a direction vector (linear 扫掠).
    Sweep {
        /// profile shape: `rectangle` or `circle`
        #[arg(long, default_value = "rectangle")]
        profile: String,
        /// rectangle side / circle radius
        #[arg(long, default_value_t = 10.0)]
        size: f64,
        /// sweep length
        #[arg(long, default_value_t = 20.0)]
        length: f64,
        /// sweep direction as three numbers: x y z
        #[arg(long, num_args = 3, default_values_t = [0.0, 0.0, 1.0])]
        dir: Vec<f64>,
        output: PathBuf,
    },
    /// Report measurements (dimensions, center, volume, area) for a mesh file.
    Measure { input: PathBuf },
    /// Split a mesh by a plane (normal · x = offset) into two solids (实体分割).
    Split {
        /// plane normal as three numbers: x y z
        #[arg(long, num_args = 3, default_values_t = [1.0, 0.0, 0.0])]
        normal: Vec<f64>,
        /// plane offset along the normal (signed distance from origin)
        #[arg(long, default_value_t = 0.0)]
        offset: f64,
        input: PathBuf,
        output: PathBuf,
    },
    /// Split a mesh into its connected components (STL 工程: 分离分割).
    Decompose { input: PathBuf, output: PathBuf },
    /// Repair inconsistent triangle orientations (STL 工程: 修复法向).
    Repair { input: PathBuf, output: PathBuf },
}

fn export_mesh(mesh: &TriangleMesh, out: &PathBuf) -> Result<()> {
    let ext = out
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .ok_or_else(|| anyhow!("output path has no extension"))?;
    match ext.as_str() {
        "stl" => tdmodeler_io::stl::write_binary_stl(mesh, out)
            .map_err(|e| anyhow!("stl write: {e}"))?,
        "obj" => tdmodeler_io::obj::write_obj(mesh, out)
            .map_err(|e| anyhow!("obj write: {e}"))?,
        "3mf" => tdmodeler_io::amf_3mf::write_3mf(mesh, out)
            .map_err(|e| anyhow!("3mf write: {e}"))?,
        other => return Err(anyhow!("unsupported output extension: {other}")),
    }
    Ok(())
}

fn mesh_from_solid(s: &Solid) -> TriangleMesh {
    s.to_mesh()
}

/// Derive a sibling path `<stem>_<suffix>.<ext>` next to `base`.
fn with_suffix(base: &PathBuf, suffix: &str) -> PathBuf {
    let stem = base
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out".to_string());
    let ext = base
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_else(|| "stl".to_string());
    let mut p = base.clone();
    p.set_file_name(format!("{stem}_{suffix}"));
    p.set_extension(&ext);
    p
}

fn print_stats(name: &str, mesh: &TriangleMesh) {
    let (min, max) = mesh.bounding_box();
    let dims = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
    let center = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0, (min[2] + max[2]) / 2.0];
    println!(
        "{name}: verts={} tris={} volume≈{:.4} watertight={}\n  dims=[{:.2},{:.2},{:.2}] center=[{:.2},{:.2},{:.2}] bounds=[({:.2},{:.2},{:.2})..({:.2},{:.2},{:.2})]",
        mesh.num_vert(),
        mesh.num_tri(),
        mesh.volume(),
        mesh.is_watertight(),
        dims[0], dims[1], dims[2],
        center[0], center[1], center[2],
        min[0], min[1], min[2], max[0], max[1], max[2],
    );
}

fn build_demo() -> Solid {
    let mut part = solid::box_(30.0, 30.0, 6.0, true);
    for (x, y) in [(-9.0, -9.0), (9.0, -9.0), (-9.0, 9.0), (9.0, 9.0)] {
        let hole = features::translate(&solid::cylinder(8.0, 2.0, 2.0, 32), x, y, 0.0);
        part = features::difference(&part, &hole);
    }
    // peg on top, its base sitting on the plate (plate top at z = +3)
    let peg = features::translate(&solid::cylinder(10.0, 3.0, 3.0, 32), 0.0, 0.0, 3.0);
    features::union(&part, &peg)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.gui {
        #[cfg(feature = "gui")]
        {
            return gui::run();
        }
        #[cfg(not(feature = "gui"))]
        {
            return Err(anyhow!(
                "tdmodeler was built without the `gui` feature; rebuild with --features gui"
            ));
        }
    }

    let cmd = cli
        .cmd
        .ok_or_else(|| anyhow!("no subcommand given (use --help)"))?;
    match &cmd {
        Cmd::Demo { output } => {
            std::fs::create_dir_all(output)
                .with_context(|| format!("creating {output:?}"))?;
            let part = build_demo();
            if !part.is_valid() {
                eprintln!("warning: demo solid is not manifold-valid");
            }
            let mesh = mesh_from_solid(&part);
            print_stats("demo", &mesh);

            let stl = output.join("model.stl");
            let m3mf = output.join("model.3mf");
            let obj = output.join("model.obj");
            export_mesh(&mesh, &stl)?;
            export_mesh(&mesh, &m3mf)?;
            export_mesh(&mesh, &obj)?;
            println!("wrote {} {} {}", stl.display(), m3mf.display(), obj.display());
        }
        Cmd::Convert { input, output } => {
            let mesh = tdmodeler_io::load_mesh(input)
                .with_context(|| format!("loading {input:?}"))?;
            print_stats("input", &mesh);
            export_mesh(&mesh, output)?;
            println!("wrote {}", output.display());
        }
        Cmd::Info { input } => {
            let mesh = tdmodeler_io::load_mesh(input)
                .with_context(|| format!("loading {input:?}"))?;
            print_stats("mesh", &mesh);
        }
        Cmd::Box { w, h, d, center, output } => {
            let s = solid::box_(*w, *h, *d, *center);
            export_mesh(&mesh_from_solid(&s), output)?;
            println!("wrote {}", output.display());
        }
        Cmd::Sphere { r, seg, output } => {
            let s = solid::sphere(*r, *seg);
            export_mesh(&mesh_from_solid(&s), output)?;
            println!("wrote {}", output.display());
        }
        Cmd::Cylinder { h, r, seg, output } => {
            let s = solid::cylinder(*h, *r, *r, *seg);
            export_mesh(&mesh_from_solid(&s), output)?;
            println!("wrote {}", output.display());
        }
        Cmd::Cone { h, r, seg, output } => {
            let s = solid::cone(*h, *r, *seg);
            export_mesh(&mesh_from_solid(&s), output)?;
            println!("wrote {}", output.display());
        }
        Cmd::Torus { major, minor, seg, output } => {
            let s = solid::torus(*major, *minor, *seg);
            export_mesh(&mesh_from_solid(&s), output)?;
            println!("wrote {}", output.display());
        }
        Cmd::Extrude { w, h, depth, output } => {
            let cs = sketch::rectangle(*w, *h, true);
            let s = features::extrude(&cs, *depth, 0.0, (1.0, 1.0));
            export_mesh(&mesh_from_solid(&s), output)?;
            println!("wrote {}", output.display());
        }
        Cmd::Revolve { r, seg, output } => {
            let cs = sketch::circle(*r, *seg);
            let s = features::revolve(&cs, *seg, 360.0);
            export_mesh(&mesh_from_solid(&s), output)?;
            println!("wrote {}", output.display());
        }
        Cmd::Ellipsoid { rx, ry, rz, seg, output } => {
            let s = solid::ellipsoid(*rx, *ry, *rz, *seg);
            export_mesh(&mesh_from_solid(&s), output)?;
            println!("wrote {}", output.display());
        }
        Cmd::Sweep {
            profile,
            size,
            length,
            dir,
            output,
        } => {
            let cs = if profile == "circle" {
                sketch::circle(*size, 64)
            } else {
                sketch::rectangle(*size, *size, true)
            };
            let d = [dir[0], dir[1], dir[2]];
            let s = features::sweep_linear(&cs, d, *length);
            if !s.is_valid() {
                eprintln!("warning: swept solid is not manifold-valid");
            }
            export_mesh(&mesh_from_solid(&s), output)?;
            println!("wrote {}", output.display());
        }
        Cmd::Measure { input } => {
            let mesh = tdmodeler_io::load_mesh(input)
                .with_context(|| format!("loading {input:?}"))?;
            print_stats("mesh", &mesh);
        }
        Cmd::Split {
            normal,
            offset,
            input,
            output,
        } => {
            let mesh = tdmodeler_io::load_mesh(input)
                .with_context(|| format!("loading {input:?}"))?;
            let s = Solid::from_mesh(&mesh);
            let (neg, pos) =
                features::split_by_plane(&s, [normal[0], normal[1], normal[2]], *offset);
            let np = with_suffix(output, "neg");
            let pp = with_suffix(output, "pos");
            export_mesh(&neg.to_mesh(), &np)?;
            export_mesh(&pos.to_mesh(), &pp)?;
            println!("wrote {} {}", np.display(), pp.display());
        }
        Cmd::Decompose { input, output } => {
            let mesh = tdmodeler_io::load_mesh(input)
                .with_context(|| format!("loading {input:?}"))?;
            let parts = solid::decompose(&Solid::from_mesh(&mesh));
            if parts.is_empty() {
                return Err(anyhow!("mesh decomposed into zero components"));
            }
            for (i, p) in parts.iter().enumerate() {
                let out = if parts.len() == 1 {
                    output.clone()
                } else {
                    with_suffix(output, &i.to_string())
                };
                export_mesh(&p.to_mesh(), &out)?;
            }
            println!("decomposed into {} component(s)", parts.len());
        }
        Cmd::Repair { input, output } => {
            let mesh = tdmodeler_io::load_mesh(input)
                .with_context(|| format!("loading {input:?}"))?;
            let r = solid::repair(&Solid::from_mesh(&mesh));
            export_mesh(&r.to_mesh(), output)?;
            println!("wrote {}", output.display());
        }
    }

    // Touch the camera import so the viewer crate stays wired even in CLI-only
    // builds (used by the `gui` feature below).
    let _ = std::mem::size_of::<OrbitCamera>();

    Ok(())
}

#[cfg(feature = "gui")]
mod gui;
