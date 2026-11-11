//! Commandes du ruban Modifier appliquées aux traits d'un tracé en cours : une seule
//! fonction calcule le résultat, pour l'aperçu sous le curseur comme pour le clic.
use super::*;


use crate::ui::ruban::CibleTrace;

/// Contours après la commande au point `p`. `Ok(None)` : il manque encore un point
/// (départ, repère) ; `Err` : la commande ne peut pas s'appliquer ici.
#[allow(clippy::too_many_arguments)]
pub(super) fn transformer_contours(
    contours: &[Vec<[f64; 2]>],
    commande: CommandeContour,
    segment: Option<(usize, usize)>,
    sommet: Option<(usize, usize)>,
    depart: Option<DVec2>,
    repere: Option<DVec2>,
    p: DVec2,
    reference: Option<[DVec2; 2]>,
) -> Result<Option<Vec<Vec<[f64; 2]>>>, String> {
    use CommandeContour as C;
    // sommets visés : le sommet sélectionné, ou les deux extrémités du trait
    let (c, cibles) = match (sommet, segment) {
        (Some((c, i)), _) => (c, vec![i]),
        (None, Some((c, i))) => (c, vec![i, (i + 1) % contours.get(c).map_or(1, |x| x.len().max(1))]),
        (None, None) => return Err("Sélectionnez un trait ou un sommet.".into()),
    };
    if contours.get(c).is_none_or(|x| cibles.iter().any(|k| *k >= x.len())) {
        return Err("Sélectionnez un trait ou un sommet.".into());
    }
    let mut r = contours.to_vec();
    let point = |k: usize| DVec2::from_array(contours[c][k]);
    let mut deplacer = |f: &dyn Fn(DVec2) -> DVec2| for &k in &cibles { r[c][k] = f(point(k)).to_array(); };
    let trait_ = || segment.map(|(_, i)| (point(i), point((i + 1) % contours[c].len()))).ok_or_else(|| "Sélectionnez un trait.".to_string());
    match commande {
        C::Deplacer => {
            let Some(d) = depart else { return Ok(None) };
            deplacer(&|q| q + (p - d));
        }
        C::Copier | C::Reseau(_) => {
            let Some(d) = depart else { return Ok(None) };
            let (a, b) = trait_()?;
            let copies = if let C::Reseau(n) = commande { n.max(1) } else { 1 };
            for pas in 1..=copies {
                let delta = (p - d) * pas as f64;
                r.push(vec![(a + delta).to_array(), (b + delta).to_array()]);
            }
        }
        C::Symetrie => {
            let Some(d) = depart else { return Ok(None) };
            if (p - d).length_squared() <= 1e-12 { return Ok(None); }
            deplacer(&|q| DVec2::from_array(crate::ui::transformer::miroir_point(q.to_array(), d.to_array(), p.to_array())));
        }
        C::Tourner | C::Echelle => {
            let (Some(centre), Some(repere)) = (depart, repere) else { return Ok(None) };
            let (initial, final_) = (repere - centre, p - centre);
            if initial.length_squared() <= 1e-12 || final_.length_squared() <= 1e-12 { return Ok(None); }
            let angle = initial.perp_dot(final_).atan2(initial.dot(final_));
            let facteur = final_.length() / initial.length();
            deplacer(&|q| centre + if commande == C::Tourner { DVec2::from_angle(angle).rotate(q - centre) } else { (q - centre) * facteur });
        }
        C::Aligner | C::Ajuster => {
            let (a, b) = trait_()?;
            let Some([ra, rb]) = reference else { return Err("Cliquez un autre segment du tracé.".into()) };
            let (direction, v) = (rb - ra, b - a);
            if direction.length_squared() <= 1e-12 || v.length_squared() <= 1e-12 { return Err("Segment de longueur nulle.".into()); }
            if commande == C::Aligner {
                // le trait vient sur la droite de la référence : ses deux extrémités y sont
                // projetées (simple translation si les segments sont parallèles)
                let u = direction.normalize();
                if v.dot(u).abs() <= 1e-6 * v.length() { return Err("Le trait est perpendiculaire à la référence : il n'aurait plus de longueur.".into()); }
                deplacer(&|q| ra + u * (q - ra).dot(u));
            } else {
                let denominateur = v.perp_dot(direction);
                if denominateur.abs() <= 1e-9 { return Err("Les segments sont parallèles.".into()); }
                let intersection = a + v * ((ra - a).perp_dot(direction) / denominateur);
                let k = if a.distance_squared(intersection) <= b.distance_squared(intersection) { cibles[0] } else { cibles[1] };
                r[c][k] = intersection.to_array();
            }
        }
        C::Scinder => {
            let (a, b) = trait_()?;
            let direction = b - a;
            let t = if direction.length_squared() > 1e-12 { (p - a).dot(direction) / direction.length_squared() } else { 0.0 };
            if !(t > 1e-6 && t < 1.0 - 1e-6) { return Err("Cliquez à l'intérieur du segment.".into()); }
            r[c].insert(cibles[0] + 1, (a + direction * t).to_array());
        }
        C::Decaler => {
            let (a, b) = trait_()?;
            let Some(normale) = (b - a).try_normalize().map(|d| d.perp()) else { return Err("Segment de longueur nulle.".into()) };
            let delta = normale * (p - a).dot(normale);
            deplacer(&|q| q + delta);
        }
    }
    Ok(Some(r))
}

impl ViewerApp {
    /// Cible des commandes du ruban Modifier dans le tracé en cours, topologie figée
    /// (pas d'ajout ni de retrait de sommet) et raison pour laquelle un trait ne s'y
    /// copie pas.
    pub(in crate::ui) fn cible_trace(&self) -> (CibleTrace, bool, Option<&'static str>) {
        let Some(e) = self.dessin2d.edition_contour.as_ref() else { return (CibleTrace::Aucune, false, None) };
        let cible = if e.dessin { CibleTrace::Aucune } else if e.sommet.is_some() { CibleTrace::Sommet } else if e.segment.is_some() { CibleTrace::Segment } else { CibleTrace::Aucune };
        let fixe = matches!(e.creation, Some(Usage::Terrain | Usage::Cadrage | Usage::Escalier | Usage::GardeCorps | Usage::Annotation));
        let sans_copie = if e.ligne() || matches!(e.creation, Some(Usage::Terrain | Usage::Cadrage | Usage::Escalier | Usage::GardeCorps)) {
            Some("Ce tracé n'accepte qu'un seul contour")
        } else if e.creation != Some(Usage::Annotation) {
            // dalle, toiture, terre-plein, limite : un trait copié resterait ouvert
            Some("Une surface n'a que des contours fermés : tracer un nouveau contour avec Polygone ou Rectangle")
        } else {
            None
        };
        (cible, fixe, sans_copie)
    }

    /// Outil du ruban Modifier dont la commande est en cours sur le tracé.
    pub(in crate::ui) fn outil_commande_trace(&self) -> Option<crate::ui::ruban::Outil> {
        use crate::ui::ruban::Outil;
        use CommandeContour as C;
        Some(match self.dessin2d.edition_contour.as_ref().filter(|e| !e.dessin)?.commande? {
            C::Deplacer => Outil::Deplacer,
            C::Copier => Outil::Copier,
            C::Reseau(_) => Outil::Reseau,
            C::Tourner => Outil::Tourner,
            C::Symetrie => Outil::Symetrie,
            C::Echelle => Outil::Echelle,
            C::Decaler => Outil::Decaler,
            C::Scinder => Outil::Scinder,
            C::Aligner => Outil::Aligner,
            C::Ajuster => Outil::Ajuster,
        })
    }

    /// Segment du tracé sous le curseur, autre que le trait sélectionné : référence
    /// d'Aligner et d'Ajuster.
    pub(super) fn segment_reference(&self, rect: egui::Rect, q: egui::Pos2) -> Option<[DVec2; 2]> {
        let e = self.dessin2d.edition_contour.as_ref()?;
        let mut choix: Option<(f32, [DVec2; 2])> = None;
        for (c, points) in self.dessin2d.dalle.iter().enumerate() {
            for i in 0..points.len().saturating_sub(usize::from(!e.fermee(c))) {
                if e.segment == Some((c, i)) { continue; }
                let (a, b) = (DVec2::from_array(points[i]), DVec2::from_array(points[(i + 1) % points.len()]));
                let (Some(ea), Some(eb)) = (self.vers_ecran(rect, a), self.vers_ecran(rect, b)) else { continue };
                let d = eb - ea;
                let t = if d.length_sq() > 0.0 { ((q - ea).dot(d) / d.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
                let distance = q.distance(ea + d * t);
                if distance < 10.0 && choix.is_none_or(|x| distance < x.0) { choix = Some((distance, [a, b])); }
            }
        }
        choix.map(|x| x.1)
    }

    /// Repère et point retenus pour la commande. Tourner et Échelle prennent l'angle (en
    /// degrés) ou le facteur tapé dès que le centre est donné, sans point de référence.
    fn points_commande(&self, commande: CommandeContour, p: DVec2) -> (Option<DVec2>, DVec2) {
        use CommandeContour as C;
        let e = self.dessin2d.edition_contour.as_ref();
        let repere = e.and_then(|e| e.repere);
        if !matches!(commande, C::Tourner | C::Echelle) { return (repere, self.point_commande(commande, p)); }
        let valeur = self.dessin2d.saisie.trim_start_matches(['<', '>']).replace(',', ".").parse::<f64>().ok().filter(|v| v.is_finite());
        if let (Some(centre), Some(valeur)) = (e.and_then(|e| e.depart), valeur) {
            let r = repere.unwrap_or(centre + DVec2::X * 1000.0);
            if commande == C::Tourner { return (Some(r), centre + DVec2::from_angle(valeur.to_radians()).rotate(r - centre)); }
            if valeur > 0.0 { return (Some(r), centre + (r - centre) * valeur); }
        }
        (repere, p)
    }

    /// Point retenu pour la commande : Décaler prend la distance tapée, du côté du curseur.
    fn point_commande(&self, commande: CommandeContour, p: DVec2) -> DVec2 {
        if commande != CommandeContour::Decaler { return p; }
        let Some(distance) = super::saisie::distance_saisie(&self.dessin2d.saisie, crate::config::options::unite().facteur()) else { return p };
        let Some((c, i)) = self.dessin2d.edition_contour.as_ref().and_then(|e| e.segment) else { return p };
        let Some(points) = self.dessin2d.dalle.get(c).filter(|x| i < x.len()) else { return p };
        let (a, b) = (DVec2::from_array(points[i]), DVec2::from_array(points[(i + 1) % points.len()]));
        let Some(n) = (b - a).try_normalize().map(|d| d.perp()) else { return p };
        a + n * distance.copysign((p - a).dot(n))
    }

    /// Clic pendant une commande du ruban Modifier sur le tracé : point de départ, de
    /// repère, puis application.
    pub(super) fn clic_commande_contour(&mut self, rect: egui::Rect, commande: CommandeContour, p: DVec2, curseur: Option<egui::Pos2>) {
        use CommandeContour as C;
        let (repere, p) = self.points_commande(commande, p);
        let reference = if matches!(commande, C::Aligner | C::Ajuster) { curseur.and_then(|q| self.segment_reference(rect, q)) } else { None };
        let e = self.dessin2d.edition_contour.as_mut().unwrap();
        // points préalables : départ (ou centre), puis repère d'une rotation ou d'une échelle
        if matches!(commande, C::Deplacer | C::Copier | C::Reseau(_) | C::Symetrie | C::Tourner | C::Echelle) && e.depart.is_none() {
            e.depart = Some(p);
            self.dessin2d.saisie.clear();
            return;
        }
        if matches!(commande, C::Tourner | C::Echelle) && e.repere.is_none() {
            // une valeur tapée s'applique sans point de référence
            let Some(r) = repere else {
                e.repere = Some(p);
                self.dessin2d.saisie.clear();
                return;
            };
            e.repere = Some(r);
        }
        match transformer_contours(&self.dessin2d.dalle, commande, e.segment, e.sommet, e.depart, e.repere, p, reference) {
            Ok(Some(nouveaux)) => {
                let ajoutes = nouveaux.len().saturating_sub(self.dessin2d.dalle.len());
                e.fermees.extend(std::iter::repeat_n(false, ajoutes));
                if commande == C::Scinder && let Some((c, i)) = e.segment {
                    // le nouveau trait garde le type de trait de celui qu'il prolonge
                    if c == 0 && let Some(AnnotationForme::Polyligne { traits, .. }) = e.annotation.as_mut().map(|a| &mut a.forme) && !traits.is_empty() {
                        let type_trait = traits.get(i).cloned().unwrap_or_default();
                        traits.insert((i + 1).min(traits.len()), type_trait);
                    }
                    e.segment = None;
                }
                (e.commande, e.depart, e.repere) = (None, None, None);
                self.dessin2d.dalle = nouveaux;
                self.dessin2d.saisie.clear();
                self.dessin2d.message = None;
            }
            Ok(None) => {}
            Err(m) => self.dessin2d.message = Some(m),
        }
    }

    /// Aperçu de la commande du ruban Modifier en cours sur le tracé : résultat sous le
    /// curseur en tirets, lignes de construction, référence visée et valeur (distance,
    /// angle, facteur).
    pub(super) fn apercu_commande_contour(&self, painter: &egui::Painter, rect: egui::Rect) {
        use CommandeContour as C;
        let Some(e) = self.dessin2d.edition_contour.as_ref().filter(|e| !e.dessin) else { return };
        let Some(commande) = e.commande else { return };
        let Some(p) = self.dessin2d.point.map(|p| p.0).or(self.dessin2d.curseur) else { return };
        let Some(ep) = self.vers_ecran(rect, p) else { return };
        let (repere, p) = self.points_commande(commande, p);
        let orange = Color32::from_rgb(230, 110, 20);
        let vert = Color32::from_rgb(20, 160, 60);
        let tirets = |pts: &[egui::Pos2], couleur: Color32| { painter.add(egui::Shape::dashed_line(pts, egui::Stroke::new(1.5, couleur), 6.0, 4.0)); };
        let u = crate::config::options::unite();
        let longueur = |l: f64| format!("{}", u.afficher(l.round()));
        let mut valeur: Option<String> = None;
        // lignes de construction depuis le départ (ou le centre)
        if let Some(d) = e.depart && let Some(ed) = self.vers_ecran(rect, d) {
            painter.circle_stroke(ed, 4.0, egui::Stroke::new(1.5, vert));
            match commande {
                C::Symetrie => {
                    // axe de symétrie, prolongé
                    if (ep - ed).length_sq() > 1.0 { let dir = (ep - ed).normalized(); tirets(&[ed - dir * 4000.0, ed + dir * 4000.0], vert); }
                }
                C::Tourner | C::Echelle => {
                    if let Some(er) = repere.and_then(|r| self.vers_ecran(rect, r)) { painter.line_segment([ed, er], egui::Stroke::new(1.0, vert)); }
                    tirets(&[ed, ep], vert);
                    if let Some(r) = repere {
                        let (i, f) = (r - d, p - d);
                        if i.length_squared() > 1e-12 && f.length_squared() > 1e-12 {
                            valeur = Some(if commande == C::Tourner { format!("{:.1}°", i.perp_dot(f).atan2(i.dot(f)).to_degrees()) } else { format!("× {:.3}", f.length() / i.length()) });
                        }
                    }
                }
                _ => {
                    tirets(&[ed, ep], vert);
                    valeur = Some(longueur(p.distance(d)));
                }
            }
        }
        let reference = if matches!(commande, C::Aligner | C::Ajuster) { self.segment_reference(rect, ep) } else { None };
        if let Some([a, b]) = reference && let (Some(a), Some(b)) = (self.vers_ecran(rect, a), self.vers_ecran(rect, b)) {
            painter.line_segment([a, b], egui::Stroke::new(3.0, vert));
        }
        if commande == C::Decaler && let Some((c, i)) = e.segment && let Some(points) = self.dessin2d.dalle.get(c) && i < points.len() {
            let (a, b) = (DVec2::from_array(points[i]), DVec2::from_array(points[(i + 1) % points.len()]));
            if let Some(n) = (b - a).try_normalize().map(|d| d.perp()) { valeur = Some(longueur((p - a).dot(n).abs())); }
        }
        // résultat : les contours modifiés ou ajoutés
        if let Ok(Some(nouveaux)) = transformer_contours(&self.dessin2d.dalle, commande, e.segment, e.sommet, e.depart, repere, p, reference) {
            for (i, contour) in nouveaux.iter().enumerate() {
                if self.dessin2d.dalle.get(i) == Some(contour) { continue; }
                let mut pts: Vec<egui::Pos2> = contour.iter().filter_map(|q| self.vers_ecran(rect, DVec2::from_array(*q))).collect();
                if i < self.dessin2d.dalle.len() && e.fermee(i) && let Some(q) = pts.first().copied() { pts.push(q); }
                tirets(&pts, orange);
                for q in &pts { painter.circle_filled(*q, 2.5, orange); }
            }
            if commande == C::Scinder { painter.circle_filled(ep, 5.0, orange); }
        }
        if let Some(texte) = valeur {
            let texte = painter.layout_no_wrap(texte, egui::FontId::proportional(12.0), Color32::WHITE);
            let cadre = egui::Rect::from_min_size(ep + egui::vec2(16.0, -22.0), texte.size() + egui::vec2(10.0, 4.0));
            painter.rect_filled(cadre, 3.0, Color32::from_rgba_unmultiplied(40, 40, 40, 220));
            painter.galley(cadre.min + egui::vec2(5.0, 2.0), texte, Color32::WHITE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use CommandeContour as C;

    fn carre() -> Vec<Vec<[f64; 2]>> {
        vec![vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]]
    }

    #[test]
    fn deplacer_attend_le_depart_puis_translate_le_trait() {
        let c = carre();
        assert_eq!(transformer_contours(&c, C::Deplacer, Some((0, 0)), None, None, None, DVec2::ZERO, None), Ok(None));
        let r = transformer_contours(&c, C::Deplacer, Some((0, 0)), None, Some(DVec2::ZERO), None, DVec2::new(0.0, -5.0), None).unwrap().unwrap();
        assert_eq!(r[0], vec![[0.0, -5.0], [10.0, -5.0], [10.0, 10.0], [0.0, 10.0]]);
        // dernier trait d'un contour fermé : il rejoint le premier sommet
        let r = transformer_contours(&c, C::Deplacer, Some((0, 3)), None, Some(DVec2::ZERO), None, DVec2::new(-2.0, 0.0), None).unwrap().unwrap();
        assert_eq!(r[0], vec![[-2.0, 0.0], [10.0, 0.0], [10.0, 10.0], [-2.0, 10.0]]);
    }

    #[test]
    fn decaler_scinder_et_copier() {
        let c = carre();
        let r = transformer_contours(&c, C::Decaler, Some((0, 0)), None, None, None, DVec2::new(3.0, -4.0), None).unwrap().unwrap();
        assert_eq!(r[0][..2], [[0.0, -4.0], [10.0, -4.0]]);
        let r = transformer_contours(&c, C::Scinder, Some((0, 0)), None, None, None, DVec2::new(4.0, 1.0), None).unwrap().unwrap();
        assert_eq!(r[0].len(), 5);
        assert_eq!(r[0][1], [4.0, 0.0]);
        assert!(transformer_contours(&c, C::Scinder, Some((0, 0)), None, None, None, DVec2::new(40.0, 1.0), None).is_err());
        let r = transformer_contours(&c, C::Copier, Some((0, 0)), None, Some(DVec2::ZERO), None, DVec2::new(0.0, 20.0), None).unwrap().unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[1], vec![[0.0, 20.0], [10.0, 20.0]]);
        assert!(transformer_contours(&c, C::Copier, None, Some((0, 0)), Some(DVec2::ZERO), None, DVec2::ONE, None).is_err());
    }

    #[test]
    fn tourner_un_sommet_et_ajuster_sur_une_reference() {
        let c = carre();
        let r = transformer_contours(&c, C::Tourner, None, Some((0, 1)), Some(DVec2::ZERO), Some(DVec2::X), DVec2::Y, None).unwrap().unwrap();
        assert!((DVec2::from_array(r[0][1]) - DVec2::new(0.0, 10.0)).length() < 1e-9);
        // trait du bas prolongé jusqu'à la droite x = 15
        let reference = Some([DVec2::new(15.0, -5.0), DVec2::new(15.0, 5.0)]);
        let r = transformer_contours(&c, C::Ajuster, Some((0, 0)), None, None, None, DVec2::ZERO, reference).unwrap().unwrap();
        assert_eq!(r[0][1], [15.0, 0.0]);
        // Aligner : une référence perpendiculaire au trait ou absente est refusée
        assert!(transformer_contours(&c, C::Aligner, Some((0, 0)), None, None, None, DVec2::ZERO, reference).is_err());
        assert!(transformer_contours(&c, C::Aligner, Some((0, 0)), None, None, None, DVec2::ZERO, None).is_err());
    }

    #[test]
    fn aligner_sur_une_reference_parallele_ou_oblique() {
        let c = carre();
        // parallèle : le trait du bas descend sur la droite y = -3
        let parallele = Some([DVec2::new(20.0, -3.0), DVec2::new(30.0, -3.0)]);
        let r = transformer_contours(&c, C::Aligner, Some((0, 0)), None, None, None, DVec2::ZERO, parallele).unwrap().unwrap();
        assert_eq!(r[0], vec![[0.0, -3.0], [10.0, -3.0], [10.0, 10.0], [0.0, 10.0]]);
        // oblique : ses deux extrémités viennent sur la droite y = x - 20
        let oblique = Some([DVec2::new(20.0, 0.0), DVec2::new(30.0, 10.0)]);
        let r = transformer_contours(&c, C::Aligner, Some((0, 0)), None, None, None, DVec2::ZERO, oblique).unwrap().unwrap();
        for k in 0..2 { assert!((r[0][k][1] - (r[0][k][0] - 20.0)).abs() < 1e-9); }
        assert!((DVec2::from_array(r[0][0]) - DVec2::new(10.0, -10.0)).length() < 1e-9);
        assert_eq!(r[0][2..], [[10.0, 10.0], [0.0, 10.0]]);
    }

    #[test]
    fn reseau_ajoute_le_nombre_de_copies_demande() {
        let c = carre();
        let r = transformer_contours(&c, C::Reseau(5), Some((0, 0)), None, Some(DVec2::ZERO), None, DVec2::new(0.0, -2.0), None).unwrap().unwrap();
        assert_eq!(r.len(), 6);
        assert_eq!(r[5], vec![[0.0, -10.0], [10.0, -10.0]]);
    }
}
