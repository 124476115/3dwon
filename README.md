# TDModeler

A cross-platform 3D modeling tool (in the spirit of 中望3DOne), focused on
watertight solid modeling and export to the formats 3D printers use — STL, OBJ,
and 3MF. It provides a headless, scriptable CLI pipeline and, behind the `gui`
feature, an interactive `wgpu` + `egui` viewer.

> This program was first created by opencode.

## Features

- CSG solid modeling: box / cylinder / sphere / cone, boolean union /
  difference / intersection, translate / rotate / scale.
- B-rep → triangle mesh with watertight, manifold output.
- Export to STL (ASCII + binary), OBJ, and 3MF.
- Headless CLI (no GPU required) plus an interactive GPU viewer.

## Repository layout

- `crates/tdmodeler-core` — geometry, CSG, and mesh generation.
- `crates/tdmodeler-mesh` — triangle-mesh data structures and validation.
- `crates/tdmodeler-io` — STL / OBJ / 3MF import & export.
- `crates/tdmodeler-render` — `wgpu` rendering pipeline (camera, depth/stencil).
- `crates/tdmodeler-app` — CLI front-end and the optional `gui` viewer.

## Build

```bash
# Headless CLI only (no GPU needed)
cargo build -p tdmodeler-app
cargo test --workspace

# GUI viewer — requires the `gui` feature + a GPU/display
cargo build -p tdmodeler-app --features gui
```

### Running the GUI

On Linux you also need the system display libraries:

```bash
sudo apt-get install libx11-dev libxkbcommon-dev libwayland-dev \
  libxrandr-dev libxinerama-dev libxcursor-dev libegl1-mesa-dev \
  libgl1-mesa-dev libvulkan-dev
```

Then launch the viewer (the `gui` feature must be enabled at build time):

```bash
cargo run -p tdmodeler-app --features gui -- --gui
```

Controls: left-drag to orbit, mouse wheel to zoom.

## CLI examples

```bash
cargo run -p tdmodeler-app -- demo -o out          # export demo part (stl/obj/3mf)
cargo run -p tdmodeler-app -- box -w 10 -h 10 -d 10 -o box.stl
cargo run -p tdmodeler-app -- --help               # all subcommands
```
