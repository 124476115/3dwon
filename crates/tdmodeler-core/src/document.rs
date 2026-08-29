//! Document model: a set of named bodies with an undo/redo command history.

use crate::features;
use crate::solid::Solid;

/// Boolean operation used by [`Document::combine`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    Union,
    Difference,
    Intersection,
}

/// A named solid body with a display color.
#[derive(Debug, Clone)]
pub struct Body {
    pub id: u32,
    pub name: String,
    pub solid: Solid,
    pub color: [f32; 3],
}

#[derive(Debug, Clone)]
pub struct Document {
    pub bodies: Vec<Body>,
    next_id: u32,
    undo_stack: Vec<Vec<Body>>,
    redo_stack: Vec<Vec<Body>>,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    pub fn new() -> Self {
        Self {
            bodies: Vec::new(),
            next_id: 1,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    fn snapshot(&mut self) {
        self.undo_stack.push(self.bodies.clone());
        self.redo_stack.clear();
    }

    /// Add a body, returning its id.
    pub fn add_body(&mut self, name: &str, solid: Solid) -> u32 {
        self.snapshot();
        let id = self.next_id;
        self.next_id += 1;
        self.bodies.push(Body {
            id,
            name: name.to_string(),
            solid,
            color: [0.8, 0.8, 0.85],
        });
        id
    }

    pub fn remove(&mut self, id: u32) {
        if let Some(pos) = self.bodies.iter().position(|b| b.id == id) {
            self.snapshot();
            self.bodies.remove(pos);
        }
    }

    /// Boolean-combine two bodies `a` and `b`, replacing them with the result.
    pub fn combine(&mut self, a: u32, b: u32, op: BoolOp) -> Result<u32, DocError> {
        let pa = self
            .bodies
            .iter()
            .position(|x| x.id == a)
            .ok_or(DocError::NotFound)?;
        let pb = self
            .bodies
            .iter()
            .position(|x| x.id == b)
            .ok_or(DocError::NotFound)?;
        self.snapshot();
        let sa = self.bodies[pa].solid.clone();
        let sb = self.bodies[pb].solid.clone();
        let res = match op {
            BoolOp::Union => features::union(&sa, &sb),
            BoolOp::Difference => features::difference(&sa, &sb),
            BoolOp::Intersection => features::intersection(&sa, &sb),
        };
        let id = self.next_id;
        self.next_id += 1;
        // remove higher index first so the lower index stays valid
        let (hi, lo) = if pa > pb { (pa, pb) } else { (pb, pa) };
        self.bodies.remove(hi);
        self.bodies.remove(lo);
        self.bodies.push(Body {
            id,
            name: format!("combine({a},{b})"),
            solid: res,
            color: [0.8, 0.8, 0.85],
        });
        Ok(id)
    }

    pub fn undo(&mut self) {
        if let Some(snap) = self.undo_stack.pop() {
            self.redo_stack.push(std::mem::replace(&mut self.bodies, snap));
        }
    }

    /// Transform a body in place (translation, Euler rotation in degrees, scale).
    /// Snapshots so it participates in undo/redo.
    pub fn transform_body(
        &mut self,
        id: u32,
        tx: f64,
        ty: f64,
        tz: f64,
        rx: f64,
        ry: f64,
        rz: f64,
        sx: f64,
        sy: f64,
        sz: f64,
    ) {
        if let Some(pos) = self.bodies.iter().position(|b| b.id == id) {
            self.snapshot();
            let s = self.bodies[pos].solid.clone();
            let t = features::translate(&s, tx, ty, tz);
            let r = features::rotate(&t, rx, ry, rz);
            self.bodies[pos].solid = features::scale(&r, sx, sy, sz);
        }
    }

    /// Replace a body with a linear pattern of itself (original included).
    pub fn linear_pattern_body(&mut self, id: u32, dx: f64, dy: f64, dz: f64, count: usize) {
        if let Some(pos) = self.bodies.iter().position(|b| b.id == id) {
            self.snapshot();
            let s = self.bodies[pos].solid.clone();
            self.bodies[pos].solid = features::linear_pattern(&s, dx, dy, dz, count);
        }
    }

    /// Replace a body with a circular pattern of itself around the Z axis.
    pub fn circular_pattern_body(&mut self, id: u32, radius: f64, count: usize) {
        if let Some(pos) = self.bodies.iter().position(|b| b.id == id) {
            self.snapshot();
            let s = self.bodies[pos].solid.clone();
            self.bodies[pos].solid = features::circular_pattern(&s, radius, count);
        }
    }

    pub fn set_body_color(&mut self, id: u32, color: [f32; 3]) {
        if let Some(b) = self.bodies.iter_mut().find(|b| b.id == id) {
            b.color = color;
        }
    }

    pub fn redo(&mut self) {
        if let Some(snap) = self.redo_stack.pop() {
            self.undo_stack.push(std::mem::replace(&mut self.bodies, snap));
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Total triangle count across all bodies (for render budgeting / stats).
    pub fn total_tri(&self) -> usize {
        self.bodies.iter().map(|b| b.solid.num_tri()).sum()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error("body not found")]
    NotFound,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solid;

    #[test]
    fn add_and_count() {
        let mut doc = Document::new();
        doc.add_body("cube", solid::box_(2.0, 2.0, 2.0, true));
        assert_eq!(doc.bodies.len(), 1);
        assert_eq!(doc.total_tri(), doc.bodies[0].solid.num_tri());
    }

    #[test]
    fn combine_and_undo() {
        let mut doc = Document::new();
        let a = doc.add_body("a", solid::box_(4.0, 4.0, 4.0, true));
        let b = doc.add_body("b", solid::sphere(1.0, 32));
        let before = doc.total_tri();
        let _ = doc.combine(a, b, BoolOp::Union);
        assert_eq!(doc.bodies.len(), 1);
        assert!(doc.total_tri() <= before);
        // undo restores the two original bodies
        doc.undo();
        assert_eq!(doc.bodies.len(), 2);
        assert_eq!(doc.total_tri(), before);
    }

    #[test]
    fn redo_restores_combine() {
        let mut doc = Document::new();
        let a = doc.add_body("a", solid::box_(2.0, 2.0, 2.0, true));
        let b = doc.add_body("b", solid::sphere(1.0, 32));
        let _ = doc.combine(a, b, BoolOp::Union);
        doc.undo();
        doc.redo();
        assert_eq!(doc.bodies.len(), 1);
    }

    #[test]
    fn transform_body_changes_extent() {
        let mut doc = Document::new();
        let id = doc.add_body("cube", solid::box_(2.0, 2.0, 2.0, true));
        let before = doc.bodies[0].solid.volume();
        doc.transform_body(id, 5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        doc.undo();
        assert!((doc.bodies[0].solid.volume() - before).abs() < 1e-9);
        doc.redo();
        let (mn, mx) = doc.bodies[0].solid.bounding_box();
        assert!((mx[0] - mn[0] - 2.0).abs() < 1e-6);
        assert!((mn[0] - 4.0).abs() < 1e-6); // shifted +5 from centered [-1,1] -> [4,6]
    }

    #[test]
    fn linear_pattern_body_multiplies_volume() {
        let mut doc = Document::new();
        let id = doc.add_body("unit", solid::box_(1.0, 1.0, 1.0, true));
        let v0 = doc.bodies[0].solid.volume();
        doc.linear_pattern_body(id, 2.0, 0.0, 0.0, 4);
        let v1 = doc.bodies[0].solid.volume();
        assert!((v1 - 4.0 * v0).abs() < 1e-6);
        doc.undo();
        assert!((doc.bodies[0].solid.volume() - v0).abs() < 1e-9);
    }

    #[test]
    fn circular_pattern_body_count() {
        let mut doc = Document::new();
        let id = doc.add_body("peg", solid::box_(1.0, 1.0, 1.0, true));
        let v0 = doc.bodies[0].solid.volume();
        doc.circular_pattern_body(id, 5.0, 6);
        let v1 = doc.bodies[0].solid.volume();
        assert!((v1 - 6.0 * v0).abs() < 1e-6);
    }
}
