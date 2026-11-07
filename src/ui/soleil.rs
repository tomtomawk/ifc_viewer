//! Lieu du projet (Début), date et heure du soleil de la vue (menu Ombres) et réglages
//! du ciel physique (fenêtre Ciel, pas encore reliée au rendu).
use super::*;
use crate::modele::soleil::{self, AmbianceCiel, Lieu, ReglagesCiel};

const MOIS: [&str; 12] = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];

fn heure_texte(h: f32) -> String {
    let m = (h * 60.0).round() as i32;
    format!("{:02} h {:02}", (m / 60) % 24, m % 60)
}

impl ViewerApp {
    /// Identifiant de la vue active pour ses réglages de rendu : vue du projet, ou « 3d ».
    pub(super) fn id_vue_rendu(&self) -> Option<String> {
        let numero = self.vue_active.and_then(|i| self.scene.as_ref()?.navigateur.as_ref()?.vues.get(i)).map(|v| v.revit_id)?;
        let projet = &self.natif.as_ref()?.0;
        if scene::numero_vue("3d") == numero {
            return Some("3d".into());
        }
        projet.vues.iter().find(|v| scene::numero_vue(&v.id) == numero).map(|v| v.id.clone())
    }

    /// Réglages de rendu en cours de la vue.
    pub(super) fn rendu_vue_courant(&self) -> soleil::RenduVue {
        soleil::RenduVue {
            style: Some(self.style.nom().into()),
            ombres_portees: self.ombres_portees,
            ombre_ambiante: self.ombre_ambiante,
            jour: self.soleil_jour,
            mois: self.soleil_mois,
            heure: self.soleil_heure,
            ciel: self.ciel.clone(),
        }
    }

    /// Enregistre les réglages de rendu de la vue active dans le projet s'ils ont changé
    /// (une étape d'annulation).
    pub(super) fn enregistrer_rendu_vue(&mut self) {
        if self.saisie_active || self.partage_en_cours { return; }
        let Some(id) = self.id_vue_rendu() else { return };
        let rendu = self.rendu_vue_courant();
        let Some((projet, _)) = &mut self.natif else { return };
        if projet.rendus_vues.get(&id) == Some(&rendu) {
            return;
        }
        let avant = projet.clone();
        projet.rendus_vues.insert(id, rendu);
        let resultat = if let Some(c) = &mut self.collaboration { c.editer(&avant, projet) } else { self.historique.pousser(&avant, projet) };
        if let Err(e) = resultat {
            *projet = avant;
            self.error = Some(e);
            return;
        }
        self.natif_modifie = true;
    }

    /// Applique les réglages de rendu enregistrés de la vue `id` (ouverture de la vue).
    pub(super) fn appliquer_rendu_vue(&mut self, id: &str) {
        let Some(r) = self.natif.as_ref().and_then(|(p, _)| p.rendus_vues.get(id).cloned()) else { return };
        if let Some(s) = r.style.as_deref().and_then(|n| StyleVisuel::TOUS.into_iter().find(|s| s.nom() == n)) {
            self.style = s;
        }
        self.ombres_portees = r.ombres_portees;
        self.ombre_ambiante = r.ombre_ambiante;
        self.soleil_mois = r.mois.clamp(1, 12);
        self.soleil_jour = r.jour.clamp(1, soleil::jours_du_mois(self.soleil_mois));
        self.soleil_heure = r.heure.clamp(0.0, 23.99);
        self.ciel = r.ciel;
    }

    /// Lieu du projet (Rennes par défaut).
    pub(super) fn lieu_projet(&self) -> Lieu {
        self.natif.as_ref().and_then(|(p, _)| p.lieu.clone()).unwrap_or_default()
    }

    /// Azimut et hauteur du soleil de la vue (degrés).
    pub(super) fn position_soleil(&self) -> (f64, f64) {
        soleil::position(&self.lieu_projet(), self.soleil_jour, self.soleil_mois, self.soleil_heure as f64)
    }

    /// Direction vers le soleil dans la scène : date, heure et lieu du projet, nord du
    /// projet. Sous l'horizon, le soleil est gardé au ras de l'horizon (2°).
    pub(super) fn direction_soleil(&self) -> Vec3 {
        let (az, h) = self.position_soleil();
        let nord = self.natif.as_ref().map_or(0.0, |(p, _)| p.orientation_nord);
        let [x, y, z] = soleil::direction(az, h.max(2.0), nord);
        Vec3::new(x as f32, y as f32, z as f32).normalize()
    }

    /// Choix du lieu du projet : ville, coordonnées collées (« 48.1173, -1.6778 »),
    /// latitude, longitude, fuseau et heure d'été. Appliqué en fin de saisie.
    pub(super) fn editeur_lieu(&mut self, ui: &mut egui::Ui) {
        let Some(actuel) = self.natif.as_ref().map(|(p, _)| p.lieu.clone()) else { return };
        let id = egui::Id::unique("saisie_lieu_projet");
        let mut l = ui.data(|d| d.get_temp::<Lieu>(id)).unwrap_or_else(|| actuel.clone().unwrap_or_default());
        let mut fini = false;
        egui::Grid::new("lieu_projet").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
            ui.label("Ville");
            egui::ComboBox::from_id_salt("lieu_ville").width(180.0).selected_text(if l.nom.is_empty() { "Coordonnées libres".to_string() } else { l.nom.clone() }).show_ui(ui, |ui| {
                for (nom, lat, lon, fuseau) in soleil::VILLES {
                    if ui.selectable_label(l.nom == *nom, *nom).clicked() {
                        l = Lieu { nom: nom.to_string(), latitude: *lat, longitude: *lon, fuseau: *fuseau, heure_ete: true };
                        fini = true;
                    }
                }
            });
            ui.end_row();
            ui.label("Coller").on_hover_text("Coordonnées copiées d'une carte (Google Maps, Géoportail…) : « latitude, longitude » en degrés décimaux");
            let id_texte = id.with("texte");
            let mut texte = ui.data(|d| d.get_temp::<String>(id_texte)).unwrap_or_default();
            let r = ui.add(egui::TextEdit::singleline(&mut texte).desired_width(180.0).hint_text("48.1173, -1.6778"));
            if r.lost_focus() || r.changed() && texte.contains(',') && texte.split(',').count() == 2 && texte.split(',').all(|x| x.trim().parse::<f64>().is_ok()) {
                let v: Vec<f64> = texte.split(|c| c == ',' || c == ';').filter_map(|x| x.trim().parse().ok()).collect();
                if let [lat, lon] = v[..] && lat.abs() <= 90.0 && lon.abs() <= 180.0 {
                    l.latitude = lat;
                    l.longitude = lon;
                    l.nom.clear();
                    fini = true;
                    texte.clear();
                }
            }
            ui.data_mut(|d| d.insert_temp(id_texte, texte));
            ui.end_row();
            let champ = |ui: &mut egui::Ui, valeur: &mut f64, borne: f64, positif: &str, negatif: &str| {
                ui.horizontal(|ui| {
                    let r = ui.add(egui::DragValue::new(valeur).speed(0.001).range(-borne..=borne).max_decimals(5).suffix(" °"));
                    ui.label(RichText::new(if *valeur >= 0.0 { positif } else { negatif }).weak());
                    r.drag_stopped() || r.lost_focus()
                })
                .inner
            };
            ui.label("Latitude");
            let avant = (l.latitude, l.longitude);
            fini |= champ(ui, &mut l.latitude, 90.0, "Nord", "Sud");
            ui.end_row();
            ui.label("Longitude");
            fini |= champ(ui, &mut l.longitude, 180.0, "Est", "Ouest");
            ui.end_row();
            if (l.latitude, l.longitude) != avant {
                l.nom.clear();
            }
            ui.label("Fuseau");
            let r = ui.add(egui::DragValue::new(&mut l.fuseau).speed(0.25).range(-12.0..=14.0).max_decimals(2).prefix("UTC ").custom_formatter(|v, _| format!("{v:+}")));
            fini |= r.drag_stopped() || r.lost_focus();
            ui.end_row();
            ui.label("");
            fini |= ui.checkbox(&mut l.heure_ete, "Heure d'été (Europe)").on_hover_text("+1 h du dernier dimanche de mars au dernier dimanche d'octobre").changed();
            ui.end_row();
        });
        let (az, h) = soleil::position(&l, self.soleil_jour, self.soleil_mois, self.soleil_heure as f64);
        ui.label(RichText::new(format!("Soleil de la vue : azimut {az:.0}°, hauteur {h:.0}°")).small().weak());
        if fini && Some(&l) != actuel.as_ref() {
            ui.data_mut(|d| d.remove::<Lieu>(id));
            if let Some((p, _)) = &self.natif {
                let mut p = p.clone();
                p.lieu = Some(l);
                match self.appliquer_projet(p) {
                    Ok(_) => self.status = "Lieu du projet modifié.".into(),
                    Err(e) => self.error = Some(e),
                }
            }
        } else {
            ui.data_mut(|d| d.insert_temp(id, l));
        }
    }

    /// Date (jour, mois) et heure du soleil de la vue, dans le menu Ombres. Vrai si changé.
    pub(super) fn reglages_soleil_vue(&mut self, ui: &mut egui::Ui) -> bool {
        let mut change = false;
        ui.label(RichText::new("Soleil de la vue").strong());
        ui.horizontal(|ui| {
            ui.label("Date");
            let max = soleil::jours_du_mois(self.soleil_mois);
            change |= ui.add(egui::DragValue::new(&mut self.soleil_jour).range(1..=max)).changed();
            egui::ComboBox::from_id_salt("soleil_mois").width(90.0).selected_text(MOIS[(self.soleil_mois.clamp(1, 12) - 1) as usize]).show_ui(ui, |ui| {
                for (k, m) in MOIS.iter().enumerate() {
                    change |= ui.selectable_value(&mut self.soleil_mois, k as u32 + 1, *m).changed();
                }
            });
            self.soleil_jour = self.soleil_jour.min(soleil::jours_du_mois(self.soleil_mois));
        });
        ui.horizontal(|ui| {
            ui.label("Heure");
            change |= ui.add(egui::Slider::new(&mut self.soleil_heure, 0.0..=23.99).step_by(0.25).custom_formatter(|v, _| heure_texte(v as f32)).custom_parser(|t| {
                let t = t.replace(['h', 'H', ':'], " ");
                let mut it = t.split_whitespace().filter_map(|x| x.parse::<f64>().ok());
                it.next().map(|h| h + it.next().unwrap_or(0.0) / 60.0)
            })).changed();
        });
        ui.horizontal(|ui| {
            for (nom, jour, mois) in [("Été", 21, 6), ("Équinoxe", 20, 3), ("Hiver", 21, 12)] {
                if ui.small_button(nom).on_hover_text(format!("{jour} {}", MOIS[mois as usize - 1])).clicked() {
                    (self.soleil_jour, self.soleil_mois) = (jour, mois);
                    change = true;
                }
            }
        });
        let (az, h) = self.position_soleil();
        let lieu = self.lieu_projet();
        ui.label(RichText::new(if h > 0.0 { format!("Azimut {az:.0}°, hauteur {h:.0}°") } else { format!("Soleil sous l'horizon (azimut {az:.0}°)") }).small().weak());
        ui.label(RichText::new(format!("Lieu : {} ({:.4}°, {:.4}°) · Début › Lieu", if lieu.nom.is_empty() { "coordonnées" } else { lieu.nom.as_str() }, lieu.latitude, lieu.longitude)).small().weak());
        change
    }

    /// Fenêtre Ciel : réglages simples du ciel physique de la vue.
    pub(super) fn fenetre_ciel(&mut self, ctx: &egui::Context) {
        if !self.ciel_ouvert {
            return;
        }
        let mut ouvert = true;
        let mut c = self.ciel.clone();
        egui::Window::new("Ciel").id(egui::Id::unique("fenetre_ciel")).open(&mut ouvert).resizable(false).collapsible(false).default_width(300.0).show(ctx, |ui| {
            ui.label(RichText::new("Ambiance").strong());
            ui.horizontal_wrapped(|ui| {
                for a in AmbianceCiel::TOUTES {
                    if ui.selectable_label(c.ambiance == a, a.nom()).clicked() {
                        a.appliquer(&mut c);
                    }
                }
            });
            ui.add_space(4.0);
            egui::Grid::new("reglages_ciel").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                let pourcent = |v: f64, _| format!("{:.0} %", v * 100.0);
                ui.label("Nuages");
                ui.add(egui::Slider::new(&mut c.nuages, 0.0..=1.0).custom_formatter(pourcent));
                ui.end_row();
                ui.label("Brume");
                ui.add(egui::Slider::new(&mut c.brume, 0.0..=1.0).custom_formatter(pourcent)).on_hover_text("Aérosols : ciel plus blanc, halo autour du soleil, lointains voilés");
                ui.end_row();
                ui.label("Soleil");
                ui.add(egui::Slider::new(&mut c.intensite_soleil, 0.0..=2.0).custom_formatter(pourcent)).on_hover_text("Intensité du soleil (100 % : valeur physique)");
                ui.end_row();
                ui.label("Ciel");
                ui.add(egui::Slider::new(&mut c.intensite_ciel, 0.0..=2.0).custom_formatter(pourcent)).on_hover_text("Intensité de la lumière du ciel (100 % : valeur physique)");
                ui.end_row();
                ui.label("Taille du soleil");
                ui.add(egui::Slider::new(&mut c.taille_soleil, 1.0..=10.0).custom_formatter(|v, _| format!("× {v:.1}"))).on_hover_text("Plus grand : ombres plus douces");
                ui.end_row();
                ui.label("Sol");
                ui.color_edit_button_srgb(&mut c.sol).on_hover_text("Couleur du sol sous l'horizon, renvoyée vers le ciel");
                ui.end_row();
                ui.label("Exposition");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut c.exposition_auto, "Auto");
                    ui.add_enabled(!c.exposition_auto, egui::Slider::new(&mut c.exposition, -4.0..=4.0).step_by(0.1).suffix(" EV"));
                });
                ui.end_row();
            });
            if ui.button("Rétablir").clicked() {
                c = ReglagesCiel::default();
            }
            ui.separator();
            self.reglages_soleil_vue(ui);
            ui.add_space(4.0);
            ui.label(RichText::new("Style Réaliste. Réglages enregistrés avec la vue.").small().weak());
        });
        if c != self.ciel {
            self.ciel = c;
        }
        if !ctx.input(|i| i.pointer.any_down()) {
            self.enregistrer_rendu_vue();
        }
        if !ouvert {
            self.ciel_ouvert = false;
        }
    }
}
