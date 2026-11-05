//! Options de tracé et niveau de tracé.
use super::*;

impl ViewerApp {
    /// Barre d'options du tracé, en haut de la vue : forme du mur (et chaînage) ou de
    /// l'élément horizontal.
    pub(in crate::ui) fn reinitialiser_forme_cadrage(&mut self) {
        self.dessin2d.forme_cadrage=Some(OutilDessin::Rectangle);
        self.dessin2d.trace=None;
        self.dessin2d.dalle.clear();self.dessin2d.ouverts.clear();
    }

    pub(in crate::ui) fn options_trace(&mut self, ui: &mut egui::Ui) {
        match self.usage_dessin().map(|(u, _)| u) {
            Some(Usage::Mur) => self.options_trace_mur(ui),
            Some(Usage::Escalier | Usage::GardeCorps) => {}
            Some(Usage::Cadrage) => { ui.horizontal(|ui| {
                let forme=self.dessin2d.forme_cadrage.unwrap_or(OutilDessin::Rectangle);
                for (valeur,nom) in [(OutilDessin::Rectangle,"Rectangle"),(OutilDessin::Polyligne,"Ligne")] {
                    if ui.selectable_label(forme==valeur,nom).clicked() {self.dessin2d.forme_cadrage=Some(valeur);self.dessin2d.trace=None;self.dessin2d.dalle.clear();self.dessin2d.ouverts.clear();}
                }
            }); }
            Some(Usage::Terrain) => {
                // type (épaisseur) ; cotes NGF des sommets et des points : à côté de chacun
                // dans la vue
                let types = self.natif.as_ref().map(|(p, _)| p.types_terrains_disponibles()).unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label("Type");
                    let actuel = self.dessin2d.terrain_type.as_ref().and_then(|id| types.iter().find(|t| &t.id == id)).or(types.first());
                    let libelle = |t: &crate::modele::elements::site::TypeTerrain| format!("{} · {}", t.nom, crate::config::options::unite().afficher(t.epaisseur));
                    egui::ComboBox::from_id_salt("type_terrain_trace").selected_text(actuel.map(libelle).unwrap_or_default()).show_ui(ui, |ui| {
                        for t in &types {
                            if ui.selectable_label(actuel.is_some_and(|a| a.id == t.id), libelle(t)).clicked() {
                                // le type par défaut n'est pas enregistré tant que le projet n'en a aucun
                                self.dessin2d.terrain_type = self.natif.as_ref().is_some_and(|(p, _)| !p.types_terrains.is_empty()).then(|| t.id.clone());
                            }
                        }
                    });
                    if !self.dessin2d.dalle.is_empty() {
                        ui.separator();
                        ui.checkbox(&mut self.dessin2d.terrain_ajouter_point, "Placer des points cotés");
                        ui.label("Cote NGF");
                        ui.add(egui::DragValue::new(&mut self.dessin2d.terrain_altitude).speed(10.0).dimensions()).on_hover_text("Cote NGF des prochains points cotés");
                    }
                });
            }
            Some(Usage::Surface | Usage::LimitePropriete) => {
                ui.horizontal(|ui| {
                    ui.label("Forme");
                    let actuelle = self.usage_dessin().map(|(_, o)| o);
                    for (forme, nom, aide) in [
                        (OutilDessin::Polyligne, "Polygone", "Cliquer les sommets ; recliquer le premier, F ou clic droit pour fermer. Entrée valide les contours tracés."),
                        (OutilDessin::Rectangle, "Rectangle", "Deux coins opposés, dans les axes du plan"),
                        (OutilDessin::Cercle, "Cercle", "Centre puis rayon (contour à 48 côtés)"),
                    ] {
                        if ui.selectable_label(actuelle == Some(forme), nom).on_hover_text(aide).clicked() {
                            self.ruban_edition.forme_dalle = forme;
                        }
                    }
                });
            }
            _ => {}
        }
    }

    /// Le trait le plus proche : conserve les arcs/cercle et choisit un segment d'une polyligne.
    pub(in crate::ui) fn trait_mur_vise(&self,rect:egui::Rect,q:egui::Pos2)->Option<d2::Forme2D> {
        if !self.vue_en_plan() || !self.point_dans_cadrage(rect,q) {return None;}
        let (p,_)=self.natif.as_ref()?;
        let vue=self.vue_native_id()?;
        let v=p.vues.iter().find(|v|v.id==vue)?;
        let (o,u,h)=crate::modele::documentation::repere(p,v);
        let point=|a:DVec2| (o+u*a.x+h*a.y).truncate();
        let mut choix:Option<(f32,d2::Forme2D)>=None;
        let mut essayer=|forme:d2::Forme2D| {
            for ligne in forme.polylignes() {
                for w in ligne.windows(2) {
                    let (Some(a),Some(b))=(self.vers_ecran(rect,w[0]),self.vers_ecran(rect,w[1])) else {continue};
                    let ab=b-a;let t=if ab.length_sq()>0. {((q-a).dot(ab)/ab.length_sq()).clamp(0.,1.)}else{0.};
                    let distance=q.distance(a+ab*t);
                    if distance<=10. && choix.as_ref().is_none_or(|c|distance<c.0) {choix=Some((distance,forme.clone()));}
                }
            }
        };
        for a in p.annotations.iter().filter(|a|a.vue==vue && !v.elements_masques.contains(&a.id)) {
            let Some(f)=a.forme.forme_2d() else {continue};
            match f {
                d2::Forme2D::Ligne([a,b])=>essayer(d2::Forme2D::Ligne([point(a),point(b)])),
                d2::Forme2D::Arc(pts)=>essayer(d2::Forme2D::Arc(pts.map(point))),
                d2::Forme2D::Cercle{centre,rayon}=>essayer(d2::Forme2D::Cercle{centre:point(centre),rayon}),
                _=>for l in f.polylignes() {for w in l.windows(2){essayer(d2::Forme2D::Ligne([point(w[0]),point(w[1])]))}},
            }
        }
        choix.map(|c|c.1).or_else(||self.trait_annotation_vise(q,rect).map(|[a,b]| {
            let origine=self.scene.as_ref().map_or(DVec3::ZERO,|s|s.origin);
            d2::Forme2D::Ligne([(a.as_dvec3()+origine).truncate()*1000.,(b.as_dvec3()+origine).truncate()*1000.])
        }))
    }

    /// Barre d'options de l'outil Mur, en haut de la vue : forme tracée et chaînage.
    pub(in crate::ui) fn options_trace_mur(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.selectable_label(self.ruban_edition.selection_mur,"Sélection").on_hover_text("Sélectionner une ligne existante dans la vue pour tracer le mur").clicked() {
                self.ruban_edition.selection_mur=true;
                self.terminer_edition_axe();
            }
            for (forme, nom, aide) in [
                (OutilDessin::Ligne, "Ligne", "Murs droits, point par point"),
                (OutilDessin::Rectangle, "Rectangle", "Quatre murs par deux coins opposés"),
                (OutilDessin::Arc, "Arc", "Mur courbe : départ, point de passage, fin"),
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
        if let Some(crate::modele::GenreVue::Plan { niveau, .. }) = vue.map(|v| &v.genre) {
            return Some(niveau.clone());
        }
        p.niveaux.iter().min_by(|a, b| a.altitude.total_cmp(&b.altitude)).map(|n| n.id.clone())
    }

}
