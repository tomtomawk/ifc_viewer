//! Escaliers et garde-corps : création, schéma, limites.
use super::*;

impl ViewerApp {
    /// Crée un escalier de ligne de foulée `trace` : du niveau de tracé au niveau
    /// immédiatement au-dessus (sinon 2,80 m), avec ses garde-corps selon le ruban.
    pub(in crate::ui) fn creer_escalier(&mut self, trace: Vec<[f64; 2]>) -> Result<(), String> {
        use crate::ui::ruban::Outil;
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let (niveau, haut, hauteur, decalage_bas) = self.base_escalier().ok_or("Créez d'abord un niveau.")?;
        let trace = self.trace_justifie(&trace);
        let escalier = crate::modele::Escalier {
            id: String::new(),
            nom: format!("Escalier {}", p.escaliers.len() + 1),
            niveau: niveau.clone(),
            niveau_haut: haut,
            decalage_bas,
            decalage_haut: self.dessin2d.esc_decalage_haut,
            hauteur,
            trace,
            largeur: self.ruban_edition.nombre_option(Outil::Escalier, 0),
            hauteur_marche: self.ruban_edition.nombre_option(Outil::Escalier, 1),
            paillasse: 150.0,
        };
        let id = self.executer_bim(Commande::Escalier { escalier })?.into_iter().next().ok_or("Escalier non créé.")?;
        let cotes = match self.ruban_edition.texte_option(Outil::Escalier, 2).as_str() {
            "Gauche" => vec![0],
            "Droite" => vec![1],
            "Aucun" => vec![],
            _ => vec![0, 1],
        };
        self.garde_corps_escalier(&id, &cotes)?;
        self.status = "Escalier créé — Ctrl+S pour enregistrer.".into();
        Ok(())
    }

    /// Garde-corps hébergés par l'escalier `id` : côtés 0 (gauche) et 1 (droite) du
    /// sens de montée, à 50 mm du bord.
    pub(in crate::ui) fn garde_corps_escalier(&mut self, id: &str, cotes: &[usize]) -> Result<(), String> {
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let e = p.escaliers.iter().find(|e| e.id == id).ok_or("Escalier absent.")?.clone();
        let traces = e.traces_garde_corps(50.0);
        for &k in cotes {
            self.creer_garde_corps(traces[k].clone(), Some(e.id.clone()))?;
        }
        Ok(())
    }

    /// Crée un garde-corps (réglages du ruban), hébergé ou non par un escalier.
    pub(in crate::ui) fn creer_garde_corps(&mut self, trace: Vec<[f64; 2]>, escalier: Option<String>) -> Result<String, String> {
        use crate::ui::ruban::Outil;
        let (p, _) = self.natif.as_ref().ok_or("Aucun projet natif.")?;
        let niveau = match escalier.as_ref().and_then(|id| p.escaliers.iter().find(|e| &e.id == id)) {
            Some(e) => e.niveau.clone(),
            None => self.niveau_trace().ok_or("Créez d'abord un niveau.")?,
        };
        let standard = self.standard_garde_corps();
        let type_actif = self.ruban_edition.type_garde_corps.as_ref()
            .and_then(|id| p.types_garde_corps.iter().find(|t| &t.id == id));
        let t = type_actif.unwrap_or(&standard);
        let garde_corps = crate::modele::GardeCorps {
            id: String::new(),
            nom: format!("Garde-corps {}", p.garde_corps.len() + 1),
            niveau,
            decalage: if escalier.is_some() { 0.0 } else { self.ruban_edition.nombre_option(Outil::GardeCorps, 1) },
            trace,
            hauteur: t.hauteur,
            remplissage: t.remplissage,
            type_garde_corps: type_actif.map(|t| t.id.clone()),
            escalier,
        };
        self.executer_bim(Commande::GardeCorps { garde_corps })?.into_iter().next().ok_or_else(|| "Garde-corps non créé.".to_string())
    }

    /// Niveau de départ, niveau d'arrivée, hauteur à franchir et décalage de la base de
    /// l'escalier en création, selon ses limites (par défaut : niveau du plan, premier
    /// niveau au-dessus, 2,80 m sans niveau au-dessus).
    pub(in crate::ui) fn base_escalier(&self) -> Option<(String, Option<String>, f64, f64)> {
        let (p, _) = self.natif.as_ref()?;
        let d = &self.dessin2d;
        let niveau = d.esc_bas.clone().filter(|id| p.niveau(id).is_some()).or_else(|| self.niveau_trace())?;
        let base = p.niveau(&niveau).map_or(0.0, |n| n.altitude) + d.esc_decalage_bas;
        let libre = if d.esc_hauteur_libre > 0.0 { d.esc_hauteur_libre } else { 2800.0 };
        let (haut, hauteur) = match &d.esc_haut {
            HautEscalier::Niveau(id) if p.niveau(id).is_some() => (Some(id.clone()), p.niveau(id).map_or(0.0, |n| n.altitude) + d.esc_decalage_haut - base),
            HautEscalier::Libre => (None, libre),
            _ => match p.niveaux.iter().filter(|n| n.altitude + d.esc_decalage_haut > base + 1.0).min_by(|a, b| a.altitude.total_cmp(&b.altitude)) {
                Some(n) => (Some(n.id.clone()), n.altitude + d.esc_decalage_haut - base),
                None => (None, libre),
            },
        };
        Some((niveau, haut, hauteur, d.esc_decalage_bas))
    }

    /// Croquis des limites de l'escalier, comme pour le mur : un escalier en coupe entre
    /// ses deux limites, Haut et Bas sur une ligne chacune (niveau, puis décalage de la
    /// base ou hauteur libre).
    pub(in crate::ui) fn croquis_limites_escalier(&mut self, ui: &mut egui::Ui) {
        use crate::config::options::Dimensions;
        let Some((p, _)) = self.natif.as_ref() else { return };
        let niveaux: Vec<(String, String)> = p.niveaux.iter().map(|n| (n.id.clone(), n.nom.clone())).collect();
        let nom_plan = self.niveau_trace().and_then(|id| p.niveau(&id).map(|n| n.nom.clone())).unwrap_or_default();
        let nom = |id: &str| niveaux.iter().find(|(i, _)| i == id).map_or(String::new(), |(_, n)| n.clone());
        let bleu = Color32::from_rgb(18, 158, 205);
        let origine = ui.cursor().min;
        let mut centres = [0.0_f32; 2];
        let mut gauche = 0.0_f32;
        ui.add_space(10.0);
        ui.horizontal_top(|ui| {
            ui.add_space(64.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 14.0;
                for haut in [true, false] {
                    let ligne = ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.add_sized([30.0, 20.0], egui::Label::new(RichText::new(if haut { "Haut" } else { "Bas" }).color(bleu)));
                        let champ = 72.0;
                        let largeur = (ui.available_width() - champ - 4.0).max(60.0);
                        if haut {
                            let texte = match &self.dessin2d.esc_haut {
                                HautEscalier::Auto => "Niveau suivant".to_string(),
                                HautEscalier::Niveau(id) => nom(id),
                                HautEscalier::Libre => "Libre".to_string(),
                            };
                            egui::ComboBox::from_id_salt("esc_haut").width(largeur).truncate().selected_text(texte).show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.dessin2d.esc_haut, HautEscalier::Auto, "Niveau suivant");
                                for (id, n) in &niveaux {
                                    ui.selectable_value(&mut self.dessin2d.esc_haut, HautEscalier::Niveau(id.clone()), n);
                                }
                                ui.selectable_value(&mut self.dessin2d.esc_haut, HautEscalier::Libre, "Libre (hauteur)");
                            }).response.on_hover_text("Limite haute : un niveau, le niveau suivant, ou libre (hauteur saisie)");
                            if self.dessin2d.esc_haut == HautEscalier::Libre || (self.dessin2d.esc_haut == HautEscalier::Auto && self.base_escalier().is_some_and(|b| b.1.is_none())) {
                                if self.dessin2d.esc_hauteur_libre <= 0.0 { self.dessin2d.esc_hauteur_libre = 2800.0; }
                                ui.add_sized([champ, 20.0], egui::DragValue::new(&mut self.dessin2d.esc_hauteur_libre).speed(10.0).range(1.0..=f64::MAX).dimensions()).on_hover_text("Hauteur à franchir");
                            } else {
                                ui.add_sized([champ, 20.0], egui::DragValue::new(&mut self.dessin2d.esc_decalage_haut).speed(10.0).dimensions()).on_hover_text("Décalage de l'arrivée par rapport au niveau");
                            }
                        } else {
                            let texte = self.dessin2d.esc_bas.as_deref().map_or(nom_plan.clone(), |id| nom(id));
                            egui::ComboBox::from_id_salt("esc_bas").width(largeur).truncate().selected_text(texte).show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.dessin2d.esc_bas, None, format!("Niveau du plan ({nom_plan})"));
                                for (id, n) in &niveaux {
                                    ui.selectable_value(&mut self.dessin2d.esc_bas, Some(id.clone()), n);
                                }
                            }).response.on_hover_text("Limite basse : niveau de départ de l'escalier");
                            ui.add_sized([champ, 20.0], egui::DragValue::new(&mut self.dessin2d.esc_decalage_bas).speed(10.0).dimensions()).on_hover_text("Décalage de la base par rapport au niveau");
                        }
                    });
                    gauche = ligne.response.rect.left();
                    centres[if haut { 0 } else { 1 }] = ligne.response.rect.center().y;
                }
            });
        });
        ui.add_space(10.0);
        let peintre = ui.painter();
        let (x0, x1) = (origine.x + 6.0, origine.x + 54.0);
        let (yh, yb) = (centres[0], centres[1]);
        let niveau_trait = egui::Stroke::new(1.5, bleu);
        peintre.line_segment([egui::pos2(x0 - 4.0, yh), egui::pos2(x1 + 4.0, yh)], niveau_trait);
        peintre.line_segment([egui::pos2(x0 - 4.0, yb), egui::pos2(x1 + 4.0, yb)], niveau_trait);
        let (dx, dy) = ((x1 - x0) / 4.0, (yb - yh) / 4.0);
        let mut points = vec![egui::pos2(x0, yb)];
        for _ in 0..4 {
            let b = points[points.len() - 1];
            points.push(b - egui::vec2(0.0, dy));
            points.push(b + egui::vec2(dx, -dy));
        }
        peintre.add(egui::Shape::line(points, egui::Stroke::new(1.4, ui.visuals().text_color())));
        // rappels vers les lignes de limites
        let rappel = egui::Stroke::new(0.8, bleu.gamma_multiply(0.8));
        for y in [yh, yb] {
            let mut x = x1 + 6.0;
            while x < gauche - 3.0 {
                peintre.line_segment([egui::pos2(x, y), egui::pos2((x + 2.5).min(gauche - 3.0), y)], rappel);
                x += 5.0;
            }
        }
    }

    /// Giron visé (pas de marche, jamais appliqué au dessin) : imposé, sinon celui que donne Blondel (2h + g = 630) pour la
    /// hauteur de marche choisie. Sert au magnétisme de la longueur des volées.
    pub(in crate::ui) fn giron_vise(&self) -> f64 {
        use crate::ui::ruban::Outil;
        let Some((_, _, hauteur, _)) = self.base_escalier() else { return 280.0 };
        let hauteur = hauteur.max(1.0);
        let vise = self.ruban_edition.nombre_option(Outil::Escalier, 1).max(50.0);
        let h = hauteur / ((hauteur / vise).round().max(2.0));
        self.dessin2d.giron_impose.unwrap_or((630.0 - 2.0 * h).max(150.0))
    }

    /// Ligne de foulée centrée à partir de la ligne tracée : chaque volée a sa
    /// justification (axe, bord gauche, bord droit), figée quand elle est posée ; Espace
    /// ne change que celle de la volée en cours.
    pub(in crate::ui) fn trace_justifie(&self, trace: &[[f64; 2]]) -> Vec<[f64; 2]> {
        let l = self.ruban_edition.nombre_option(crate::ui::ruban::Outil::Escalier, 0);
        let segments = trace.len().saturating_sub(1);
        let d: Vec<f64> = (0..segments).map(|i| match self.dessin2d.just_segments.get(i).copied().unwrap_or(self.dessin2d.justification_escalier) {
            1 => -l / 2.0,
            2 => l / 2.0,
            _ => 0.0,
        }).collect();
        if d.is_empty() || d.iter().all(|x| *x == d[0]) {
            return crate::modele::elements::escalier::decaler_polyligne(trace, d.first().copied().unwrap_or(0.0));
        }
        let p: Vec<DVec2> = trace.iter().map(|q| DVec2::from_array(*q)).collect();
        let t: Vec<DVec2> = (0..segments).map(|i| (p[i + 1] - p[i]).normalize_or_zero()).collect();
        let mut sortie = vec![p[0] + t[0].perp() * d[0]];
        for k in 1..segments {
            let (a0, a1) = (p[k] + t[k - 1].perp() * d[k - 1], p[k] + t[k].perp() * d[k]);
            let den = t[k - 1].perp_dot(t[k]);
            let milieu = (a0 + a1) / 2.0;
            sortie.push(if den.abs() < 1e-6 {
                milieu
            } else {
                let q = a0 + t[k - 1] * ((a1 - a0).perp_dot(t[k]) / den);
                if q.distance(p[k]) > 3.0 * l.max(1.0) { milieu } else { q }
            });
        }
        sortie.push(p[segments] + t[segments - 1].perp() * d[segments - 1]);
        sortie.into_iter().map(|q| q.to_array()).collect()
    }

    /// Propriétés de l'escalier en création : schéma de trois marches (fixe) avec, à côté
    /// de chaque cote, son champ modifiable : giron, hauteur de marche, nombre de marches
    /// et Blondel. Un champ ne s'applique qu'à Entrée ou en le quittant.
    pub(in crate::ui) fn schema_escalier(&mut self, ui: &mut egui::Ui) {
        use crate::ui::ruban::Outil;
        ui.label(RichText::new("Escalier").strong());
        self.croquis_limites_escalier(ui);
        let Some((_, _, hauteur, _)) = self.base_escalier() else { return };
        if hauteur <= 1.0 {
            ui.colored_label(Color32::from_rgb(200, 40, 40), "L'escalier doit monter : limite haute au-dessus de la base.");
            return;
        }
        let vise = self.ruban_edition.nombre_option(Outil::Escalier, 1).max(50.0);
        let nombre = ((hauteur / vise).round() as u32).max(2);
        let h = hauteur / nombre as f64;
        let impose = self.dessin2d.giron_impose;
        // le giron saisi ou par défaut (Blondel) ne suit jamais le tracé : il sert de pas
        let giron = impose.unwrap_or((630.0 - 2.0 * h).max(150.0));
        let blondel = (2.0 * h + giron) / 10.0;
        let hors = !(60.0..=64.0).contains(&blondel);
        let u = crate::config::options::unite();
        let (zone, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().max(220.0), 100.0), Sense::hover());
        let painter = ui.painter_at(zone);
        let trait_ = egui::Stroke::new(1.6, ui.visuals().text_color());
        let cote = egui::Stroke::new(1.0, Color32::from_rgb(55, 130, 200));
        // trois marches, géométrie fixe
        let (g, hm) = (46.0_f32, 22.0_f32);
        let x0 = zone.left() + 84.0;
        let y0 = zone.top() + 70.0;
        let mut points = vec![egui::pos2(x0, y0)];
        for _ in 0..3 {
            let b = points[points.len() - 1];
            points.push(b - egui::vec2(0.0, hm));
            points.push(b + egui::vec2(g, -hm));
        }
        painter.add(egui::Shape::line(points, trait_));
        painter.line_segment([egui::pos2(x0 - 12.0, y0), egui::pos2(x0 + 3.0 * g + 8.0, y0)], egui::Stroke::new(1.0, ui.visuals().weak_text_color()));
        // cote de hauteur (première contremarche) et de giron (deuxième marche)
        painter.line_segment([egui::pos2(x0 - 6.0, y0), egui::pos2(x0 - 6.0, y0 - hm)], cote);
        painter.line_segment([egui::pos2(x0 + g, y0 + 6.0), egui::pos2(x0 + 2.0 * g, y0 + 6.0)], cote);
        let champ = |ui: &mut egui::Ui, rect: egui::Rect, nom: &str, v: f64, facteur: f64, decimales: Option<usize>| -> Option<f64> {
            // new_child : n'avance pas le curseur du panneau (scope_builder le ramènerait dans le schéma)
            let mut enfant = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            champ_valeur(&mut enfant, nom, v, facteur, decimales, rect.width())
        };
        let (l, hc) = (58.0, 20.0);
        let r_h = egui::Rect::from_min_size(egui::pos2(x0 - 12.0 - l, y0 - hm / 2.0 - hc / 2.0), egui::vec2(l, hc));
        let r_g = egui::Rect::from_min_size(egui::pos2(x0 + 1.5 * g - l / 2.0, y0 + 10.0), egui::vec2(l, hc));
        let r_n = egui::Rect::from_min_size(egui::pos2(x0 + 3.0 * g + 12.0, y0 - 3.0 * hm), egui::vec2(l, hc));
        painter.text(egui::pos2(r_h.left(), r_h.top() - 1.0), egui::Align2::LEFT_BOTTOM, "hauteur", egui::FontId::proportional(10.0), cote.color);
        painter.text(egui::pos2(r_g.right() + 4.0, r_g.center().y), egui::Align2::LEFT_CENTER, "giron", egui::FontId::proportional(10.0), cote.color);
        painter.text(egui::pos2(r_n.left(), r_n.top() - 1.0), egui::Align2::LEFT_BOTTOM, "marches", egui::FontId::proportional(10.0), cote.color);
        let nouvelle_h = champ(ui, r_h, "esc_h", h, u.facteur(), None);
        let nouveau_giron = champ(ui, r_g, "esc_g", giron, u.facteur(), None);
        let nouveau_nombre = champ(ui, r_n, "esc_n", nombre as f64, 1.0, Some(0));
        ui.horizontal(|ui| {
            ui.label("Blondel 2h + g");
            let rouge = Color32::from_rgb(200, 40, 40);
            if hors { ui.visuals_mut().override_text_color = Some(rouge); }
            let nouveau = champ_valeur(ui, "esc_b", blondel * 10.0, 10.0, Some(1), 52.0);
            ui.label("cm");
            if hors { ui.label("hors confort (60 à 64)"); }
            if let Some(b) = nouveau { self.dessin2d.giron_impose = Some((b - 2.0 * h).max(50.0)); }
            if impose.is_some() && ui.small_button("↺").on_hover_text("Revenir au giron calculé d'après le tracé").clicked() {
                self.dessin2d.giron_impose = None;
            }
        });
        if let Some(n) = nouveau_nombre {
            let n = n.round().clamp(2.0, 100.0);
            self.ruban_edition.definir_nombre_option(Outil::Escalier, 1, hauteur / n);
        } else if let Some(v) = nouvelle_h {
            self.ruban_edition.definir_nombre_option(Outil::Escalier, 1, v.clamp(50.0, 400.0));
        }
        if let Some(v) = nouveau_giron {
            self.dessin2d.giron_impose = Some(v.max(50.0));
        }
    }

    /// Escalier en cours : emmarchement (bords de la largeur) et flèche de montée.
    pub(in crate::ui) fn apercu_escalier(&self, painter: &egui::Painter, rect: egui::Rect, lignes: &[Vec<DVec2>]) {
        let w = self.ruban_edition.nombre_option(crate::ui::ruban::Outil::Escalier, 0) / 2.0;
        let couleur = Color32::from_rgb(55, 130, 200);
        let giron = self.giron_vise();
        for l in lignes.iter().filter(|l| l.len() >= 2) {
            let brut: Vec<[f64; 2]> = l.iter().map(|p| p.to_array()).collect();
            // justification : la ligne tracée est l'axe ou un bord ; la foulée est au centre
            let trace = self.trace_justifie(&brut);
            // marches en pointillé : un trait toutes les `giron` le long de chaque volée
            let n = trace.len();
            for i in 0..n - 1 {
                let (a, b) = (DVec2::from_array(trace[i]), DVec2::from_array(trace[i + 1]));
                let d = (b - a).normalize_or_zero();
                if d == DVec2::ZERO { continue; }
                let debut = if i > 0 { w } else { 0.0 };
                let fin = a.distance(b) - if i + 2 < n { w } else { 0.0 };
                let mut t = debut + giron;
                let mut k = 0;
                while t <= fin + 1e-6 && k < 300 {
                    let c = a + d * t;
                    let nrm = d.perp() * w;
                    if let (Some(p), Some(q)) = (self.vers_ecran(rect, c - nrm), self.vers_ecran(rect, c + nrm)) {
                        painter.add(egui::Shape::dashed_line(&[p, q], egui::Stroke::new(1.0, couleur), 4.0, 3.0));
                    }
                    t += giron;
                    k += 1;
                }
            }
            for d in [w, -w] {
                let bord: Vec<egui::Pos2> = crate::modele::elements::escalier::decaler_polyligne(&trace, d).iter().filter_map(|q| self.vers_ecran(rect, DVec2::from_array(*q))).collect();
                painter.add(egui::Shape::line(bord, egui::Stroke::new(1.5, couleur)));
            }
            // flèche à l'arrivée
            let (a, b) = (l[l.len() - 2], l[l.len() - 1]);
            if let (Some(pa), Some(pb)) = (self.vers_ecran(rect, a), self.vers_ecran(rect, b)) {
                let d = (pb - pa).normalized() * 12.0;
                let n = egui::vec2(-d.y, d.x) * 0.5;
                painter.add(egui::Shape::convex_polygon(vec![pb, pb - d + n, pb - d - n], couleur, egui::Stroke::NONE));
            }
        }
    }
}
