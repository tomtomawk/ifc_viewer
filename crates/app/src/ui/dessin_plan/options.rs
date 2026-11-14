//! Options de tracé et niveau de tracé.
use super::*;

/// Une arête courbe reste un arc ; une courbe libre est découpée en segments.
fn formes_selection(geometrie: Geom) -> Vec<noyau::forme2d::Forme2D> {
    match geometrie {
        Geom::Segment([a,b]) if a.distance_squared(b) > 1e-12 => vec![noyau::forme2d::Forme2D::Ligne([a,b])],
        Geom::Arc { centre, rayon, depart, balayage } => {
            let point = |angle| centre + DVec2::from_angle(angle) * rayon;
            vec![noyau::forme2d::Forme2D::Arc([point(depart), point(depart + balayage * 0.5), point(depart + balayage)])]
        }
        Geom::Cercle { centre, rayon } => vec![noyau::forme2d::Forme2D::Cercle { centre, rayon }],
        Geom::Courbe(points) => points.windows(2).map(|w| noyau::forme2d::Forme2D::Ligne([w[0],w[1]])).collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests_selection {
    use super::*;

    #[test]
    fn selection_conserve_les_arcs_et_ignore_les_points_isoles() {
        assert!(formes_selection(Geom::Segment([DVec2::ZERO, DVec2::ZERO])).is_empty());
        let formes = formes_selection(Geom::Arc { centre: DVec2::new(2.0, 3.0), rayon: 5.0, depart: 0.0, balayage: std::f64::consts::FRAC_PI_2 });
        let [noyau::forme2d::Forme2D::Arc(points)] = formes.as_slice() else { panic!("arc attendu") };
        let (centre, rayon, _, angle) = noyau::forme2d::arc_3_points(points[0], points[1], points[2]).unwrap();
        assert!(centre.distance(DVec2::new(2.0, 3.0)) < 1e-9 && (rayon - 5.0).abs() < 1e-9 && (angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    }
}

/// Formes de tracé proposées pour un usage, dans l'ordre de la barre d'options (forme,
/// nom, aide). Le mur garde sa propre liste (`options_trace_mur`).
fn formes_usage(usage: Usage) -> &'static [(OutilDessin, &'static str, &'static str)] {
    const POLYGONE: (OutilDessin, &str, &str) = (OutilDessin::Polyligne, "Polygone", "Cliquer les sommets ; recliquer le premier, F ou clic droit pour fermer. Entrée valide les contours tracés.");
    const RECTANGLE: (OutilDessin, &str, &str) = (OutilDessin::Rectangle, "Rectangle", "Deux coins opposés, dans les axes du plan");
    const CERCLE: (OutilDessin, &str, &str) = (OutilDessin::Cercle, "Cercle", "Centre puis rayon (contour à 48 côtés)");
    const POLYLIGNE: (OutilDessin, &str, &str) = (OutilDessin::Polyligne, "Polyligne", "Cliquer les sommets ; Entrée ou clic droit pour terminer. Un seul segment fait une ligne.");
    match usage {
        Usage::Surface | Usage::LimitePropriete | Usage::Terrain => &[POLYGONE, RECTANGLE, CERCLE],
        Usage::Cadrage => &[POLYGONE, RECTANGLE],
        Usage::Escalier | Usage::GardeCorps | Usage::Annotation => &[POLYLIGNE],
        Usage::Mur => &[],
    }
}

/// Boutons de la barre de tracé, partagés par les vues de projet et de famille.
pub(in crate::ui) fn boutons_trace(ui: &mut egui::Ui, selection: bool, commande: bool, outil: OutilDessin, formes: &[(OutilDessin, &'static str, &'static str)], raison: Option<&str>) -> (bool, Option<OutilDessin>) {
    let selectionner = ui.add(egui::Button::selectable(selection && !commande, "Sélectionner"))
        .on_hover_text("Interrompre le tracé et sélectionner ses traits et sommets (Échap).")
        .clicked();
    ui.separator();
    let mut choix = None;
    for &(forme, nom, aide) in formes {
        if ui.add_enabled(raison.is_none(), egui::Button::selectable(!selection && outil == forme, nom)).on_hover_text(aide).on_disabled_hover_text(raison.unwrap_or("")).clicked() { choix = Some(forme); }
    }
    (selectionner, choix)
}

impl ViewerApp {
    pub(in crate::ui) fn reinitialiser_forme_cadrage(&mut self) {
        self.dessin2d.forme_cadrage=Some(OutilDessin::Rectangle);
        self.dessin2d.trace=None;
        self.dessin2d.vider_contours();
    }

    /// Barre d'options du tracé, en haut de la vue, identique à la création et à la
    /// modification : Sélectionner, puis les formes de l'usage (Suppr retire le trait
    /// sélectionné).
    /// Une action sans objet est grisée avec sa raison, jamais déplacée.
    pub(in crate::ui) fn options_trace(&mut self, ui: &mut egui::Ui) {
        let Some((usage, outil)) = self.usage_dessin() else { return };
        if usage == Usage::Mur {
            self.options_trace_mur(ui);
            return;
        }
        if usage == Usage::Annotation && (outil != OutilDessin::Polyligne || self.outil_cotation().is_some()) {
            return;
        }
        let edition = self.dessin2d.edition_contour.as_ref();
        let selection = edition.is_some_and(|e| !e.dessin);
        let commande = edition.is_some_and(|e| e.commande.is_some());
        let ligne = edition.is_some_and(|e| e.ligne());
        let contours = self.dessin2d.dalle.len();
        let mut copies = edition.and_then(|e| match e.commande { Some(CommandeContour::Reseau(n)) => Some(n), _ => None });
        // contour unique : tracé repris d'une polyligne ou d'un garde-corps, terrain déjà tracé
        let raison_forme = if ligne {
            Some("Double-cliquer une extrémité du tracé pour le prolonger")
        } else if usage == Usage::Terrain && contours > 0 {
            Some("Un seul contour est attendu : glisser ses sommets")
        } else {
            None
        };
        let (mut choisir_selection, mut forme_choisie) = (false, None);
        ui.horizontal(|ui| {
            (choisir_selection, forme_choisie) = boutons_trace(ui, selection, commande, outil, formes_usage(usage), raison_forme);
            if let Some(n) = copies.as_mut() {
                ui.separator();
                ui.label("Copies");
                ui.add(egui::DragValue::new(n).range(1..=50)).on_hover_text("Nombre de copies du réseau, espacées de l'écart donné");
            }
        });
        if let (Some(n), Some(e)) = (copies, self.dessin2d.edition_contour.as_mut()) && matches!(e.commande, Some(CommandeContour::Reseau(_))) {
            e.commande = Some(CommandeContour::Reseau(n));
        }
        if choisir_selection {
            self.passer_en_selection();
        }
        if let Some(forme) = forme_choisie {
            self.choisir_forme_trace(usage, outil, forme);
        }
    }

    /// Bouton Sélectionner : le tracé en cours s'arrête où il en est, ses traits et ses
    /// sommets se sélectionnent.
    pub(in crate::ui) fn passer_en_selection(&mut self) {
        if self.dessin2d.edition_contour.is_none() {
            self.selectionner_trace_en_creation();
            return;
        }
        self.terminer_raccord_ouvert();
        if let Some((points, k)) = self.dessin2d.trace.as_ref().filter(|t| t.outil == OutilDessin::Polyligne && t.points.len() >= 2).map(|t| (t.points.iter().map(|p| p.to_array()).collect(), t.courbures())) {
            self.dessin2d.pousser_contour(points, k);
            self.dessin2d.edition_contour.as_mut().unwrap().fermees.push(false);
        }
        if let Some(t) = self.dessin2d.trace.as_mut() {
            t.points.clear();
        }
        if let Some(e) = &mut self.dessin2d.edition_contour {
            (e.dessin, e.commande, e.depart, e.repere) = (false, None, None, None);
        }
    }

    /// Bouton d'une forme : le tracé reprend avec elle (nouveau contour).
    fn choisir_forme_trace(&mut self, usage: Usage, actuelle: OutilDessin, forme: OutilDessin) {
        if actuelle != forme {
            self.terminer_raccord_ouvert();
        }
        if usage == Usage::Cadrage {
            // un seul contour de cadrage : le choix d'une forme le recommence
            self.dessin2d.forme_cadrage = Some(forme);
            self.dessin2d.trace = None;
            self.dessin2d.vider_contours();
            self.dessin2d.edition_contour = None;
            return;
        }
        self.ruban_edition.forme_dalle = forme;
        if let Some(e) = &mut self.dessin2d.edition_contour {
            (e.dessin, e.commande, e.depart, e.repere) = (true, None, None, None);
            // le premier clic choisit l'extrémité à reprendre
            e.raccord = None;
        }
    }

    /// Le trait le plus proche : conserve les arcs/cercle et choisit un segment d'une polyligne.
    pub(in crate::ui) fn trait_mur_vise(&mut self,rect:egui::Rect,q:egui::Pos2)->Option<noyau::forme2d::Forme2D> {
        if !self.vue_en_plan() || !self.point_dans_cadrage(rect,q) {return None;}
        let geometries = self.geoms_dessin_avec_mode(true);
        let (p,_)=self.natif.as_ref()?;
        let mut choix:Option<(f32,noyau::forme2d::Forme2D)>=None;
        let mut essayer=|forme:noyau::forme2d::Forme2D| {
            for ligne in forme.polylignes() {
                for w in ligne.windows(2) {
                    let (Some(a),Some(b))=(self.vers_ecran(rect,w[0]),self.vers_ecran(rect,w[1])) else {continue};
                    let ab=b-a;let t=if ab.length_sq()>0. {((q-a).dot(ab)/ab.length_sq()).clamp(0.,1.)}else{0.};
                    let distance=q.distance(a+ab*t);
                    if distance<=10. && choix.as_ref().is_none_or(|c|distance<c.0) {choix=Some((distance,forme.clone()));}
                }
            }
        };
        if let Some(v) = self.vue_native_id().and_then(|id| p.vues.iter().find(|v| v.id == id)) {
            let (o,u,h)=modele::documentation::repere(p,v);
            let point=|a:DVec2| (o+u*a.x+h*a.y).truncate();
            for a in p.annotations.iter().filter(|a|a.vue==v.id && !v.elements_masques.contains(&a.id)) {
                let Some(f)=a.forme.forme_2d() else {continue};
                match f {
                    noyau::forme2d::Forme2D::Ligne([a,b])=>essayer(noyau::forme2d::Forme2D::Ligne([point(a),point(b)])),
                    noyau::forme2d::Forme2D::Arc(pts)=>essayer(noyau::forme2d::Forme2D::Arc(pts.map(point))),
                    noyau::forme2d::Forme2D::Cercle{centre,rayon}=>essayer(noyau::forme2d::Forme2D::Cercle{centre:point(centre),rayon}),
                    _=>for l in f.polylignes() {for w in l.windows(2){essayer(noyau::forme2d::Forme2D::Ligne([point(w[0]),point(w[1])]))}},
                }
            }
        }
        for geometrie in geometries {
            for forme in formes_selection(geometrie) { essayer(forme); }
        }
        choix.map(|c|c.1).or_else(||self.trait_annotation_vise(q,rect).map(|[a,b]| {
            let origine=self.scene.as_ref().map_or(DVec3::ZERO,|s|s.origin);
            noyau::forme2d::Forme2D::Ligne([(a.as_dvec3()+origine).truncate()*1000.,(b.as_dvec3()+origine).truncate()*1000.])
        }))
    }

    /// Barre d'options de l'outil Mur, en haut de la vue : forme tracée et chaînage.
    pub(in crate::ui) fn options_trace_mur(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.selectable_label(self.ruban_edition.selection_mur,"Suivre un trait").on_hover_text("Cliquer un trait existant dans la vue : le mur suit ce trait").clicked() {
                self.ruban_edition.selection_mur=true;
                self.terminer_edition_axe();
            }
            for (forme, nom, aide) in [
                (OutilDessin::Ligne, "Ligne", "Murs droits, point par point"),
                (OutilDessin::Rectangle, "Rectangle", "Quatre murs par deux coins opposés"),
                (OutilDessin::Arc, "Arc", "Mur courbe : début, fin, puis courbure"),
                (OutilDessin::Cercle, "Cercle", "Centre, rayon, début et fin du périmètre"),
            ] {
                if ui.selectable_label(!self.ruban_edition.selection_mur && self.ruban_edition.forme_mur == forme, nom).on_hover_text(aide).clicked() {
                    self.ruban_edition.selection_mur=false;
                    self.ruban_edition.forme_mur = forme;
                }
            }
        });
    }

    /// Niveau du tracé d'un escalier ou d'un garde-corps : celui du plan ouvert, sinon
    /// le plus bas.
    pub(in crate::ui) fn niveau_trace(&self) -> Option<String> {
        let (p, _) = self.natif.as_ref()?;
        let vue = self.vue_native_id().and_then(|id| p.vues.iter().find(|v| v.id == id));
        if let Some(modele::GenreVue::Plan { niveau, .. }) = vue.map(|v| &v.genre) {
            return Some(niveau.clone());
        }
        p.niveaux.iter().min_by(|a, b| a.altitude.total_cmp(&b.altitude)).map(|n| n.id.clone())
    }

}
