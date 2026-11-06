//! Ajuster/prolonger graphique : une arête limite (n'importe quelle arête d'un mur ou
//! d'une poutre), puis les extrémités visées. La ligne prolongée est la ligne de
//! justification du mur (l'axe d'une poutre) : les raccords se font sur les faces.
use super::*;
use crate::modele::{Projet, edition::commandes::Commande};

#[derive(Default)]
pub(super) struct EtatAjustement {
    pub reference: Option<String>,
    /// Arête limite choisie (plan, mm).
    ligne: Option<[[f64; 2]; 2]>,
    fin_reference: bool,
    survol: Option<(String, bool)>,
    /// Arête sous le curseur avant le choix de la limite.
    survol_arete: Option<(String, [[f64; 2]; 2])>,
    cache: Option<(u64, String, String, bool, bool, Option<bool>, Option<[[u64; 2]; 2]>)>,
    apercu: Option<Result<Apercu, String>>,
}
struct Apercu {
    id: String,
    fin: bool,
    a: [f64; 2],
    b: [f64; 2],
    ancien: [f64; 2],
    point: [f64; 2],
    z: f64,
    contours: Vec<Vec<[f64; 2]>>,
    angle: Option<(bool, [f64; 2], [f64; 2], f64)>,
}

fn cle_ligne(l: Option<[[f64; 2]; 2]>) -> Option<[[u64; 2]; 2]> {
    l.map(|l| l.map(|p| p.map(f64::to_bits)))
}

fn preparer(
    p: &Projet,
    id: &str,
    reference: &str,
    ligne: Option<[[f64; 2]; 2]>,
    fin: bool,
    prolonger: bool,
    angle: Option<bool>,
) -> Result<Apercu, String> {
    let (a, b) = p
        .ligne_ajustable(id)
        .ok_or("Choisissez un mur droit, un assemblage droit ou une poutre.")?;
    let z = p.altitude_ajustable(id).unwrap_or(0.0);
    let ancien = if fin { b } else { a };
    let fixe = if fin { a } else { b };
    let limite = match (angle, ligne) {
        (Some(_), _) | (None, None) => p.ligne_ajustable(reference).map(|(a, b)| [a, b]).ok_or("Référence absente")?,
        (None, Some(l)) => l,
    };
    let point = p.cible_justification(id, limite, fin)?;
    let direction = glam::DVec2::from_array(ancien) - glam::DVec2::from_array(fixe);
    if prolonger
        && (glam::DVec2::from_array(point) - glam::DVec2::from_array(ancien)).dot(direction) <= 1e-6
    {
        return Err(
            "Cette limite raccourcit le mur. Choisissez le mode « Ajuster ou prolonger ».".into(),
        );
    }
    // La prévisualisation utilise la même transaction validée que le clic final.
    let commande = commande_ajuster(id, reference, fin, angle, ligne);
    if prolonger && angle.is_some() {
        let (ra, rb) = p.ligne_ajustable(reference).ok_or("Référence absente")?;
        let (ancien, fixe) = if angle == Some(true) { (rb, ra) } else { (ra, rb) };
        if (glam::DVec2::from_array(point) - glam::DVec2::from_array(ancien))
            .dot(glam::DVec2::from_array(ancien) - glam::DVec2::from_array(fixe)) < -1e-6 {
            return Err("La limite serait raccourcie : choisissez « Ajuster ou prolonger ».".into());
        }
    }
    let (q, _) = p.executer(commande)?;
    let (a, b) = q.ligne_ajustable(id).ok_or("Ligne ajustée absente")?;
    let coupes = crate::modele::murs::jonctions::coupes(&q);
    let contours = q
        .murs
        .iter()
        .filter(|m| m.id == id || m.assemblage.as_deref() == Some(id) || (angle.is_some() && (m.id == reference || m.assemblage.as_deref() == Some(reference))))
        .filter_map(|m| crate::modele::murs::jonctions::contour(&q, m, &coupes).map(|c| c.to_vec()))
        .chain([id, reference].into_iter().filter(|x| *x == id || angle.is_some()).flat_map(|x| {
            // poutres : rectangle de la section en plan
            let a = q.aretes_plan(x);
            (q.barres.iter().any(|b| b.id == x) && a.len() == 4).then(|| a.iter().map(|e| e[0]).collect::<Vec<_>>())
        }))
        .collect();
    Ok(Apercu {
        id: id.into(),
        fin,
        a,
        b,
        ancien,
        point,
        z,
        contours,
        angle: match angle {
            Some(fin) => {
                let (a, b) = q.ligne_ajustable(reference).ok_or("Référence introuvable après l'ajustement.")?;
                Some((fin, a, b, q.altitude_ajustable(reference).unwrap_or(z)))
            }
            None => None,
        },
    })
}

fn commande_ajuster(id: &str, reference: &str, fin: bool, angle: Option<bool>, ligne: Option<[[f64; 2]; 2]>) -> Commande {
    match angle {
        Some(fin_reference) => Commande::AjusterAngle { id: id.into(), reference: reference.into(), fin, fin_reference },
        None => Commande::Ajuster { id: id.into(), reference: reference.into(), fin, ligne },
    }
}

impl ViewerApp {
    pub(super) fn etat_ajustement(&self) -> serde_json::Value {
        let a = self.ajustement.as_ref();
        let apercu = a.and_then(|a| a.apercu.as_ref());
        serde_json::json!({"actif":self.ruban_edition.outil()==Some(crate::ui::ruban::Outil::Ajuster),"angle":self.ruban_edition.prolonger_angle,"reference":a.and_then(|a|a.reference.as_ref()),"ligne":a.and_then(|a|a.ligne),"survol":a.and_then(|a|a.survol.as_ref()),
            "apercu":apercu.and_then(|r|r.as_ref().ok()).map(|p|serde_json::json!({"id":p.id,"fin":p.fin,"point":p.point})),
            "erreur":apercu.and_then(|r|r.as_ref().err())})
    }
    pub(super) fn preparer_outil_ajuster(&mut self) -> bool {
        let actif = self.ruban_edition.outil() == Some(crate::ui::ruban::Outil::Ajuster)
            && self.natif.is_some()
            && self.loading.is_none();
        if actif {
            let etat = self.ajustement.get_or_insert_with(Default::default);
            if etat
                .reference
                .as_ref()
                .is_some_and(|id| self.natif.as_ref().unwrap().0.ligne_ajustable(id).is_none() && self.natif.as_ref().unwrap().0.aretes_plan(id).is_empty())
            {
                *etat = Default::default();
            }
        } else {
            self.ajustement = None;
        }
        actif
    }

    /// Éléments ajustables visibles : murs (simples ou composés) et poutres.
    fn ajustables_visibles(&self) -> Vec<String> {
        let Some((p, _)) = &self.natif else { return Vec::new() };
        p.murs
            .iter()
            .filter(|m| m.assemblage.is_none())
            .map(|m| (&m.id, "mur"))
            .chain(p.assemblages.iter().map(|a| (&a.id, "assemblage")))
            .chain(p.barres.iter().filter(|b| b.genre == crate::modele::GenreBarre::Poutre).map(|b| (&b.id, "barre")))
            .filter(|(id, genre)| self.objects_under(scene::id_natif(genre, id)).iter().any(|i| self.visible.get(*i as usize).copied().unwrap_or(false)))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Distance à l'écran du point `q` au segment (a, b) du plan à l'altitude `z`.
    fn distance_ecran(&self, rect: egui::Rect, q: egui::Pos2, a: [f64; 2], b: [f64; 2], z: f64) -> Option<f32> {
        let (a, b) = (self.ecran_plan(rect, a, z)?, self.ecran_plan(rect, b, z)?);
        let d = b - a;
        let t = if d.length_sq() > 0.0 { ((q - a).dot(d) / d.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
        Some(q.distance(a + t * d))
    }

    /// Arête la plus proche du curseur, sur n'importe quel mur ou poutre visible.
    fn arete_ajuster_visee(&self, rect: egui::Rect, q: egui::Pos2) -> Option<(String, [[f64; 2]; 2])> {
        let (p, _) = self.natif.as_ref()?;
        if !self.point_dans_cadrage(rect, q) {
            return None;
        }
        self.ajustables_visibles()
            .into_iter()
            .flat_map(|id| {
                let z = p.altitude_ajustable(&id).unwrap_or(0.0);
                p.aretes_plan(&id).into_iter().map(move |e| (id.clone(), e, z))
            })
            .filter_map(|(id, e, z)| {
                let d = self.distance_ecran(rect, q, e[0], e[1], z)?;
                (d <= 12.0).then_some((d, id, e))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id, e)| (id, e))
    }

    /// Mur ou poutre visé et extrémité la plus proche du curseur.
    fn mur_ajuster_vise(
        &self,
        rect: egui::Rect,
        q: egui::Pos2,
        hit: Option<u32>,
    ) -> Option<(String, bool)> {
        let p = &self.natif.as_ref()?.0;
        let s = self.scene.as_ref()?;
        let candidat = |id: String| -> Option<(String, bool)> {
            let (a, b) = p.ligne_ajustable(&id)?;
            let z = p.altitude_ajustable(&id)?;
            let (a, b) = (self.ecran_plan(rect, a, z)?, self.ecran_plan(rect, b, z)?);
            Some((id, q.distance(b) <= q.distance(a)))
        };
        if let Some(id) = hit
            .and_then(|i| s.objects.get(i as usize))
            .and_then(|o| self.cle_native(self.cible_clic(o.id)))
        {
            if let Some(c) = candidat(id) {
                return Some(c);
            }
        }
        if !self.point_dans_cadrage(rect, q) {
            return None;
        }
        // Tolérance sur toutes les arêtes pour les vues au trait et les murs très fins.
        self.ajustables_visibles()
            .into_iter()
            .filter_map(|id| {
                let z = p.altitude_ajustable(&id)?;
                let d = p.aretes_plan(&id).into_iter().filter_map(|e| self.distance_ecran(rect, q, e[0], e[1], z)).fold(f32::MAX, f32::min);
                (d <= 12.0).then_some((d, id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .and_then(|(_, id)| candidat(id))
    }

    pub(super) fn souris_ajuster(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        resp: &egui::Response,
        hit: Option<u32>,
    ) {
        if resp.clicked_by(PointerButton::Secondary) {
            if self.ajustement.as_ref().is_some_and(|a| a.reference.is_some()) {
                self.ajustement = Some(Default::default());
            } else {
                self.ruban_edition.annuler();
                self.ajustement = None;
            }
            return;
        }
        let angle = self.ruban_edition.prolonger_angle;
        let curseur = resp.hover_pos().or(resp.interact_pointer_pos());
        let choix_arete = !angle && self.ajustement.as_ref().is_some_and(|e| e.reference.is_none());
        let survol_arete = if choix_arete { curseur.and_then(|q| self.arete_ajuster_visee(rect, q)) } else { None };
        let survol = curseur.and_then(|q| self.mur_ajuster_vise(rect, q, hit));
        let prolonger = self
            .ruban_edition
            .texte_option(crate::ui::ruban::Outil::Ajuster, 0)
            == "Prolonger seulement";
        let Some(etat) = self.ajustement.as_mut() else {
            return;
        };
        etat.survol = survol.clone();
        etat.survol_arete = survol_arete.clone();
        if let (Some(reference), Some((id, fin))) = (&etat.reference, &survol) {
            let cle = (
                self.bim.revision,
                reference.clone(),
                id.clone(),
                *fin,
                prolonger,
                angle.then_some(etat.fin_reference),
                cle_ligne(etat.ligne),
            );
            if etat.cache.as_ref() != Some(&cle) {
                etat.apercu = Some(preparer(
                    &self.natif.as_ref().unwrap().0,
                    id,
                    reference,
                    etat.ligne,
                    *fin,
                    prolonger,
                    angle.then_some(etat.fin_reference),
                ));
                etat.cache = Some(cle);
            }
        } else {
            etat.apercu = None;
            etat.cache = None;
        }
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
        if !resp.clicked_by(PointerButton::Primary) {
            return;
        }
        if etat.reference.is_none() {
            if angle {
                if let Some((id, fin)) = survol {
                    etat.fin_reference = fin;
                    etat.reference = Some(id);
                    etat.cache = None;
                    self.status = "Première extrémité choisie : cliquez près de la seconde pour raccorder.".into();
                }
            } else if let Some((id, ligne)) = survol_arete {
                etat.reference = Some(id);
                etat.ligne = Some(ligne);
                etat.cache = None;
                self.status = "Arête limite choisie : cliquez près de l'extrémité à ajuster ou prolonger.".into();
            }
            return;
        }
        let commande = match &etat.apercu {
            Some(Ok(a)) => etat.reference.as_deref().map(|r| commande_ajuster(&a.id, r, a.fin, a.angle.map(|a| a.0), etat.ligne)),
            Some(Err(e)) => {
                self.status = e.clone();
                None
            }
            None => None,
        };
        if let Some(c) = commande {
            match self.executer_bim(c) {
                Ok(_) => {
                    self.status = if angle { "Angle créé." } else { "Élément ajusté — autre extrémité, ou Échap pour changer d'arête." }.into();
                    if angle {
                        self.ajustement = Some(Default::default());
                    }
                }
                Err(e) => self.error = Some(e),
            }
            if let Some(e) = &mut self.ajustement {
                e.cache = None;
                e.apercu = None;
            }
        }
    }

    pub(super) fn dessiner_ajustement(&self, ui: &egui::Ui, rect: egui::Rect) {
        let Some(e) = &self.ajustement else { return };
        let Some((p, _)) = &self.natif else { return };
        let painter = ui.painter().with_clip_rect(rect);
        let orange = Color32::from_rgb(220, 125, 10);
        let bleu = Color32::from_rgb(0, 125, 215);
        let segment = |a: [f64; 2], b: [f64; 2], z: f64, couleur: Color32, infini: bool| {
            if let (Some(a), Some(b)) = (self.ecran_plan(rect, a, z), self.ecran_plan(rect, b, z)) {
                if infini {
                    let u = (b - a).normalized();
                    painter.add(egui::Shape::dashed_line(&[a - u * rect.size().length(), b + u * rect.size().length()], egui::Stroke::new(1.5, couleur), 7.0, 4.0));
                }
                painter.line_segment([a, b], egui::Stroke::new(3.0, couleur));
            }
        };
        let ligne = |id: &str, couleur: Color32, infini: bool| {
            if let (Some((a, b)), Some(z)) = (p.ligne_ajustable(id), p.altitude_ajustable(id)) {
                segment(a, b, z, couleur, infini);
            }
        };
        if self.ruban_edition.prolonger_angle {
            if let (Some((a, b)), Some(z)) = (e.reference.as_deref().and_then(|id| p.ligne_ajustable(id)), e.reference.as_deref().and_then(|id| p.altitude_ajustable(id))) {
                if let Some(q) = self.ecran_plan(rect, if e.fin_reference { b } else { a }, z) {
                    painter.circle_stroke(q, 7.0, egui::Stroke::new(2.5, orange));
                }
            }
        }
        // arête limite (choisie, ou survolée avant le choix), prolongée en pointillés
        match (&e.reference, e.ligne, &e.survol_arete) {
            (Some(id), Some(l), _) => segment(l[0], l[1], p.altitude_ajustable(id).unwrap_or(0.0), orange, true),
            (Some(id), None, _) => ligne(id, orange, true),
            (None, _, Some((id, l))) => segment(l[0], l[1], p.altitude_ajustable(id).unwrap_or(0.0), orange, true),
            _ => {}
        }
        if let Some((id, _)) = &e.survol {
            if e.reference.is_some() && e.reference.as_ref() != Some(id) {
                ligne(id, bleu, false);
            }
        }
        match &e.apercu {
            Some(Ok(a)) => {
                if let Some((_, x, y, z)) = a.angle {
                    segment(x, y, z, bleu, false);
                }
                for c in &a.contours {
                    let pts: Vec<_> = c.iter().filter_map(|p| self.ecran_plan(rect, *p, a.z)).collect();
                    if pts.len() >= 3 {
                        painter.add(egui::Shape::convex_polygon(pts, bleu.gamma_multiply(0.2), egui::Stroke::new(1.5, bleu)));
                    }
                }
                if let (Some(x), Some(y), Some(ancien), Some(point)) = (
                    self.ecran_plan(rect, a.a, a.z),
                    self.ecran_plan(rect, a.b, a.z),
                    self.ecran_plan(rect, a.ancien, a.z),
                    self.ecran_plan(rect, a.point, a.z),
                ) {
                    painter.line_segment([x, y], egui::Stroke::new(2.5, bleu));
                    painter.add(egui::Shape::dashed_line(&[ancien, point], egui::Stroke::new(2.0, Color32::from_rgb(210, 70, 55)), 5.0, 3.0));
                    painter.circle(point, 6.0, Color32::WHITE, egui::Stroke::new(2.0, bleu));
                }
            }
            Some(Err(erreur)) => {
                if let Some(q) = ui.input(|i| i.pointer.hover_pos()).filter(|q| rect.contains(*q)) {
                    painter.text(q + egui::vec2(15.0, 20.0), egui::Align2::LEFT_TOP, erreur, egui::FontId::proportional(12.0), Color32::from_rgb(185, 55, 40));
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn angle_raccorde_deux_lignes_de_justification_en_une_transaction() {
        let mut p = Projet::depuis_gabarit("Angle", &crate::config::gabarit::Gabarit::defaut());
        let id = p.ajouter_mur([0., 0.], [2000., 0.], &Default::default()).unwrap();
        let reference = p.ajouter_mur([4000., -3000.], [4000., -1000.], &Default::default()).unwrap();
        let (ja, _) = p.ligne_ajustable(&reference).unwrap();
        let apercu = preparer(&p, &id, &reference, None, true, true, Some(true)).unwrap();
        let (q, ids) = p.executer(commande_ajuster(&id, &reference, true, Some(true), None)).unwrap();
        assert_eq!(ids, vec![id.clone(), reference.clone()]);
        // les deux lignes de justification se rejoignent au point de l'aperçu
        assert_eq!(q.ligne_ajustable(&id).unwrap().1, apercu.point);
        assert_eq!(q.ligne_ajustable(&reference).unwrap().1, apercu.point);
        assert_eq!(apercu.point[0], ja[0]);
        assert!(preparer(&p, &id, &reference, None, true, false, Some(false)).is_err());
        assert_eq!(p.axe_element(&id).unwrap().1, [2000., 0.]);
    }
    #[test]
    fn prolonge_la_justification_jusqu_a_une_arete_quelconque() {
        let mut p = Projet::depuis_gabarit("Ajuster", &crate::config::gabarit::Gabarit::defaut());
        let cible = p.ajouter_mur([0.0, 0.0], [2000.0, 0.0], &Default::default()).unwrap();
        let limite = p.ajouter_mur([4000.0, -2000.0], [4000.0, 2000.0], &Default::default()).unwrap();
        // chaque arête du mur limite donne un point différent ; la justification y arrive
        let aretes: Vec<_> = p.aretes_plan(&limite).into_iter().filter(|e| (e[0][0] - e[1][0]).abs() < 1e-6).collect();
        assert!(aretes.len() >= 2);
        let mut xs = Vec::new();
        for e in &aretes {
            let (q, _) = p.executer(Commande::Ajuster { id: cible.clone(), reference: limite.clone(), fin: true, ligne: Some(*e) }).unwrap();
            let (_, b) = q.ligne_ajustable(&cible).unwrap();
            assert!((b[0] - e[0][0]).abs() < 1e-6, "justification sur l'arête : {b:?} {e:?}");
            xs.push(b[0]);
        }
        xs.sort_by(f64::total_cmp);
        assert!(xs.last().unwrap() - xs.first().unwrap() > 50.0, "faces distinctes : {xs:?}");
        assert!(preparer(&p, &cible, &limite, None, false, false, None).is_err());
        let parallele = p.ajouter_mur([0.0, 3000.0], [3000.0, 3000.0], &Default::default()).unwrap();
        assert!(preparer(&p, &cible, &parallele, None, true, false, None).is_err());
    }
    #[test]
    fn poutre_prolongee_jusqu_a_l_arete_d_un_mur() {
        let mut p = Projet::depuis_gabarit("Poutre", &crate::config::gabarit::Gabarit::defaut());
        let mur = p.ajouter_mur([4000.0, -2000.0], [4000.0, 2000.0], &Default::default()).unwrap();
        let niveau = p.niveaux[0].id.clone();
        let b: crate::modele::Barre = serde_json::from_value(serde_json::json!({
            "id": "poutre-1", "nom": "Poutre", "genre": "poutre", "niveau": niveau,
            "debut": [0.0, 0.0, 2500.0], "fin": [2000.0, 0.0, 2500.0], "largeur": 200.0, "hauteur": 400.0
        })).unwrap();
        p.barres.push(b);
        let arete = p.aretes_plan(&mur).into_iter().find(|e| (e[0][0] - e[1][0]).abs() < 1e-6).unwrap();
        let (q, _) = p.executer(Commande::Ajuster { id: "poutre-1".into(), reference: mur, fin: true, ligne: Some(arete) }).unwrap();
        let b = q.barres.iter().find(|b| b.id == "poutre-1").unwrap();
        assert!((b.fin[0] - arete[0][0]).abs() < 1e-6 && b.fin[2] == 2500.0, "{:?}", b.fin);
        assert_eq!(p.aretes_plan("poutre-1").len(), 4);
    }
}
