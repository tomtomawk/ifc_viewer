//! Outil Poutre : tracé par le dessin commun (polyligne enchaînée ou rectangle, bandeau
//! de dessin commun), comme les murs ; niveau de référence et décalage ; justification
//! horizontale (Ctrl pendant le tracé) et arase ; type choisi dans Propriétés ; aperçu
//! de l'emprise réelle, raccords compris.
use crate::*;
use glam::DVec2;
use modele::elements::poutre::{axe_reel, Arase, Justification, PosePoutre, TypePoutre};

/// Réglages des prochaines poutres.
pub(crate) struct ReglagesPoutre {
    pub type_poutre: Option<String>,
    pub justification: Justification,
    pub arase: Arase,
    /// Niveau de référence (None : niveau de la vue) et cote de l'arase au-dessus (mm).
    pub niveau: Option<String>,
    pub decalage: f64,
    /// Section sans type (mm).
    pub largeur: f64,
    pub hauteur: f64,
}

impl Default for ReglagesPoutre {
    fn default() -> Self {
        ReglagesPoutre { type_poutre: None, justification: Justification::Axe, arase: Arase::Superieure, niveau: None, decalage: 2800.0, largeur: 200.0, hauteur: 400.0 }
    }
}

impl ReglagesPoutre {
    fn barre(&self, niveau: String, a: DVec2, b: DVec2) -> modele::Barre {
        modele::Barre {
            id: String::new(),
            nom: "Poutre".into(),
            genre: modele::GenreBarre::Poutre,
            niveau,
            debut: [a.x, a.y, self.decalage],
            fin: [b.x, b.y, self.decalage],
            largeur: self.largeur,
            hauteur: self.hauteur,
            rotation: 0.0,
            pose: PosePoutre { type_poutre: self.type_poutre.clone(), justification: self.justification, arase: self.arase },
            origine: None,
        }
    }

    fn croquis(&self, ui: &mut egui::Ui, projet: &modele::Projet, niveau: Option<&modele::Niveau>) {
        use crate::outils::croquis::{cadre, BLEU};
        let barre = self.barre(String::new(), DVec2::ZERO, DVec2::X);
        let Ok((largeur, hauteur, contour)) = modele::elements::poutre::section(projet, &barre) else { return };
        // Section réelle par rapport au point (ligne tracée, cote d’arase).
        let centre_x = self.justification.decalage(largeur);
        let centre_y = self.arase.bas(hauteur) + hauteur / 2.0;
        let section: Vec<_> = contour.into_iter().map(|[x, y]| [x + centre_x, y + centre_y]).collect();
        cadre(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(90.0, 76.0), egui::Sense::hover());
                let echelle = (64.0 / largeur).min(54.0 / hauteur);
                let point = |[x, y]: [f64; 2]| rect.center() - egui::vec2(((x - centre_x) * echelle) as f32, ((y - centre_y) * echelle) as f32);
                let p = ui.painter();
                let repere = point([0.0, 0.0]);
                let axe = egui::Stroke::new(0.8, ui.visuals().weak_text_color());
                for ligne in [
                    [egui::pos2(rect.left() + 3.0, repere.y), egui::pos2(rect.right() - 3.0, repere.y)],
                    [egui::pos2(repere.x, rect.top() + 3.0), egui::pos2(repere.x, rect.bottom() - 3.0)],
                ] {
                    p.add(egui::Shape::dashed_line(&ligne, axe, 3.0, 3.0));
                }
                p.add(egui::Shape::closed_line(section.iter().copied().map(point).collect(), egui::Stroke::new(1.8, BLEU)));
                p.circle_filled(repere, 2.5, BLEU);
                ui.vertical(|ui| {
                    let unite = modele::unite::unite();
                    ui.label(RichText::new("Coupe de la poutre").strong());
                    ui.label(format!("{} × {}", unite.afficher(largeur), unite.afficher(hauteur)));
                    ui.label(self.arase.libelle());
                    if let Some(niveau) = niveau {
                        let signe = if self.decalage < 0.0 { "−" } else { "+" };
                        ui.label(format!("{} {signe} {}", niveau.nom, unite.afficher(self.decalage.abs())));
                    }
                });
            });
        });
    }
}

/// Segments des poutres d'une forme tracée : polyligne (segments enchaînés, fermée ou
/// non) ou contour de rectangle.
pub(crate) fn segments(f: &noyau::forme2d::Forme2D) -> Vec<(DVec2, DVec2)> {
    use noyau::forme2d::Forme2D;
    let (points, fermee): (Vec<DVec2>, bool) = match f {
        Forme2D::Polyligne { points, fermee, .. } => (points.clone(), *fermee),
        _ => match f.polylignes().into_iter().next() {
            Some(p) => (p, true),
            None => return Vec::new(),
        },
    };
    let mut v: Vec<(DVec2, DVec2)> = points.windows(2).map(|w| (w[0], w[1])).collect();
    if fermee && points.len() > 2 && points[0].distance(points[points.len() - 1]) > 1e-6 {
        v.push((points[points.len() - 1], points[0]));
    }
    v.retain(|(a, b)| a.distance(*b) > 1e-6);
    v
}

/// Emprise en plan des poutres d'une polyligne : un quadrilatère par segment, côtés
/// raccordés en onglet aux sommets intérieurs (n° 9).
pub(crate) fn emprise(points: &[DVec2], largeur: f64, justification: Justification) -> Vec<[DVec2; 4]> {
    let segs: Vec<(DVec2, DVec2)> = points.windows(2).filter(|w| w[0].distance(w[1]) > 1e-6).map(|w| (w[0], w[1])).collect();
    let decaler = |d: f64| -> Vec<(DVec2, DVec2)> {
        segs.iter().map(|(a, b)| { let n = (*b - *a).normalize().perp() * d; (*a + n, *b + n) }).collect()
    };
    let c = justification.decalage(largeur);
    let (gauche, droite) = (decaler(c + largeur / 2.0), decaler(c - largeur / 2.0));
    // intersection des côtés de deux segments consécutifs (onglet), sinon bout carré
    let joint = |l: &[(DVec2, DVec2)], i: usize| -> DVec2 {
        let ((a, b), (c, d)) = (l[i], l[(i + 1) % l.len()]);
        let (u, v) = (b - a, d - c);
        let det = u.perp_dot(v);
        if det.abs() < 1e-9 { return b; }
        a + u * ((c - a).perp_dot(v) / det)
    };
    let ferme = segs.len() >= 3 && segs[0].0.distance(segs[segs.len() - 1].1) < 1e-6;
    (0..segs.len()).map(|i| {
        let precedent = (i + segs.len() - 1) % segs.len();
        let g0 = if i > 0 || ferme { joint(&gauche, precedent) } else { gauche[i].0 };
        let d0 = if i > 0 || ferme { joint(&droite, precedent) } else { droite[i].0 };
        let g1 = if i + 1 < segs.len() || ferme { joint(&gauche, i) } else { gauche[i].1 };
        let d1 = if i + 1 < segs.len() || ferme { joint(&droite, i) } else { droite[i].1 };
        [d0, d1, g1, g0]
    }).collect()
}

impl Application {
    fn largeur_poutre(&self) -> f64 {
        let r = &self.bim.poutre;
        r.type_poutre.as_ref().and_then(|t| self.natif.as_ref()?.0.type_poutre(t)).map_or(r.largeur, |t| t.largeur)
    }

    /// Crée une poutre par segment des formes tracées (une seule étape d'annulation).
    pub(crate) fn creer_poutres(&mut self, formes: &[noyau::forme2d::Forme2D]) {
        let Some((p, _)) = &self.natif else { return };
        let r = &self.bim.poutre;
        let Some(niveau) = r.niveau.clone().or_else(|| self.niveau_trace()) else {
            self.error = Some("Choisissez un niveau pour la poutre.".into());
            return;
        };
        let mut q = p.clone();
        let contexte = modele::ContexteElement {
            phase: self.ruban_edition.phase_creation.clone(),
            sous_projet: self.ruban_edition.sous_projet_creation.clone(),
            variante: self.ruban_edition.variante_creation.clone(),
        };
        let mut n = 0;
        for (a, b) in formes.iter().flat_map(segments) {
            n += 1;
            let id = format!("poutre-{}", noyau::uuid::uuid());
            if contexte.phase.is_some() || contexte.sous_projet.is_some() || contexte.variante.is_some() {
                q.contextes.insert(id.clone(), contexte.clone());
            }
            let mut barre = r.barre(niveau.clone(), a, b);
            barre.id = id;
            barre.nom = format!("Poutre {}", q.barres.iter().filter(|b| b.genre == modele::GenreBarre::Poutre).count() + 1);
            q.barres.push(barre);
        }
        if n == 0 { return; }
        match self.appliquer_projet(q) {
            Ok(()) => self.informer(format!("{n} poutre(s) créée(s)")),
            Err(e) => self.error = Some(e),
        }
    }

    /// Aperçu des poutres en cours de tracé : emprise réelle (largeur, justification,
    /// raccords) et ligne tracée.
    pub(crate) fn apercu_poutres(&self, painter: &egui::Painter, rect: egui::Rect, lignes: &[Vec<DVec2>]) {
        let (largeur, j) = (self.largeur_poutre(), self.bim.poutre.justification);
        let bleu = Color32::from_rgb(0, 120, 215);
        for l in lignes {
            for quad in emprise(l, largeur, j) {
                let pts: Vec<egui::Pos2> = quad.iter().filter_map(|q| self.vers_ecran(rect, *q)).collect();
                if pts.len() == 4 {
                    painter.add(egui::Shape::convex_polygon(pts.clone(), bleu.gamma_multiply(0.18), egui::Stroke::new(1.0, bleu)));
                }
            }
            // axe réel (justifié) en tirets
            for w in l.windows(2) {
                if let Some((a, b)) = axe_reel(w[0], w[1], largeur, j)
                    && let (Some(a), Some(b)) = (self.vers_ecran(rect, a), self.vers_ecran(rect, b))
                {
                    painter.add(egui::Shape::dashed_line(&[a, b], egui::Stroke::new(1.0, bleu), 6.0, 3.0));
                }
            }
        }
    }

    /// Options de la poutre dans le bandeau de dessin : justification et arase.
    pub(crate) fn options_poutre(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        let r = &mut self.bim.poutre;
        for j in Justification::TOUTES {
            if ui.selectable_label(r.justification == j, j.libelle()).on_hover_text("Position de la poutre par rapport à la ligne tracée (Ctrl : suivante, Espace : inverser)").clicked() {
                r.justification = j;
            }
        }
        ui.separator();
        egui::ComboBox::from_id_salt("arase_poutre").selected_text(r.arase.libelle()).show_ui(ui, |ui| {
            for a in Arase::TOUTES { ui.selectable_value(&mut r.arase, a, a.libelle()); }
        });
    }

    /// Ctrl seul : justification suivante ; Espace : gauche et droite échangées.
    pub(crate) fn raccourci_poutre(&mut self, justifier: bool, inverser: bool) {
        let j = &mut self.bim.poutre.justification;
        if justifier { *j = j.suivante(); }
        if inverser { *j = match *j { Justification::Gauche => Justification::Droite, Justification::Droite => Justification::Gauche, x => x }; }
    }

    /// Propriétés de l'outil Poutre : type (profil, dimensions, matériau), niveau de
    /// référence, décalage de l'arase.
    /// Emplacement « sélecteur » de la poutre : type (section libre ou type du projet) et son édition.
    pub(crate) fn selecteur_poutre(&mut self, ui: &mut egui::Ui) {
        let Some((p, _)) = &self.natif else { return };
        let (types, profils, materiaux, niveaux) = (p.types_poutres.clone(), p.profils.clone(), p.materiaux.iter().map(|m| m.nom.clone()).collect::<Vec<_>>(), p.niveaux.clone());
        let r = &mut self.bim.poutre;
        if r.type_poutre.as_ref().is_some_and(|id| !types.iter().any(|t| &t.id == id)) {
            r.type_poutre = None;
        }
        if r.niveau.as_ref().is_some_and(|id| !niveaux.iter().any(|n| &n.id == id)) {
            r.niveau = None;
        }
        let mut nouveau_type = None;
        let nom = r.type_poutre.as_ref().and_then(|id| types.iter().find(|t| &t.id == id)).map_or_else(|| format!("Section {} × {} mm", r.largeur, r.hauteur), |t| t.nom.clone());
        {
            use crate::zones::ruban::selecteur;
            let details: Vec<String> = types.iter().map(|t| format!("{} × {} mm", t.largeur, t.hauteur)).collect();
            let mut lignes = vec![selecteur::Ligne { nom: "Section rectangulaire libre", courant: r.type_poutre.is_none(), etiquette: None, sous_titre: None, detail: Some(format!("{} × {} mm", r.largeur, r.hauteur)), actions: &[] }];
            lignes.extend(types.iter().zip(&details).map(|(t, d)| selecteur::Ligne { nom: &t.nom, courant: r.type_poutre.as_ref() == Some(&t.id), etiquette: None, sous_titre: None, detail: Some(d.clone()), actions: &[] }));
            let titre = format!("Poutre · {nom}");
            if let Some(k) = selecteur::simple(ui, &titre, None, &format!("{titre}\nType de poutre : profil, dimensions, matériau."), &lignes, None).ligne {
                r.type_poutre = k.checked_sub(1).map(|i| types[i].id.clone());
            }
        }
        let type_courant = r.type_poutre.as_ref().and_then(|id| types.iter().find(|t| &t.id == id)).cloned();
        match type_courant {
            None => {
                ui.horizontal(|ui| {
                    ui.label("Section");
                    ui.add(egui::DragValue::new(&mut r.largeur).range(10.0..=5000.0).dimensions());
                    ui.label("×");
                    ui.add(egui::DragValue::new(&mut r.hauteur).range(10.0..=5000.0).dimensions());
                });
                if ui.button("Créer un type d'après cette section…").clicked() {
                    nouveau_type = Some(TypePoutre { id: format!("type-poutre-{}", noyau::uuid::uuid()), nom: format!("Poutre {} × {}", r.largeur, r.hauteur), profil: None, largeur: r.largeur, hauteur: r.hauteur, materiau: None });
                }
            }
            Some(mut t) => {
                let avant = t.clone();
                ui.horizontal(|ui| { ui.label("Nom"); ui.text_edit_singleline(&mut t.nom); });
                egui::ComboBox::from_label("Profil").selected_text(t.profil.as_ref().and_then(|id| profils.iter().find(|x| &x.id == id)).map_or("Rectangulaire", |x| x.nom.as_str())).show_ui(ui, |ui| {
                    ui.selectable_value(&mut t.profil, None, "Rectangulaire");
                    for x in &profils { ui.selectable_value(&mut t.profil, Some(x.id.clone()), &x.nom); }
                });
                ui.horizontal(|ui| {
                    ui.label("Largeur");
                    ui.add(egui::DragValue::new(&mut t.largeur).range(10.0..=5000.0).dimensions());
                    ui.label("Hauteur");
                    ui.add(egui::DragValue::new(&mut t.hauteur).range(10.0..=5000.0).dimensions());
                });
                egui::ComboBox::from_label("Matériau").selected_text(t.materiau.clone().unwrap_or_else(|| "Par défaut".into())).show_ui(ui, |ui| {
                    ui.selectable_value(&mut t.materiau, None, "Par défaut");
                    for m in &materiaux { ui.selectable_value(&mut t.materiau, Some(m.clone()), m); }
                });
                if t != avant { nouveau_type = Some(t); }
            }
        }
        if let Some(t) = nouveau_type {
            let mut q = self.natif.as_ref().unwrap().0.clone();
            match q.types_poutres.iter_mut().find(|x| x.id == t.id) {
                Some(x) => *x = t.clone(),
                None => q.types_poutres.push(t.clone()),
            }
            match self.appliquer_projet(q) {
                Ok(()) => self.bim.poutre.type_poutre = Some(t.id),
                Err(e) => self.error = Some(e),
            }
        }
    }

    /// Emplacement « schéma » de la poutre : coupe de la poutre avec ses cotes.
    pub(crate) fn schema_poutre(&mut self, ui: &mut egui::Ui) {
        let Some((p, _)) = &self.natif else { return };
        let niveau_vue = self.niveau_trace();
        let r = &mut self.bim.poutre;
        let niveau = r.niveau.as_ref().or(niveau_vue.as_ref()).and_then(|id| p.niveau(id));
        r.croquis(ui, p, niveau);
    }

    /// Emplacement « paramètres » de la poutre : niveau de référence, décalage, arase.
    pub(crate) fn parametres_poutre(&mut self, ui: &mut egui::Ui) {
        let Some((p, _)) = &self.natif else { return };
        let niveaux = p.niveaux.clone();
        let niveau_vue = self.niveau_trace();
        let r = &mut self.bim.poutre;
        if r.niveau.as_ref().is_some_and(|id| !niveaux.iter().any(|n| &n.id == id)) {
            r.niveau = None;
        }
        crate::zones::palette_creation::titre_parametres(ui, "Poutre");
        let actuel = r.niveau.clone().or(niveau_vue.clone());
        egui::ComboBox::from_label("Niveau de référence").selected_text(actuel.as_ref().and_then(|id| niveaux.iter().find(|n| &n.id == id)).map_or("—", |n| n.nom.as_str())).show_ui(ui, |ui| {
            for n in &niveaux { if ui.selectable_label(actuel.as_ref() == Some(&n.id), &n.nom).clicked() { r.niveau = Some(n.id.clone()); } }
        });
        ui.horizontal(|ui| {
            ui.label("Décalage");
            ui.add(egui::DragValue::new(&mut r.decalage).speed(10.0).dimensions()).on_hover_text("Cote de l'arase choisie au-dessus du niveau de référence");
        });
        egui::ComboBox::from_label("Arase").selected_text(r.arase.libelle()).show_ui(ui, |ui| {
            for a in Arase::TOUTES { ui.selectable_value(&mut r.arase, a, a.libelle()); }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// N° 9 : emprise d'une polyligne en L de 200 mm de large, justifiée à gauche :
    /// deux quadrilatères raccordés en onglet, toute la largeur d'un côté de la ligne.
    #[test]
    fn emprise_en_l_raccordee() {
        let points = [DVec2::ZERO, DVec2::new(1000.0, 0.0), DVec2::new(1000.0, 1000.0)];
        let q = emprise(&points, 200.0, Justification::Gauche);
        assert_eq!(q.len(), 2);
        // tracé vers +x, puis +y : la poutre est à droite de la ligne (y < 0, puis x > 1000)
        assert!(q[0].iter().all(|p| p.y <= 1e-9 && p.y >= -200.0 - 1e-9));
        // sommet d'onglet partagé par les deux segments, au coin extérieur (1200, −200)
        assert!(q[0][1].distance(DVec2::new(1200.0, -200.0)) < 1e-6 && q[1][0].distance(DVec2::new(1200.0, -200.0)) < 1e-6);
        let axe = emprise(&points, 200.0, Justification::Axe);
        assert!(axe[0].iter().all(|p| p.y.abs() <= 100.0 + 1e-9));
    }

    /// Rectangle tracé : quatre poutres fermant le contour.
    #[test]
    fn rectangle_quatre_poutres() {
        let f = noyau::forme2d::Forme2D::Polyligne { points: vec![DVec2::ZERO, DVec2::new(3000.0, 0.0), DVec2::new(3000.0, 2000.0), DVec2::new(0.0, 2000.0)], fermee: true, courbures: Vec::new() };
        assert_eq!(segments(&f).len(), 4);
        let mut points: Vec<_> = segments(&f).into_iter().map(|s| s.0).collect();
        points.push(points[0]);
        let q = emprise(&points, 200.0, Justification::Axe);
        assert_eq!((q[0][0], q[0][3]), (q[3][1], q[3][2]));
    }
}
