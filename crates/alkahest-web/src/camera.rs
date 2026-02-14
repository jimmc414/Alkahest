use alkahest_core::constants::CHUNK_SIZE;
use alkahest_render::CameraUniforms;
use glam::{Mat4, Vec3};

/// Camera mode: orbit around a target or first-person free movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    Orbit,
    FirstPerson,
}

/// Free-orbit or first-person camera.
pub struct Camera {
    // Shared
    pub mode: CameraMode,
    pub yaw: f32,
    pub pitch: f32,
    pub fov_y_rad: f32,
    // Orbit mode
    pub target: Vec3,
    pub distance: f32,
    // First-person mode
    pub fp_position: Vec3,
}

impl Camera {
    pub fn new() -> Self {
        let half = CHUNK_SIZE as f32 / 2.0;
        Self {
            mode: CameraMode::Orbit,
            target: Vec3::new(half, half, half),
            distance: 60.0,
            yaw: std::f32::consts::FRAC_PI_4,
            pitch: -0.4,
            fov_y_rad: std::f32::consts::FRAC_PI_4,
            fp_position: Vec3::new(half, half + 20.0, half),
        }
    }

    pub fn eye_position(&self) -> Vec3 {
        match self.mode {
            CameraMode::Orbit => {
                let x = self.distance * self.pitch.cos() * self.yaw.sin();
                let y = self.distance * self.pitch.sin();
                let z = self.distance * self.pitch.cos() * self.yaw.cos();
                self.target + Vec3::new(x, y, z)
            }
            CameraMode::FirstPerson => self.fp_position,
        }
    }

    /// Orbit camera: rotate around target.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * 0.005;
        self.pitch = (self.pitch - dy * 0.005).clamp(-1.5, 1.5);
    }

    /// Orbit camera: pan target position.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let eye = self.eye_position();
        let forward = (self.target - eye).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward).normalize();

        let speed = self.distance * 0.002;
        self.target += right * (-dx * speed) + up * (dy * speed);
    }

    /// Orbit camera: zoom in/out.
    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance - delta * self.distance * 0.1).clamp(2.0, 200.0);
    }

    /// First-person camera: mouse look.
    pub fn fp_look(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * 0.003;
        self.pitch = (self.pitch - dy * 0.003).clamp(-1.5, 1.5);
    }

    /// First-person camera: forward direction vector (horizontal).
    fn fp_forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos()).normalize()
    }

    /// First-person camera: right direction vector (horizontal).
    fn fp_right(&self) -> Vec3 {
        self.fp_forward().cross(Vec3::Y).normalize()
    }

    /// First-person camera: move with per-axis chunk-level collision.
    /// `chunk_occupied` returns true if the chunk at the given chunk coord has non-air voxels.
    pub fn fp_move(
        &mut self,
        forward: f32,
        right: f32,
        up: f32,
        speed: f32,
        chunk_occupied: impl Fn(i32, i32, i32) -> bool,
    ) {
        let fwd = self.fp_forward();
        let rt = self.fp_right();
        let delta = fwd * forward * speed + rt * right * speed + Vec3::Y * up * speed;

        let cs = CHUNK_SIZE as f32;

        // Per-axis movement with chunk-level collision
        for axis in 0..3 {
            let mut candidate = self.fp_position;
            candidate[axis] += delta[axis];

            let cx = (candidate.x / cs).floor() as i32;
            let cy = (candidate.y / cs).floor() as i32;
            let cz = (candidate.z / cs).floor() as i32;

            if !chunk_occupied(cx, cy, cz) {
                self.fp_position[axis] = candidate[axis];
            }
        }
    }

    /// Toggle between orbit and first-person. Transfers position on switch.
    pub fn toggle_mode(&mut self) {
        match self.mode {
            CameraMode::Orbit => {
                self.fp_position = self.eye_position();
                self.mode = CameraMode::FirstPerson;
            }
            CameraMode::FirstPerson => {
                self.target = self.fp_position + self.fp_forward() * 10.0;
                self.mode = CameraMode::Orbit;
            }
        }
    }

    pub fn view_proj(&self, width: f32, height: f32) -> Mat4 {
        let eye = self.eye_position();
        let look_target = match self.mode {
            CameraMode::Orbit => self.target,
            CameraMode::FirstPerson => {
                let fwd = Vec3::new(
                    self.pitch.cos() * self.yaw.sin(),
                    self.pitch.sin(),
                    self.pitch.cos() * self.yaw.cos(),
                );
                eye + fwd
            }
        };
        let view = Mat4::look_at_rh(eye, look_target, Vec3::Y);
        let aspect = width / height;
        let proj = Mat4::perspective_rh(self.fov_y_rad, aspect, 0.1, 500.0);
        proj * view
    }

    #[allow(clippy::too_many_arguments)]
    pub fn to_uniforms(
        &self,
        width: u32,
        height: u32,
        render_mode: u32,
        clip_axis: u32,
        clip_position: f32,
        cursor_x: u32,
        cursor_y: u32,
    ) -> CameraUniforms {
        let w = width as f32;
        let h = height as f32;
        let vp = self.view_proj(w, h);
        let inv_vp = vp.inverse();
        let eye = self.eye_position();

        CameraUniforms {
            inv_view_proj: inv_vp.to_cols_array_2d(),
            position: [eye.x, eye.y, eye.z, 1.0],
            screen_size: [w, h],
            near: 0.1,
            fov: self.fov_y_rad,
            render_mode,
            clip_axis,
            clip_position: clip_position.to_bits(),
            cursor_packed: cursor_x | (cursor_y << 16),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_mode_toggle() {
        let mut cam = Camera::new();
        assert_eq!(cam.mode, CameraMode::Orbit);
        cam.toggle_mode();
        assert_eq!(cam.mode, CameraMode::FirstPerson);
        cam.toggle_mode();
        assert_eq!(cam.mode, CameraMode::Orbit);
    }

    #[test]
    fn test_fp_collision_blocks_movement_into_occupied_chunk() {
        let mut cam = Camera::new();
        cam.mode = CameraMode::FirstPerson;
        cam.fp_position = Vec3::new(16.0, 16.0, 16.0);
        cam.yaw = 0.0; // forward = +Z

        let initial_pos = cam.fp_position;

        // All chunks occupied → no movement
        cam.fp_move(1.0, 0.0, 0.0, 5.0, |_, _, _| true);
        assert_eq!(
            cam.fp_position, initial_pos,
            "Should not move into occupied chunk"
        );
    }

    #[test]
    fn test_fp_movement_in_empty_world() {
        let mut cam = Camera::new();
        cam.mode = CameraMode::FirstPerson;
        cam.fp_position = Vec3::new(16.0, 16.0, 16.0);
        cam.yaw = 0.0;

        let initial_pos = cam.fp_position;

        // No chunks occupied → free movement
        cam.fp_move(1.0, 0.0, 0.0, 5.0, |_, _, _| false);
        assert_ne!(cam.fp_position, initial_pos, "Should move in empty world");
    }

    #[test]
    fn test_orbit_eye_position() {
        let cam = Camera::new();
        let eye = cam.eye_position();
        // Eye should be away from target
        let dist = (eye - cam.target).length();
        assert!(
            (dist - cam.distance).abs() < 0.1,
            "Eye distance should match camera distance"
        );
    }
}
