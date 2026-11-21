//! Profondeur des passes : matrices d'OpenGL (glam, z de -1 à 1) converties pour wgpu.
//!
//! La vue (modèle, occultation, fenêtres de vue) a une profondeur inversée : 1 au plan
//! proche, 0 au loin. Avec un tampon flottant, la précision relative est alors à peu
//! près la même à toute distance, au lieu de se perdre au loin (d² / proche). Les cartes
//! d'ombres gardent la profondeur directe (0 proche, 1 loin), relue par `ombre_cascade`.

use eframe::wgpu;
use glam::{DMat4, Mat4};

/// Effacement du tampon de profondeur de la vue (le plus loin).
pub const LOIN_INVERSE: f32 = 0.0;
/// Test de profondeur de la vue : le plus proche a la plus grande profondeur.
pub const COMPARAISON_INVERSEE: wgpu::CompareFunction = wgpu::CompareFunction::GreaterEqual;

/// Matrice de la vue : z' = (w - z) / 2. Calculée en f64 : la ligne de z' est la petite
/// différence de deux lignes presque opposées.
pub fn inversee(m: Mat4) -> Mat4 {
    convertir(m, -0.5)
}

/// Matrice des cartes d'ombres : z' = (z + w) / 2.
pub fn directe(m: Mat4) -> Mat4 {
    convertir(m, 0.5)
}

fn convertir(m: Mat4, signe_z: f64) -> Mat4 {
    let mut c = m.as_dmat4().to_cols_array_2d();
    for col in &mut c {
        col[2] = signe_z * col[2] + 0.5 * col[3];
    }
    DMat4::from_cols_array_2d(&c).as_mat4()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Vec3, Vec4};

    fn profondeur(m: Mat4, p: Vec3) -> f32 {
        let c = m * Vec4::new(p.x, p.y, p.z, 1.0);
        c.z / c.w
    }

    /// Caméra de `camera.rs` à 50 m de la cible : plans proche et lointain envoyés à 1 et
    /// 0 ; deux points distants de 2 mm à 100 m distingués en profondeur inversée, et non
    /// en profondeur directe.
    #[test]
    fn inversee_precise_au_loin() {
        let dist = 50.0;
        let (proche, loin) = (dist * 0.002, dist + 400.0);
        let p = glam::camera::rh::proj::opengl::perspective(0.8, 1.5, proche, loin);
        let v = glam::camera::rh::view::look_at_mat4(Vec3::new(120.0, -80.0, 30.0), Vec3::new(120.0, -30.0, 30.0), Vec3::Z);
        let (inv, dir) = (inversee(p * v), directe(p * v));
        let devant = Vec3::new(120.0, -80.0 + proche, 30.0);
        assert!((profondeur(inv, devant) - 1.0).abs() < 1e-4);
        assert!(profondeur(dir, devant).abs() < 1e-4);
        assert!(profondeur(inv, Vec3::new(120.0, -80.0 + loin, 30.0)).abs() < 1e-4);
        let (a, b) = (Vec3::new(121.0, 20.0, 30.5), Vec3::new(121.0, 20.002, 30.5));
        assert!(profondeur(inv, a) > profondeur(inv, b));
        assert!((profondeur(dir, b) - profondeur(dir, a)).abs() <= f32::EPSILON);
    }
}
