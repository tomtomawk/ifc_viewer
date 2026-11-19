//! Visée des éléments 2D d'une vue native au clic : annotations (traits, polylignes,
//! cotes, textes), étiquettes de pièces, niveaux et limites de propriété des coupes et
//! élévations. Un trait se vise à moins de `TOLERANCE` pixels de son tracé, un texte dans
//! son cadre ; l'intérieur d'une forme (rectangle englobant, région remplie) ne vise rien,
//! pour laisser sélectionner ce qui est dessous.
use crate::*;
use noyau::trace2d::Forme;

/// Distance maximale au trait, en pixels.
pub(crate) const TOLERANCE: f32 = 5.0;

/// Distance d'un point à un segment.
pub(crate) fn distance_segment(q: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let d = b - a;
    let t = if d.length_sq() > 0.0 { ((q - a).dot(d) / d.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    q.distance(a + d * t)
}

/// Distance d'un point à une primitive projetée à l'écran (`ecran` : point du modèle →
/// écran) : distance au plus proche segment, 0 dans le cadre d'un texte (triangles).
pub(crate) fn distance_primitive(forme: &Forme, q: egui::Pos2, ecran: &dyn Fn(glam::DVec3) -> Option<egui::Pos2>) -> Option<f32> {
    let au_trait = |segments: &mut dyn Iterator<Item = (glam::DVec3, glam::DVec3)>| {
        segments.filter_map(|(a, b)| Some(distance_segment(q, ecran(a)?, ecran(b)?))).min_by(f32::total_cmp)
    };
    match forme {
        Forme::Polyligne { points, fermee } => {
            let n = points.len();
            let fin = if *fermee && n > 2 { n } else { n.saturating_sub(1) };
            au_trait(&mut (0..fin).map(|i| (points[i], points[(i + 1) % n])))
        }
        Forme::Segments(s) => au_trait(&mut s.iter().map(|[a, b]| (*a, *b))),
        Forme::Surface(contours) => au_trait(&mut contours.iter().flat_map(|c| (0..c.len()).map(move |i| (c[i], c[(i + 1) % c.len()])))),
        Forme::Triangles(t) => {
            let points: Vec<egui::Pos2> = t.iter().flatten().filter_map(|p| ecran(*p)).collect();
            let cadre = egui::Rect::from_points(&points).expand(2.0);
            (!points.is_empty() && cadre.contains(q)).then_some(0.0)
        }
    }
}

/// Segments écran d'une primitive ; un texte (triangles) donne son cadre.
fn segments_ecran(forme: &Forme, ecran: &dyn Fn(glam::DVec3) -> Option<egui::Pos2>) -> Vec<[egui::Pos2; 2]> {
    let paire = |a: glam::DVec3, b: glam::DVec3| Some([ecran(a)?, ecran(b)?]);
    match forme {
        Forme::Polyligne { points, fermee } => {
            let n = points.len();
            let fin = if *fermee && n > 2 { n } else { n.saturating_sub(1) };
            (0..fin).filter_map(|i| paire(points[i], points[(i + 1) % n])).collect()
        }
        Forme::Segments(s) => s.iter().filter_map(|[a, b]| paire(*a, *b)).collect(),
        Forme::Surface(contours) => contours.iter().flat_map(|c| (0..c.len()).filter_map(move |i| paire(c[i], c[(i + 1) % c.len()]))).collect(),
        Forme::Triangles(t) => {
            let points: Vec<egui::Pos2> = t.iter().flatten().filter_map(|p| ecran(*p)).collect();
            if points.is_empty() {
                return Vec::new();
            }
            let r = egui::Rect::from_points(&points);
            vec![[r.left_top(), r.right_top()], [r.right_top(), r.right_bottom()], [r.right_bottom(), r.left_bottom()], [r.left_bottom(), r.left_top()]]
        }
    }
}

impl Application {
    /// Point du modèle → écran de la vue `rect`.
    fn ecran_vue(&self, rect: egui::Rect) -> impl Fn(glam::DVec3) -> Option<egui::Pos2> + '_ {
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let origine = self.scene.as_ref().map_or(glam::DVec3::ZERO, |s| s.origin);
        move |x: glam::DVec3| {
            let c = vp * (x - origine).as_vec3().extend(1.0);
            (c.w > 1e-6 && c.z >= -c.w && c.z <= c.w).then(|| egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height()))
        }
    }

    /// Primitives des éléments 2D de la vue native ouverte, avec l'identifiant de leur
    /// élément : annotations et pièces (sans l'aplat), plus niveaux et limites de propriété
    /// si `reperes`.
    fn primitives_2d(&self, reperes: bool) -> Vec<(String, Forme)> {
        let Some((p, _)) = self.natif.as_ref() else { return Vec::new() };
        let Some(vue) = self.vue_native_id().and_then(|id| p.vues.iter().find(|v| v.id == id)) else { return Vec::new() };
        if self.scene.is_none() {
            return Vec::new();
        }
        fn cle(id: &String) -> (u64, &String) { (modele::identifiants::id_natif("annotation", id), id) }
        let mut ids: std::collections::HashMap<u64, &String> = p.annotations.iter().map(|a| cle(&a.id)).chain(p.pieces.iter().map(|x| cle(&x.id))).collect();
        if reperes {
            ids.extend(p.niveaux.iter().map(|x| cle(&x.id)).chain(p.limites_propriete.iter().map(|x| cle(&x.id))));
        }
        ids.retain(|_, id| !vue.elements_masques.contains(id));
        // pièces : seule l'étiquette se vise (l'aplat laisse sélectionner les murs)
        let pieces: std::collections::HashSet<u64> = p.pieces.iter().map(|x| cle(&x.id).0).collect();
        modele::documentation::annotations(p, vue)
            .into_iter()
            .filter_map(|prim| {
                let e = prim.element?;
                let id = *ids.get(&e)?;
                (!(pieces.contains(&e) && matches!(prim.forme, Forme::Surface(_)))).then(|| (id.clone(), prim.forme))
            })
            .collect()
    }

    /// Élément 2D de la vue native ouverte visé au point écran `q` : annotation, pièce
    /// (étiquette), niveau ou limite de propriété (coupes et élévations) ; le plus proche.
    pub(crate) fn element_2d_vise(&self, rect: egui::Rect, q: egui::Pos2) -> Option<String> {
        if !self.point_dans_cadrage(rect, q) {
            return None;
        }
        let ecran = self.ecran_vue(rect);
        self.primitives_2d(true)
            .into_iter()
            .filter_map(|(id, forme)| distance_primitive(&forme, q, &ecran).filter(|d| *d <= TOLERANCE).map(|d| (d, id)))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
    }

    /// Annotations et pièces prises par une fenêtre de sélection `zone` : entièrement
    /// dedans (gauche à droite), ou dont un trait la touche (`croisement`). Le cadre d'un
    /// texte compte, pas l'intérieur d'une forme.
    pub(crate) fn elements_2d_fenetre(&self, rect: egui::Rect, zone: egui::Rect, croisement: bool) -> Vec<String> {
        let ecran = self.ecran_vue(rect);
        let mut segments: std::collections::BTreeMap<String, Vec<[egui::Pos2; 2]>> = Default::default();
        for (id, forme) in self.primitives_2d(false) {
            segments.entry(id).or_default().extend(segments_ecran(&forme, &ecran));
        }
        segments
            .into_iter()
            .filter(|(_, s)| !s.is_empty())
            .filter(|(_, s)| {
                let pris = |[a, b]: &[egui::Pos2; 2]| moteur3d::selection_rectangle::segment(zone, *a, *b, croisement);
                if croisement { s.iter().any(pris) } else { s.iter().all(pris) }
            })
            .map(|(id, _)| id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_vise_a_quelques_pixels_et_pas_dedans() {
        let ecran = |p: glam::DVec3| Some(egui::pos2(p.x as f32, p.y as f32));
        let carre = Forme::Polyligne { points: vec![glam::DVec3::ZERO, glam::DVec3::new(100.0, 0.0, 0.0), glam::DVec3::new(100.0, 100.0, 0.0), glam::DVec3::new(0.0, 100.0, 0.0)], fermee: true };
        // sur le côté de fermeture, à 3 px
        assert_eq!(distance_primitive(&carre, egui::pos2(3.0, 50.0), &ecran), Some(3.0));
        // au milieu : loin de tout trait, rien à viser
        assert!(distance_primitive(&carre, egui::pos2(50.0, 50.0), &ecran).unwrap() > TOLERANCE);
        // ouverte : le côté de fermeture n'existe pas
        let ouverte = Forme::Polyligne { points: match &carre { Forme::Polyligne { points, .. } => points.clone(), _ => unreachable!() }, fermee: false };
        assert!(distance_primitive(&ouverte, egui::pos2(3.0, 50.0), &ecran).unwrap() > TOLERANCE);
    }
}
