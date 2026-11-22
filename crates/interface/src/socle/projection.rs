//! Passage monde ↔ écran et molette, communs à toutes les vues : vues du projet, vues
//! planes et 3D de l'éditeur de familles. Une seule implémentation : les superpositions
//! (annotations, poignées, accrochages, contraintes de famille) se placent par
//! [`ecran`] avec la matrice de la caméra qui a servi au rendu.
use crate::*;

/// Coordonnées normalisées [-1, 1] d'un point de l'écran dans `rect`.
pub(crate) fn ndc(rect: egui::Rect, p: egui::Pos2) -> Vec2 {
    Vec2::new((p.x - rect.left()) / rect.width().max(1.0) * 2.0 - 1.0, 1.0 - (p.y - rect.top()) / rect.height().max(1.0) * 2.0)
}

impl Application {
    /// Intersection brute du curseur avec un plan horizontal du projet (mm).
    pub(crate) fn point_plan_horizontal(&self, rect: egui::Rect, q: egui::Pos2, altitude: f64) -> Option<glam::DVec3> {
        let scene = self.scene.as_ref()?;
        let n = ndc(rect, q);
        let (o, d) = self.camera.ray(n.x, n.y, rect.width() / rect.height().max(1.0));
        if d.z.abs() < 1e-6 { return None; }
        let t = (altitude * 0.001 - scene.origin.z - o.z as f64) / d.z as f64;
        (t.is_finite() && t > 0.0).then(|| (o.as_dvec3() + d.as_dvec3() * t + scene.origin) * 1000.0)
    }
}

/// Projection à l'écran d'un point `q` (repère de rendu, m) par la matrice `vp`.
pub(crate) fn ecran(vp: Mat4, rect: egui::Rect, q: Vec3) -> Option<egui::Pos2> {
    let c = vp * q.extend(1.0);
    (c.w > 0.0).then(|| egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height()))
}

/// Zoom de la molette au curseur : en perspective vers `cible` (point visé, sinon le
/// point du plan de la cible sous le curseur) ; en orthographique, le point sous le
/// curseur reste en place.
pub(crate) fn zoom_molette(camera: &mut Camera, rect: egui::Rect, curseur: egui::Pos2, molette: f32, cible: Option<Vec3>) {
    if molette == 0.0 {
        return;
    }
    let facteur = 1.0 / crate::saisie::accrochage::facteur_molette(molette) as f32;
    let aspect = rect.width() / rect.height().max(1.0);
    let n = ndc(rect, curseur);
    if camera.ortho {
        let (avant, _) = camera.ray(n.x, n.y, aspect);
        camera.zoom(facteur, None);
        let (apres, _) = camera.ray(n.x, n.y, aspect);
        let f = camera.forward();
        let d = avant - apres;
        camera.target += d - f * d.dot(f);
    } else {
        let cible = cible.unwrap_or_else(|| {
            let (o, dir) = camera.ray(n.x, n.y, aspect);
            let f = camera.forward();
            o + dir * ((camera.target - o).dot(f) / dir.dot(f).max(1e-6))
        });
        camera.zoom(facteur, Some(cible));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_orthographique_garde_le_point_sous_le_curseur() {
        let mut c = Camera { ortho: true, pitch: MAX_PITCH, yaw: -std::f32::consts::FRAC_PI_2, dist: 10.0, ..Default::default() };
        let rect = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(800.0, 600.0));
        let q = egui::pos2(650.0, 150.0);
        let n = ndc(rect, q);
        let (o, d) = c.ray(n.x, n.y, 800.0 / 600.0);
        let p = o + d * (-o.z / d.z);
        zoom_molette(&mut c, rect, q, 120.0, None);
        assert!(c.dist < 10.0);
        let e = ecran(c.view_proj(800.0 / 600.0), rect, p).unwrap();
        assert!((e - q).length() < 0.05, "{e:?}");
    }
}
