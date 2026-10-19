//! Caméra orbitale, axe Z vertical.

use glam::{Mat4, Vec3, Vec4};

/// Inclinaison maximale (juste sous la verticale, pour garder look_at défini).
pub const MAX_PITCH: f32 = std::f32::consts::FRAC_PI_2 - 1e-4;

pub struct Camera {
    pub target: Vec3,
    pub dist: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub fov_y: f32,
    pub ortho: bool,
    /// Rayon de la scène, pour les plans de découpe.
    pub scene_radius: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            target: Vec3::ZERO,
            dist: 30.0,
            yaw: -2.2,
            pitch: 0.5,
            fov_y: 45f32.to_radians(),
            ortho: false,
            scene_radius: 20.0,
        }
    }
}

impl Camera {
    pub fn forward(&self) -> Vec3 {
        -Vec3::new(self.pitch.cos() * self.yaw.cos(), self.pitch.cos() * self.yaw.sin(), self.pitch.sin())
    }

    pub fn eye(&self) -> Vec3 {
        self.target - self.forward() * self.dist
    }

    fn up(&self) -> Vec3 {
        if self.pitch.abs() > 1.5 {
            // Vue de dessus/dessous : on utilise la direction horizontale comme haut.
            Vec3::new(self.yaw.cos(), self.yaw.sin(), 0.0) * -self.pitch.signum()
        } else {
            Vec3::Z
        }
    }

    pub fn view(&self) -> Mat4 {
        glam::camera::rh::view::look_at_mat4(self.eye(), self.target, self.up())
    }

    pub fn proj(&self, aspect: f32) -> Mat4 {
        let far = self.dist + self.scene_radius * 4.0 + 1.0;
        if self.ortho {
            let h = self.dist * (self.fov_y * 0.5).tan();
            glam::camera::rh::proj::opengl::orthographic(-h * aspect, h * aspect, -h, h, -far, far)
        } else {
            let near = (self.dist * 0.002).max(self.scene_radius * 1e-5).max(0.005);
            glam::camera::rh::proj::opengl::perspective(self.fov_y, aspect, near, far)
        }
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        self.proj(aspect) * self.view()
    }

    /// Rayon (origine, direction) pour des coordonnées normalisées [-1, 1].
    pub fn ray(&self, ndc_x: f32, ndc_y: f32, aspect: f32) -> (Vec3, Vec3) {
        let inv = self.view_proj(aspect).inverse();
        let a = inv * Vec4::new(ndc_x, ndc_y, -1.0, 1.0);
        let b = inv * Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
        let (a, b) = (a.truncate() / a.w, b.truncate() / b.w);
        (a, (b - a).normalize())
    }

    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.008;
        self.pitch = (self.pitch + dy * 0.008).clamp(-MAX_PITCH, MAX_PITCH);
    }

    pub fn pan(&mut self, dx: f32, dy: f32, viewport_h: f32) {
        let f = self.forward();
        let right = f.cross(self.up()).normalize_or(Vec3::X);
        let up = right.cross(f);
        let k = 2.0 * self.dist * (self.fov_y * 0.5).tan() / viewport_h.max(1.0);
        self.target += (-right * dx + up * dy) * k;
    }

    /// Zoom vers `point` (si fourni) d'un facteur multiplicatif sur la distance.
    pub fn zoom(&mut self, factor: f32, point: Option<Vec3>) {
        let new_dist = (self.dist * factor).max(0.05);
        if let Some(p) = point {
            let eye = self.eye();
            let new_eye = p + (eye - p) * factor;
            self.target = new_eye + self.forward() * new_dist;
        }
        self.dist = new_dist;
    }

    /// Recentre l'orbite sur `p` sans bouger l'œil.
    pub fn set_pivot(&mut self, p: Vec3) {
        let eye = self.eye();
        let d = p - eye;
        let len = d.length();
        if len < 1e-4 {
            return;
        }
        let f = d / len;
        self.target = p;
        self.dist = len;
        self.pitch = (-f.z).clamp(-1.0, 1.0).asin().clamp(-MAX_PITCH, MAX_PITCH);
        self.yaw = (-f.y).atan2(-f.x);
    }

    pub fn fit(&mut self, min: Vec3, max: Vec3) {
        let c = (min + max) * 0.5;
        let r = ((max - min).length() * 0.5).max(0.1);
        self.target = c;
        self.dist = r / (self.fov_y * 0.5).sin() * 1.05;
    }
}
