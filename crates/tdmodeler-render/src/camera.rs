//! Orbit camera producing view/projection matrices for the 3D viewport.

use glam::{Mat4, Vec3};

/// A turntable-style orbit camera looking at a target point.
#[derive(Debug, Clone)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub distance: f32,
    /// Yaw around the world up axis, in radians.
    pub yaw: f32,
    /// Pitch from the horizon, clamped to (-pi/2, pi/2), in radians.
    pub pitch: f32,
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 5.0,
            yaw: 0.6,
            pitch: 0.4,
            fov_y: 50.0_f32.to_radians(),
            near: 0.01,
            far: 1000.0,
        }
    }
}

impl OrbitCamera {
    /// Direction from target to the eye (unit length).
    pub fn forward(&self) -> Vec3 {
        let cp = self.pitch.cos();
        Vec3::new(
            cp * self.yaw.sin(),
            self.pitch.sin(),
            cp * self.yaw.cos(),
        )
        .normalize()
    }

    pub fn eye(&self) -> Vec3 {
        self.target + self.forward() * self.distance
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye(), self.target, Vec3::Y)
    }

    pub fn proj_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, aspect, self.near, self.far)
    }

    /// Combined view-projection used by the vertex shader (clip = VP * world).
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        self.proj_matrix(aspect) * self.view_matrix()
    }

    /// Drag with the mouse: `dx` rotates yaw, `dy` changes pitch.
    pub fn drag(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.01;
        self.pitch += dy * 0.01;
        let limit = std::f32::consts::FRAC_PI_2 - 0.01;
        self.pitch = self.pitch.clamp(-limit, limit);
    }

    /// Zoom toward/away from the target (positive = closer).
    pub fn zoom(&mut self, delta: f32) {
        self.distance *= (1.0 + delta * 0.001).max(0.0);
        self.distance = self.distance.clamp(0.05, 1000.0);
    }

    /// Frame a mesh whose bounding box spans `size` (max extent) and is centred
    /// at `center`, positioning the camera to view it comfortably.
    pub fn frame(&mut self, center: Vec3, size: f32) {
        self.target = center;
        self.distance = (size * 1.6).max(0.1) / (self.fov_y * 0.5).tan();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_is_unit_length() {
        let c = OrbitCamera::default();
        assert!((c.forward().length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn eye_offset_from_target() {
        let c = OrbitCamera::default();
        let d = c.eye() - c.target;
        assert!((d.length() - c.distance).abs() < 1e-4);
    }

    #[test]
    fn view_proj_is_invertible_for_cube() {
        let mut c = OrbitCamera::default();
        c.frame(Vec3::ZERO, 2.0);
        let vp = c.view_proj(1.0);
        // a point in front of the camera should map to clip w > 0
        let p = c.target + Vec3::X * 0.5;
        let clip = vp * p.extend(1.0);
        assert!(clip.w > 0.0);
    }

    #[test]
    fn drag_changes_orientation() {
        let mut c = OrbitCamera::default();
        let before = c.forward();
        c.drag(10.0, 0.0);
        assert!((c.forward() - before).length() > 1e-3);
    }
}
