//! Minimal f32 3D vector math for mesh processing (normals, volume, bounds).

pub type Vec3 = [f32; 3];

#[inline]
pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
pub fn scale(a: Vec3, s: f32) -> Vec3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

#[inline]
pub fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
pub fn length(a: Vec3) -> f32 {
    dot(a, a).sqrt()
}

#[inline]
pub fn normalize(a: Vec3) -> Vec3 {
    let l = length(a);
    if l > 1e-12 {
        scale(a, 1.0 / l)
    } else {
        [0.0, 0.0, 0.0]
    }
}

/// Face normal for a CCW-wound triangle (p1-p0) x (p2-p0).
#[inline]
pub fn face_normal(p0: Vec3, p1: Vec3, p2: Vec3) -> Vec3 {
    normalize(cross(sub(p1, p0), sub(p2, p0)))
}
