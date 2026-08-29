//! Rendering layer for TDModeler.
//!
//! `camera` is always available (pure Rust, testable). The `wgpu`-backed
//! [`Renderer`] lives behind the `gui` feature so the crate — and the whole
//! workspace — builds on headless/CI machines that lack a GPU and display
//! libraries. On a desktop target enable `--features gui` to compile the GPU
//! viewer.

pub mod camera;

#[cfg(feature = "gui")]
pub mod renderer;
