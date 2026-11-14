//! Cotation « à la Revit » des vues en plan : la cote s'appuie sur des arêtes de
//! référence des éléments (faces et axes de murs, côtés de dalles, tableaux
//! d'ouvertures…) et les suit. Élément sélectionné : la valeur des cotes qui s'appuient
//! sur lui se saisit dans la vue et le déplace (longueur) ou le tourne (angle).
use super::*;
use crate::modele::documentation::{self as doc, GenreCote, PointCote, RefArete};
use crate::ui::ruban::Outil;

/// Arête de référence sous le curseur : élément, rang, segment (repère de la vue) et
/// abscisse relative du point visé.
pub(super) type AreteVisee = (String, usize, [DVec2; 2], f64);

impl ViewerApp {
    /// Vue en plan native ouverte et son projet.
    fn plan_natif(&self) -> Option<(&crate::modele::Projet, &crate::modele::Vue)> {
        let id = self.vue_native_id()?;
        let p = &self.natif.as_ref()?.0;
        let v = p.vues.iter().find(|v| v.id == id)?;
        matches!(v.genre, crate::modele::GenreVue::Plan { .. }).then_some((p, v))
    }

    /// Outil de cote en sélection d'arêtes : longueur ou angle, vue en plan, case
    /// « Sélectionner des points » décochée.
    pub(in crate::ui) fn cotation_par_aretes(&self) -> Option<GenreCote> {
        let genre = self.outil_cotation().filter(|g| matches!(g, GenreCote::Lineaire | GenreCote::Angulaire))?;
        let outil = if genre == GenreCote::Lineaire { Outil::Cote } else { Outil::CoteAngulaire };
        (!self.ruban_edition.case_option(outil) && self.plan_natif().is_some()).then_some(genre)
    }

    /// Consigne de l'étape en cours en sélection d'arêtes.
    pub(in crate::ui) fn invite_aretes(&self) -> Option<&'static str> {
        let genre = self.cotation_par_aretes()?;
        Some(match (self.dessin2d.aretes_cote.len(), genre) {
            (0, _) => "Première arête à coter (face ou axe de mur, côté de dalle, tableau…)",
            (1, _) => "Seconde arête",
            (_, GenreCote::Angulaire) => "Position de l'arc de cote",
            _ => "Position de la ligne de cote",
        })
    }

    /// Arêtes de référence de la vue (éléments du niveau, quadrillages, traits de la vue).
    fn aretes_vue(&mut self) -> Vec<(String, usize, [DVec2; 2])> {
        let Some((p, v)) = self.plan_natif() else { return Vec::new() };
        if let Some((r, vue, a)) = &self.dessin2d.aretes_cache && *r == self.bim.revision && vue == &v.id {
            return a.clone();
        }
        let crate::modele::GenreVue::Plan { niveau, .. } = &v.genre else { return Vec::new() };
        let en_assemblage = |m: &crate::modele::Mur| m.assemblage.as_ref().is_some_and(|a| p.assemblage(a).is_some());
        let murs: Vec<&str> = p.murs.iter().filter(|m| &m.niveau == niveau).map(|m| if en_assemblage(m) { m.assemblage.as_deref().unwrap() } else { m.id.as_str() }).collect();
        let mut ids: Vec<&str> = Vec::new();
        for id in &murs { if !ids.contains(id) { ids.push(id); } }
        ids.extend(p.dalles.iter().filter(|d| &d.niveau == niveau).map(|d| d.id.as_str()));
        ids.extend(p.garde_corps.iter().filter(|g| &g.niveau == niveau).map(|g| g.id.as_str()));
        ids.extend(p.ouvertures.iter().filter(|o| murs.contains(&o.mur.as_str()) || p.murs.iter().any(|m| m.id == o.mur && &m.niveau == niveau)).map(|o| o.id.as_str()));
        ids.extend(p.limites_propriete.iter().map(|l| l.id.as_str()));
        ids.extend(p.quadrillages.iter().map(|q| q.id.as_str()));
        ids.extend(p.annotations.iter().filter(|a| a.vue == v.id && matches!(a.forme, AnnotationForme::Ligne { .. } | AnnotationForme::Polyligne { .. })).map(|a| a.id.as_str()));
        let aretes: Vec<(String, usize, [DVec2; 2])> = ids.into_iter()
            .flat_map(|id| doc::aretes_reference(p, id).into_iter().enumerate().map(move |(k, s)| (id.to_string(), k, s)))
            .collect();
        self.dessin2d.aretes_cache = Some((self.bim.revision, v.id.clone(), aretes.clone()));
        aretes
    }

    /// Arête la plus proche du curseur, à moins du rayon d'accrochage à l'écran.
    fn arete_sous_curseur(&mut self, rect: egui::Rect, curseur: DVec2) -> Option<AreteVisee> {
        let ecran = self.vers_ecran(rect, curseur)?;
        let mut choix: Option<(f32, AreteVisee)> = None;
        for (id, k, [a, b]) in self.aretes_vue() {
            let d = b - a;
            let l2 = d.length_squared();
            if l2 < 1e-9 { continue; }
            let t = ((curseur - a).dot(d) / l2).clamp(0.0, 1.0);
            let Some(q) = self.vers_ecran(rect, a + d * t) else { continue };
            let distance = q.distance(ecran);
            if distance < crate::ui::accrochage::RAYON && choix.as_ref().is_none_or(|(x, _)| distance < *x) {
                choix = Some((distance, (id, k, [a, b], t)));
            }
        }
        choix.map(|c| c.1)
    }

    /// Survol en sélection d'arêtes : arête visée mise en évidence, point du tracé posé
    /// sur elle (aperçu de la cote). À appeler après le calcul du point accroché.
    pub(super) fn survol_aretes_cote(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        self.dessin2d.arete_survol = None;
        if self.cotation_par_aretes().is_none() {
            self.dessin2d.aretes_cote.clear();
            return;
        }
        // tracé abandonné (Échap, clic droit) : références oubliées
        if self.dessin2d.trace.as_ref().is_none_or(|t| t.points.len() < self.dessin2d.aretes_cote.len()) {
            self.dessin2d.aretes_cote.clear();
        }
        if self.dessin2d.aretes_cote.len() >= 2 { return; }
        let Some(curseur) = self.dessin2d.curseur else { return };
        let Some(visee) = self.arete_sous_curseur(rect, curseur) else {
            // hors arête : pas de point, le clic ne pose rien
            self.dessin2d.point = None;
            return;
        };
        let [a, b] = visee.2;
        let mut point = a + (b - a) * visee.3;
        // seconde arête parallèle à la première : pied de la perpendiculaire, la cote
        // mesure l'écart entre les deux
        if let (Some(GenreCote::Lineaire), Some(premier)) = (self.cotation_par_aretes(), self.dessin2d.trace.as_ref().and_then(|t| t.points.first().copied()).filter(|_| self.dessin2d.aretes_cote.len() == 1)) {
            let u = (b - a).normalize_or_zero();
            let (_, _, premiere, _) = self.dessin2d.arete_premiere.clone().unwrap_or_else(|| visee.clone());
            if (premiere[1] - premiere[0]).normalize_or_zero().perp_dot(u).abs() < 1e-3 {
                point = a + u * (premier - a).dot(u);
            }
        }
        self.dessin2d.point = Some((point, Accroche::Perpendiculaire));
        if let (Some(x), Some(y)) = (self.vers_ecran(rect, a), self.vers_ecran(rect, b)) {
            ui.painter_at(rect).line_segment([x, y], egui::Stroke::new(3.0, Color32::from_rgb(0, 120, 215)));
        }
        self.dessin2d.arete_survol = Some(visee);
    }

    /// Clic en sélection d'arêtes. Vrai : le clic est traité ici (arête retenue ou clic
    /// dans le vide ignoré) ; faux : la position de la cote suit le chemin habituel.
    pub(super) fn clic_arete_cote(&mut self) -> bool {
        let Some(genre) = self.cotation_par_aretes() else { return false };
        if self.dessin2d.aretes_cote.len() >= 2 { return false; }
        let (Some(visee), Some((point, _))) = (self.dessin2d.arete_survol.clone(), self.dessin2d.point) else {
            self.dessin2d.message = Some("Cliquez une arête, ou cochez « Sélectionner des points » dans le bandeau.".into());
            return true;
        };
        self.dessin2d.message = None;
        let premiere = self.dessin2d.arete_premiere.clone().filter(|_| self.dessin2d.aretes_cote.len() == 1);
        if premiere.as_ref().is_some_and(|p| p.0 == visee.0 && p.1 == visee.1) {
            self.dessin2d.message = Some("Choisissez une autre arête.".into());
            return true;
        }
        let Some(t) = self.dessin2d.trace.as_mut() else { return true };
        match (genre, premiere) {
            (GenreCote::Angulaire, Some(premiere)) => {
                // sommet au croisement ; branches du côté des points cliqués
                let (u, w) = ((premiere.2[1] - premiere.2[0]).normalize_or_zero(), (visee.2[1] - visee.2[0]).normalize_or_zero());
                let den = u.perp_dot(w);
                if den.abs() < 1e-6 {
                    self.dessin2d.message = Some("Arêtes parallèles : pas d'angle à coter.".into());
                    return true;
                }
                let s = premiere.2[0] + u * ((visee.2[0] - premiere.2[0]).perp_dot(w) / den);
                let clic1 = t.points.first().copied().unwrap_or(s + u);
                let branche = |dir: DVec2, clic: DVec2| { let r = (clic - s).dot(dir); s + dir * if r.abs() < 1.0 { 1000.0 } else { r } };
                t.points = vec![s, branche(u, clic1), branche(w, point)];
            }
            _ => t.points.push(point),
        }
        if self.dessin2d.aretes_cote.is_empty() { self.dessin2d.arete_premiere = Some(visee.clone()); }
        self.dessin2d.aretes_cote.push((visee.0, visee.1, visee.3));
        true
    }

    /// Références d'arêtes à porter par la cote qui vient d'être construite depuis les
    /// points ; None en sélection de points.
    pub(super) fn lier_aretes(&mut self, forme: AnnotationForme) -> AnnotationForme {
        let refs = std::mem::take(&mut self.dessin2d.aretes_cote);
        self.dessin2d.arete_premiere = None;
        let [(id1, k1, t1), (id2, k2, t2)] = refs.as_slice() else { return forme };
        match forme {
            AnnotationForme::Cote { decalage, .. } => AnnotationForme::Cote {
                debut: PointCote::Arete { id: id1.clone(), arete: *k1, t: *t1 },
                fin: PointCote::Arete { id: id2.clone(), arete: *k2, t: *t2 },
                decalage,
            },
            AnnotationForme::CoteAngulaire { sommet, debut, fin, rayon, .. } => AnnotationForme::CoteAngulaire {
                sommet, debut, fin, rayon,
                aretes: Some([RefArete { id: id1.clone(), arete: *k1 }, RefArete { id: id2.clone(), arete: *k2 }]),
            },
            autre => autre,
        }
    }

    /// Élément sélectionné dans une vue en plan : la valeur des cotes qui s'appuient sur
    /// lui d'un seul côté devient un champ ; la saisie déplace ou tourne l'élément.
    pub(in crate::ui) fn cotes_pilotables(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        if self.ruban_edition.a_un_outil() || self.dessin2d.edition_contour.is_some() { return; }
        let ids = self.selection_bim();
        let [element] = ids.as_slice() else { return };
        let Some((p, v)) = self.plan_natif() else { return };
        let (o, u, h) = doc::repere(p, v);
        let unite = crate::config::options::unite();
        let longueur = |x: f64| unite.afficher(x);
        let altitude = |q: [f64; 2], relatif: bool| doc::altitude_vue(p, v, q, relatif);
        // (cote, position du texte à l'écran, valeur, angle ?)
        let mut champs: Vec<(String, egui::Pos2, f64, bool)> = Vec::new();
        for a in p.annotations.iter().filter(|a| a.vue == v.id) {
            let Some(valeur) = doc::valeur_pilotable(p, v, &a.forme, element) else { continue };
            let taille = a.hauteur * v.echelle;
            let style = doc::StyleCote { oblique: taille * 0.6, taille, longueur: &longueur, altitude: &altitude };
            let (geometrie, angle) = match &a.forme {
                AnnotationForme::Cote { debut, fin, .. } => (doc::geometrie(&a.forme, doc::extremites_cote(p, v, debut, fin), &style), false),
                forme => (doc::angle_resolu(p, v, forme).and_then(|f| doc::geometrie(&f, None, &style)), true),
            };
            let Some((centre, ..)) = geometrie.and_then(|(_, textes)| textes.into_iter().next()) else { continue };
            if let Some(q) = self.ecran_3d(rect, o + u * centre.x + h * centre.y).filter(|q| rect.contains(*q)) {
                champs.push((a.id.clone(), q, valeur, angle));
            }
        }
        let element = element.clone();
        let mut saisie = None;
        for (id, q, valeur, angle) in champs {
            let mut v = valeur;
            egui::Area::new(egui::Id::unique(("cote_pilotable", &id)))
                .fixed_pos(q - egui::vec2(34.0, 12.0))
                .constrain_to(rect)
                .show(ui.ctx(), |ui| {
                    egui::Frame::NONE.fill(Color32::from_rgb(225, 238, 252)).stroke(egui::Stroke::new(1.0, Color32::from_rgb(0, 120, 215))).corner_radius(3.0).inner_margin(egui::Margin::symmetric(3, 1)).show(ui, |ui| {
                        // pas de glissé (vitesse nulle) : la valeur se tape, Entrée l'applique
                        let champ = egui::DragValue::new(&mut v).speed(0.0);
                        let r = ui.add(if angle { champ.max_decimals(2).suffix("°") } else { champ.dimensions() })
                            .on_hover_text(if angle { "Cliquer pour saisir l'angle : l'élément sélectionné tourne autour du sommet" } else { "Cliquer pour saisir la cote : l'élément sélectionné se déplace" });
                        if r.changed() && (v - valeur).abs() > 1e-6 { saisie = Some((id.clone(), v)); }
                    });
                });
        }
        if let Some((id, valeur)) = saisie {
            let Some((p, v)) = self.plan_natif() else { return };
            let commande = p.annotations.iter().find(|a| a.id == id).ok_or_else(|| "Cote introuvable.".to_string())
                .and_then(|a| crate::modele::edition::commandes::piloter_cote(p, v, &a.forme, &element, valeur));
            match commande.and_then(|c| self.executer_bim(c)) {
                Ok(_) => {
                    // l'élément reste sélectionné pour enchaîner les saisies
                    self.bim.selection = vec![element];
                    self.status = "Élément repositionné par la cote — Ctrl+Z pour annuler.".into();
                }
                Err(e) => self.error = Some(e),
            }
        }
    }
}
