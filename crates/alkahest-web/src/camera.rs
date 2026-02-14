use alkahest_core::constants::CHUNK_SIZE;
use alkahest_render::CameraUniforms;
use glam::{Mat4, Vec3};

/// Free-orbit camera around a target point.
pub struct Camera {
    pub target: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub fov_y_rad: f32,
}

impl Camera {
    pub fn new() -> Self {
        let half = CHUNK_SIZE as f32 / 2.0;
        Self {
            target: Vec3::new(half, half, half),
            distance: 60.0,
            yaw: std::f32::consts::FRAC_PI_4,
            pitch: -0.4,
            fov_y_rad: std::f32::consts::FRAC_PI_4,
        }
    }

    pub fn eye_position(&self) -> Vec3 {
        let x = self.distance * self.pitch.cos() * self.yaw.sin();
        let y = self.distance * self.pitch.sin();
        let z = self.distance * self.pitch.cos() * self.yaw.cos();
        self.target + Vec3::new(x, y, z)
    }

    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * 0.005;
        self.pitch = (self.pitch - dy * 0.005).clamp(-1.5, 1.5);
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        let eye = self.eye_position();
        let forward = (self.target - eye).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward).normalize();

        let speed = self.distance * 0.002;
        self.target += right * (-dx * speed) + up * (dy * speed);
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance - delta * self.distance * 0.1).clamp(2.0, 200.0);
    }

    pub fn view_proj(&self, width: f32, height: f32) -> Mat4 {
        let eye = self.eye_position();
        let view = Mat4::look_at_rh(eye, self.target, Vec3::Y);
        let aspect = width / height;
        let proj = Mat4::perspective_rh(self.fov_y_rad, aspect, 0.1, 500.0);
        proj * view
    }

    pub fn to_uniforms(&self, width: u32, height: u32, render_mode: u32) -> CameraUniforms {
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
            _pad_rm: [0; 3],
        }
    }
}
