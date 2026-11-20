//! Outil Texte du projet : saisie dans la vue. Un clic place le point d'insertion et
//! ouvre un champ à cet endroit, à la taille du texte imprimé ; Entrée crée le texte
//! (une transaction), Maj+Entrée passe à la ligne, Échap ou le clic droit abandonnent ce
//! texte. L'outil reste actif pour le texte suivant ; un clic ailleurs valide le texte en
//! cours et en commence un autre.
use crate::*;
use modele::documentation::{Annotation, AnnotationForme};

/// Texte en cours de saisie dans la vue.
pub(crate) struct TexteSaisi {
    pub vue: String,
    /// Point d'insertion dans le repère de la vue (mm).
    pub point: [f64; 2],
    pub contenu: String,
    /// Le champ prend le focus à sa première image.
    pub focus: bool,
}

impl Application {
    /// Outil Texte actif sur une vue native (onglet Annoter).
    pub(crate) fn texte_annotation_actif(&self) -> bool {
        self.onglet_ruban == OngletRuban::Annoter && self.natif.is_some() && self.loading.is_none()
            && self.ruban_edition.outil() == Some(crate::zones::ruban::Outil::Texte) && self.vue_native_id().is_some()
    }

    /// Clic de l'outil Texte : le texte en cours est validé, puis un nouveau champ s'ouvre
    /// au point accroché (ou libre) sous le clic.
    pub(crate) fn placer_texte_vue(&mut self, rect: egui::Rect, q: egui::Pos2) -> Result<(), String> {
        let vue = self.vue_native_id().ok_or("Ouvrez un plan ou une coupe native pour annoter.")?;
        self.valider_texte_saisi()?;
        // point accroché au survol (accrochages de la vue), sinon point libre sous le clic
        let accroche = self.dessin2d.point.take().map(|(p, _)| p.to_array());
        let point = accroche.or_else(|| self.point_annotation(rect, q)).ok_or("Le plan de la vue est vu par la tranche : ouvrir un plan, une coupe ou une élévation.")?;
        // Créer à l'identique : le contenu du modèle ; sinon un champ vide
        let contenu = match self.bim.annotation_identique.as_ref().map(|a| &a.forme) {
            Some(AnnotationForme::Texte { texte, .. }) => texte.clone(),
            _ => String::new(),
        };
        self.bim.texte_saisi = Some(TexteSaisi { vue, point, contenu, focus: true });
        Ok(())
    }

    /// Hauteur imprimée (mm de papier) des textes créés.
    fn hauteur_texte_creation(&self) -> f64 {
        self.bim.annotation_identique.as_ref()
            .filter(|a| matches!(a.forme, AnnotationForme::Texte { .. }))
            .map_or_else(|| self.ruban_edition.nombre_option(crate::zones::ruban::Outil::Texte, 1), |a| a.hauteur)
    }

    /// Crée le texte en cours de saisie (s'il n'est pas vide). Une seule étape d'annulation.
    pub(crate) fn valider_texte_saisi(&mut self) -> Result<(), String> {
        let Some(t) = self.bim.texte_saisi.take() else { return Ok(()) };
        let texte = t.contenu.trim_end().to_owned();
        if texte.trim().is_empty() {
            return Ok(());
        }
        let hauteur = self.hauteur_texte_creation();
        // le champ a le clavier : sans ceci, `appliquer_projet` attendrait la fin de la
        // « saisie » et Échap sur le texte suivant annulerait celui-ci
        let saisie = std::mem::replace(&mut self.saisie_active, false);
        let r = self.executer_bim(commandes::Commande::Annoter {
            annotation: Annotation { id: String::new(), vue: t.vue, hauteur, forme: AnnotationForme::Texte { point: t.point, texte } },
        });
        self.saisie_active = saisie;
        r?;
        self.status = "Texte créé — cliquer pour le suivant ; Ctrl+Z pour annuler.".into();
        Ok(())
    }

    /// Survol et saisie de l'outil Texte : consigne, point accroché, champ de saisie au
    /// point d'insertion.
    pub(crate) fn apercu_texte_vue(&mut self, ui: &egui::Ui, rect: egui::Rect, survol: Option<egui::Pos2>) {
        // Échap pendant la frappe (consommé par `valider_saisie`) : ce texte est abandonné
        if self.echap_saisie || self.bim.texte_saisi.as_ref().is_some_and(|t| Some(&t.vue) != self.vue_native_id().as_ref()) {
            self.bim.texte_saisi = None;
        }
        let consigne = if self.bim.texte_saisi.is_some() {
            "Texte : taper le contenu · Entrée : valider · Maj+Entrée : nouvelle ligne · Échap : abandonner ce texte"
        } else {
            "Texte : cliquer le point d'insertion"
        };
        if !self.message_visible() {
            crate::socle::messages::consigne_vue(ui, rect, self.dessin2d.bas_barre, consigne);
        }
        let (maj, temps) = ui.input(|i| (i.modifiers.shift, i.time));
        self.dessin2d.point = survol.and_then(|q| self.depuis_ecran(rect, q)).map(|c| self.point_dessin_depuis(rect, c, maj, temps, None));
        if let Some((point, accroche)) = self.dessin2d.point
            && let Some(e) = self.vers_ecran(rect, point)
        {
            crate::saisie::moteur_accrochage::peindre_marqueur(&ui.painter().with_clip_rect(rect), e, accroche);
        }
        self.champ_texte_saisi(ui, rect);
    }

    /// Champ de saisie au point d'insertion, à la taille du texte dans la vue.
    fn champ_texte_saisi(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        let hauteur = self.hauteur_texte_creation();
        let echelle = self.natif.as_ref().and_then(|(p, _)| {
            let id = &self.bim.texte_saisi.as_ref()?.vue;
            p.vues.iter().find(|v| &v.id == id).map(|v| v.echelle)
        }).unwrap_or(100.0);
        let Some(t) = &self.bim.texte_saisi else { return };
        let point = glam::DVec2::from_array(t.point);
        let Some(e) = self.vers_ecran(rect, point) else { return };
        // hauteur des capitales à l'écran : hauteur imprimée × échelle de la vue ; la
        // police egui se règle en cadratin (capitales ≈ 0,72 cadratin)
        let capitales = self.vers_ecran(rect, point + glam::DVec2::Y * hauteur * echelle).map_or(10.0, |h| (h - e).length());
        let taille = (capitales / 0.72).clamp(11.0, 64.0);
        let Some(t) = self.bim.texte_saisi.as_mut() else { return };
        // Entrée seule valide ; Maj+Entrée reste au champ (nouvelle ligne). `consume_key`
        // ignore Maj : le test est fait à part.
        let entree = ui.input_mut(|i| !i.modifiers.shift && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        let couleur = Color32::from_rgb(0, 90, 180);
        egui::Area::new(egui::Id::unique("texte_saisi_vue"))
            .pivot(egui::Align2::LEFT_BOTTOM)
            .fixed_pos(e)
            .constrain_to(rect)
            .show(ui.ctx(), |ui| {
                egui::Frame::NONE.fill(Color32::from_white_alpha(230)).stroke(egui::Stroke::new(1.0, couleur)).inner_margin(egui::Margin::symmetric(2, 0)).show(ui, |ui| {
                    let lignes = t.contenu.lines().count().max(1);
                    // largeur de la ligne la plus longue, plus une marge pour la frappe
                    let police = egui::FontId::proportional(taille);
                    let largeur = t.contenu.lines().chain(std::iter::once("Texte"))
                        .map(|l| crate::zones::ruban::selecteur::largeur_texte(ui, l, taille))
                        .fold(0.0f32, f32::max) + taille;
                    let champ = egui::TextEdit::multiline(&mut t.contenu)
                        .font(police)
                        .text_color(Color32::from_gray(30))
                        .frame(egui::Frame::NONE)
                        .desired_rows(lignes)
                        .desired_width(largeur.min(rect.width() * 0.8))
                        .hint_text("Texte");
                    ui.set_min_width(largeur.min(rect.width() * 0.8));
                    let r = ui.add(champ);
                    if t.focus {
                        r.request_focus();
                        t.focus = false;
                    }
                });
            });
        if entree && let Err(e) = self.valider_texte_saisi() {
            self.error = Some(e);
        }
    }
}
