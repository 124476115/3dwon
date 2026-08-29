//! `tdmodeler-core`: the modeling kernel.
//!
//! Wraps the pure-Rust `manifold-rust` geometry kernel behind a small,
//! testable API: [`Solid`] (a manifold body), sketch primitives, feature
//! operations (extrude/revolve/boolean/transform/pattern) and a
//! [`document::Document`] with undo/redo.

pub mod solid;
pub mod sketch;
pub mod features;
pub mod document;
pub mod measure;

pub use tdmodeler_mesh::TriangleMesh;
