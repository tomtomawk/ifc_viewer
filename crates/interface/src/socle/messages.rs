//! Messages de l'application, affichés en un seul endroit : en haut de la vue, sous le
//! bandeau de dessin. Information et avertissement disparaissent seuls ; une erreur
//! reste jusqu'à sa fermeture (×) ou jusqu'à l'erreur suivante. Tout message, erreur
//! ou statut entre dans le journal (fenêtre « Journal des messages », commande
//! `journal` du pilote).
//!
//! Le journal relève `error`, `status` et le message de l'outil de dessin une fois par
//! image, sans changer leur rôle : le pilote lit et efface toujours `error` pour
//! connaître le résultat de sa dernière commande. Deux valeurs écrites dans la même
//! image ne laissent que la dernière, sauf par `signaler`, qui écrit directement.
use crate::*;
use std::collections::VecDeque;
use web_time::Instant;

/// Nombre d'entrées gardées par le journal.
const TAILLE_JOURNAL: usize = 200;
/// Statuts successifs plus rapprochés que ce délai (secondes) et qui ne diffèrent que
/// par leurs chiffres : une seule entrée, la dernière (curseur de l'heure du soleil,
/// nombre d'éléments sélectionnés).
const REGROUPEMENT: f32 = 1.0;

/// Texte sans ses chiffres : deux statuts de même squelette sont une même mesure qui
/// change.
fn squelette(s: &str) -> String {
    s.chars().filter(|c| !c.is_ascii_digit()).collect()
}

/// Durée d'affichage d'une information ou d'un avertissement.
const DUREE: f32 = 3.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Niveau {
    Information,
    Avertissement,
    Erreur,
}

impl Niveau {
    fn libelle(self) -> &'static str {
        match self {
            Niveau::Information => "information",
            Niveau::Avertissement => "avertissement",
            Niveau::Erreur => "erreur",
        }
    }

    fn couleur(self) -> Color32 {
        match self {
            Niveau::Information => Color32::from_rgb(40, 100, 160),
            Niveau::Avertissement => Color32::from_rgb(190, 110, 0),
            Niveau::Erreur => Color32::from_rgb(185, 35, 35),
        }
    }
}

/// Message temporaire (information ou avertissement) et son heure d'apparition.
pub(crate) type MessageTemporaire = Option<(Niveau, String, web_time::Instant)>;

struct Entree {
    niveau: Niveau,
    texte: String,
    quand: Instant,
    /// Venue de `status` : regroupable avec le statut suivant.
    statut: bool,
}

/// Journal des messages et erreur affichée.
#[derive(Default)]
pub(crate) struct Journal {
    entrees: VecDeque<Entree>,
    vu_erreur: Option<String>,
    vu_statut: String,
    vu_dessin: Option<String>,
    /// Erreur affichée dans la vue : elle reste après la réussite d'une commande
    /// suivante (qui efface `error`), jusqu'à × ou à l'erreur suivante.
    erreur: Option<String>,
    /// Erreurs arrivées depuis la dernière ouverture de la fenêtre du journal.
    non_lues: usize,
    ouvert: bool,
}

impl Journal {
    /// Journal de départ, avec les alertes du démarrage (options, gabarit).
    pub(crate) fn avec(alertes: Vec<String>) -> Self {
        let mut j = Self::default();
        for a in alertes {
            j.ajouter(Niveau::Erreur, a, false);
        }
        j
    }

    fn ajouter(&mut self, niveau: Niveau, texte: String, statut: bool) {
        self.ajouter_entree(niveau, texte, statut, true);
    }

    fn ajouter_entree(&mut self, niveau: Niveau, texte: String, statut: bool, afficher_erreur: bool) {
        if texte.trim().is_empty() {
            return;
        }
        if statut && let Some(d) = self.entrees.back_mut() && d.statut && d.quand.elapsed().as_secs_f32() < REGROUPEMENT && squelette(&d.texte) == squelette(&texte) {
            d.texte = texte;
            d.quand = Instant::now();
            return;
        }
        if niveau == Niveau::Erreur {
            if afficher_erreur { self.erreur = Some(texte.clone()); }
            self.non_lues += 1;
        }
        self.entrees.push_back(Entree { niveau, texte, quand: Instant::now(), statut });
        while self.entrees.len() > TAILLE_JOURNAL {
            self.entrees.pop_front();
        }
    }

    /// Relève les valeurs qui ont changé depuis la dernière image. Retourne le nouveau
    /// statut à montrer, s'il y en a un.
    fn relever(&mut self, error: &Option<String>, status: &str, dessin: &Option<String>) -> Option<String> {
        if error != &self.vu_erreur {
            self.vu_erreur = error.clone();
            if let Some(e) = error { self.ajouter(Niveau::Erreur, e.clone(), false); }
        }
        if dessin != &self.vu_dessin {
            self.vu_dessin = dessin.clone();
            if let Some(e) = dessin { self.ajouter(Niveau::Erreur, e.clone(), false); }
        }
        (status != self.vu_statut).then(|| {
            self.vu_statut = status.to_string();
            self.ajouter(Niveau::Information, status.to_string(), true);
            status.to_string()
        }).filter(|s| !s.trim().is_empty())
    }

    /// Texte du journal, le plus récent en premier (bouton Copier).
    fn texte(&self) -> String {
        self.entrees.iter().rev().map(|e| format!("[{}] {} — {}\n", age(e.quand), e.niveau.libelle(), e.texte)).collect()
    }

    /// Réponse de la commande `journal` du pilote : entrées, la plus ancienne d'abord.
    pub(crate) fn json(&self) -> String {
        let entrees: Vec<_> = self.entrees.iter().map(|e| serde_json::json!({
            "niveau": e.niveau.libelle(),
            "texte": e.texte,
            "age_s": (e.quand.elapsed().as_secs_f64() * 10.0).round() / 10.0,
        })).collect();
        pilote::succes(&[("journal", serde_json::Value::from(entrees).to_string()), ("erreur_affichee", self.erreur.as_deref().map_or("null".into(), pilote::texte))])
    }
}

/// Âge lisible d'une entrée : « 12 s », « 3 min », « 2 h 05 ».
fn age(quand: Instant) -> String {
    let s = quand.elapsed().as_secs();
    match s {
        0..60 => format!("il y a {s} s"),
        60..3600 => format!("il y a {} min", s / 60),
        _ => format!("il y a {} h {:02}", s / 3600, s / 60 % 60),
    }
}

/// Position commune des consignes et des messages : sous le bandeau de dessin.
pub(crate) fn origine_consigne(rect: egui::Rect, bas_barre: f32) -> egui::Pos2 {
    rect.left_top() + egui::vec2(8.0, bas_barre.max(4.0) + 8.0)
}

/// Consigne d'outil (texte d'aide permanent), dans le style des messages d'information.
pub(crate) fn consigne_vue(ui: &egui::Ui, rect: egui::Rect, bas_barre: f32, texte: &str) {
    consigne_avec_aide(ui, rect, bas_barre, texte, "");
}

/// Une ligne dans la vue ; la consigne intégrale et les raccourcis au survol.
pub(crate) fn consigne_avec_aide(ui: &egui::Ui, rect: egui::Rect, bas_barre: f32, texte: &str, aide: &str) {
    let painter = ui.painter().with_clip_rect(rect);
    let couleur = Niveau::Information.couleur();
    let largeur = (rect.width() - 24.0).clamp(1.0, 460.0);
    let mut ligne = egui::text::LayoutJob::simple(texte.into(), egui::FontId::proportional(12.0), couleur, largeur);
    ligne.wrap.max_rows = 1;
    let galley = painter.layout_job(ligne);
    let abregee = galley.elided;
    let cadre = egui::Rect::from_min_size(origine_consigne(rect, bas_barre), galley.size() + egui::vec2(8.0, 4.0));
    painter.rect_filled(cadre, 3.0, Color32::from_gray(250));
    painter.galley(cadre.min + egui::vec2(4.0, 2.0), galley, couleur);
    if abregee || !aide.is_empty() {
        ui.interact(cadre.intersect(rect), ui.make_persistent_id(("aide_consigne", texte)), Sense::hover())
            .on_hover_ui(|ui| {
                ui.label(texte);
                if !aide.is_empty() { ui.label(aide); }
            });
    }
}

/// Message au premier plan (visible au-dessus des éditeurs flottants), `decalage` pixels
/// plus bas, avec un bouton × pour une erreur. `alpha` estompe la fin d'un message
/// temporaire. Retourne si l'utilisateur l'a fermé et la hauteur du cadre.
fn afficher_a(ui: &egui::Ui, rect: egui::Rect, bas_barre: f32, niveau: Niveau, texte: &str, alpha: f32, decalage: f32) -> (bool, f32) {
    let couleur = niveau.couleur().gamma_multiply(alpha);
    let fermable = niveau == Niveau::Erreur;
    let marge_bouton = if fermable { 22.0 } else { 0.0 };
    let origine = origine_consigne(rect, bas_barre) + egui::vec2(0.0, decalage);
    egui::Area::new(ui.scope_id().with(("message_vue", niveau.libelle())))
        .order(egui::Order::Foreground)
        .fixed_pos(origine)
        .constrain_to(rect)
        .interactable(fermable)
        .show(ui.ctx(), |ui| {
            let galley = ui.painter().layout(texte.into(), egui::FontId::proportional(13.0), couleur, (rect.width() - 24.0 - marge_bouton).max(1.0));
            let taille = galley.size() + egui::vec2(8.0 + marge_bouton, 6.0);
            let (cadre, _) = ui.allocate_exact_size(egui::vec2(taille.x, taille.y.max(22.0)), Sense::hover());
            ui.painter().rect(cadre, 3.0, Color32::from_gray(250).gamma_multiply(alpha), egui::Stroke::new(1.0, couleur.gamma_multiply(0.5)), egui::StrokeKind::Inside);
            ui.painter().galley(cadre.min + egui::vec2(4.0, 3.0), galley, couleur);
            if !fermable { return (false, cadre.height()); }
            let fermer = egui::Rect::from_min_size(cadre.right_top() - egui::vec2(22.0, 0.0), egui::vec2(22.0, 22.0));
            let ferme = ui.put(fermer, egui::Button::new(RichText::new("×").size(16.0).color(couleur)).frame(false))
                .on_hover_text("Effacer le message (il reste dans le journal)").clicked();
            (ferme, cadre.height())
        }).inner
}

impl Application {
    pub(crate) fn journaliser_conflit(&mut self, texte: String) {
        self.journal.ajouter_entree(Niveau::Erreur, texte, false, false);
    }

    /// Information temporaire (confirmation d'une action). Un avertissement se pose de
    /// même avec `Niveau::Avertissement`.
    pub(crate) fn informer(&mut self, texte: impl Into<String>) {
        self.signaler(Niveau::Information, texte);
    }

    pub(crate) fn signaler(&mut self, niveau: Niveau, texte: impl Into<String>) {
        let texte = texte.into();
        if niveau == Niveau::Erreur {
            // relevée à la prochaine image ; écrite ici pour qu'une erreur suivante
            // dans la même image ne l'efface pas du journal
            self.journal.vu_erreur = Some(texte.clone());
            self.error = Some(texte.clone());
        } else {
            self.message = Some((niveau, texte.clone(), web_time::Instant::now()));
        }
        self.journal.ajouter(niveau, texte, false);
    }

    /// Une fois par image : relève erreurs et statuts, reprend les erreurs venues
    /// d'ailleurs (rendu, copie de récupération), affiche la fenêtre du journal.
    pub(crate) fn suivre_messages(&mut self, ctx: &egui::Context) {
        // tampon refusé par le GPU (modèle trop lourd pour la carte) : dit plutôt que planter
        if let Some(alerte) = self.renderer.as_mut().and_then(|r| r.alerte_memoire()) {
            self.error = Some(alerte);
        }
        if let Some(e) = echanges::enregistrement::erreur_recuperation() {
            self.journal.ajouter(Niveau::Erreur, e, false);
            ctx.request_repaint();
        }
        let avant = self.journal.entrees.len();
        if let Some(statut) = self.journal.relever(&self.error, &self.status, &self.dessin2d.message)
            && self.loading.is_none() && self.sauvegarde.is_none()
        {
            // pendant un chargement, la barre haute montre déjà le statut
            self.message = Some((Niveau::Information, statut, web_time::Instant::now()));
        }
        if self.journal.entrees.len() != avant { ctx.request_repaint(); }
        self.fenetre_journal(ctx);
    }

    /// Ferme l'erreur affichée (elle reste dans le journal).
    pub(crate) fn effacer_erreur(&mut self) {
        self.error = None;
        self.journal.erreur = None;
    }

    /// Bouton de la barre haute : ouvre le journal ; compte les erreurs non lues.
    pub(crate) fn bouton_journal(&mut self, ui: &mut egui::Ui) {
        let n = self.journal.non_lues;
        let texte = if n > 0 { RichText::new(format!("Journal ({n})")).small().color(Niveau::Erreur.couleur()) } else { RichText::new("Journal").small() };
        let aide = if n > 0 { format!("{n} erreur(s) depuis la dernière consultation : ouvrir le journal des messages") } else { "Journal des messages".into() };
        if ui.add(egui::Button::new(texte).frame(false)).on_hover_text(aide).clicked() {
            self.journal.ouvert = !self.journal.ouvert;
        }
    }

    fn fenetre_journal(&mut self, ctx: &egui::Context) {
        if !self.journal.ouvert { return; }
        self.journal.non_lues = 0;
        let mut ouvert = true;
        let mut vider = false;
        // à droite, sous le ruban : hors de l'emplacement des messages de la vue
        let ecran = ctx.content_rect();
        egui::Window::new("Journal des messages").open(&mut ouvert).default_size([520.0, 320.0])
            .default_pos([ecran.right() - 560.0, ecran.top() + 140.0]).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Copier").on_hover_text("Copier tout le journal dans le presse-papiers").clicked() { ctx.copy_text(self.journal.texte()); }
                if ui.button("Vider").clicked() { vider = true; }
                ui.label(RichText::new(format!("{} message(s), les {TAILLE_JOURNAL} derniers gardés", self.journal.entrees.len())).small().weak());
            });
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                if self.journal.entrees.is_empty() { ui.label(RichText::new("Aucun message.").weak()); }
                for e in self.journal.entrees.iter().rev() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(age(e.quand)).small().weak());
                        ui.colored_label(e.niveau.couleur(), &e.texte);
                    });
                }
            });
        });
        if vider { self.journal.entrees.clear(); }
        self.journal.ouvert = ouvert;
    }

    /// Après une modification du projet : si un terrain vient de dépasser sa profondeur
    /// nominale (fouille d'un terre-plein plus basse), message rouge, non bloquant (n° 14).
    pub(crate) fn signaler_terrains_approfondis(&mut self, avant: &modele::Projet) {
        let Some((p, _)) = &self.natif else { return };
        let approfondi = |q: &modele::Projet, t: &modele::elements::site::Terrain| {
            modele::elements::terre_plein::couches_terrain(q, t).is_ok_and(|c| c.effective > c.nominale + 0.5)
        };
        let nouveau = p.terrains.iter().any(|t| approfondi(p, t) && !avant.terrains.iter().find(|x| x.id == t.id).is_some_and(|x| approfondi(avant, x)));
        if nouveau {
            self.error = Some("Le type de terrain a été modifié".into());
        }
    }

    /// Un message occupe l'emplacement des consignes.
    pub(crate) fn message_visible(&self) -> bool {
        self.message_conflits().is_some() || self.journal.erreur.is_some() || self.dessin2d.message.is_some() || self.message.as_ref().is_some_and(|(_, _, debut)| debut.elapsed().as_secs_f32() < DUREE)
    }

    /// Affiche l'erreur courante, puis en dessous le message temporaire.
    pub(crate) fn afficher_messages(&mut self, ui: &egui::Ui, rect: egui::Rect, bas_barre: f32) {
        self.suivre_messages(ui.ctx());
        let mut decalage = 0.0;
        if self.message_conflits().is_some() {
            let couleur = Niveau::Erreur.couleur();
            let bandeau = egui::Area::new(ui.scope_id().with("conflit_vue"))
                .order(egui::Order::Foreground)
                .fixed_pos(origine_consigne(rect, bas_barre))
                .constrain_to(rect)
                .show(ui.ctx(), |ui| {
                    ui.set_max_width((rect.width() - 16.0).max(1.0));
                    egui::Frame::new()
                        .fill(Color32::from_gray(250))
                        .stroke(egui::Stroke::new(1.0, couleur.gamma_multiply(0.5)))
                        .corner_radius(3.0)
                        .inner_margin(4.0)
                        .show(ui, |ui| self.actions_conflit_vue(ui, couleur));
                });
            decalage = bandeau.response.rect.height() + 4.0;
        }
        if let Some(texte) = self.journal.erreur.clone() {
            let (ferme, h) = afficher_a(ui, rect, bas_barre, Niveau::Erreur, &texte, 1.0, decalage);
            if ferme {
                self.journal.erreur = None;
                if self.error.as_deref() == Some(texte.as_str()) { self.error = None; }
            }
            decalage += h + 4.0;
        } else if let Some(texte) = self.dessin2d.message.clone() {
            // Erreur de l'outil de dessin en cours : effacée par l'outil ou par ×.
            let (ferme, h) = afficher_a(ui, rect, bas_barre, Niveau::Erreur, &texte, 1.0, decalage);
            if ferme { self.dessin2d.message = None; }
            decalage += h + 4.0;
        }
        let Some((niveau, texte, debut)) = &self.message else { return };
        let age = debut.elapsed().as_secs_f32();
        if age >= DUREE {
            self.message = None;
            return;
        }
        afficher_a(ui, rect, bas_barre, *niveau, texte, ((DUREE - age) / 0.5).min(1.0), decalage);
        ui.ctx().request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une erreur reste affichée quand `error` est effacé par la commande suivante ; le
    /// journal garde chaque erreur et regroupe les statuts rapprochés.
    #[test]
    fn journal_garde_les_erreurs_et_regroupe_les_statuts() {
        let mut j = Journal::default();
        assert_eq!(j.relever(&Some("A".into()), "", &None), None);
        assert_eq!(j.erreur.as_deref(), Some("A"));
        // la commande suivante réussit : `error` effacé, l'erreur reste affichée
        j.relever(&None, "", &None);
        assert_eq!(j.erreur.as_deref(), Some("A"));
        j.relever(&Some("B".into()), "", &None);
        assert_eq!(j.erreur.as_deref(), Some("B"));
        // même valeur relevée deux fois : une seule entrée
        j.relever(&Some("B".into()), "", &None);
        assert_eq!(j.entrees.iter().filter(|e| e.niveau == Niveau::Erreur).count(), 2);
        assert_eq!(j.non_lues, 2);
        // statuts rapprochés : une entrée, la dernière
        assert_eq!(j.relever(&Some("B".into()), "Soleil : 10 h", &None).as_deref(), Some("Soleil : 10 h"));
        j.relever(&Some("B".into()), "Soleil : 11 h", &None);
        assert_eq!(j.entrees.back().map(|e| e.texte.as_str()), Some("Soleil : 11 h"));
        assert_eq!(j.entrees.len(), 3);
        // statuts rapprochés mais différents : deux entrées
        j.relever(&Some("B".into()), "Chargement de a.json…", &None);
        j.relever(&Some("B".into()), "Maison — projet natif v0", &None);
        assert_eq!(j.entrees.len(), 5);
        // erreur de l'outil de dessin
        j.relever(&Some("B".into()), "Maison — projet natif v0", &Some("Tracé invalide".into()));
        assert_eq!(j.entrees.back().map(|e| e.niveau), Some(Niveau::Erreur));
        assert!(j.texte().starts_with("[il y a 0 s] erreur — Tracé invalide"));
        let json: serde_json::Value = serde_json::from_str(&j.json()).unwrap();
        assert_eq!(json["journal"].as_array().map(|a| a.len()), Some(6));
        assert_eq!(json["erreur_affichee"], "Tracé invalide");
    }

    #[test]
    fn journal_borne() {
        let mut j = Journal::avec(vec!["options illisibles".into()]);
        assert_eq!(j.erreur.as_deref(), Some("options illisibles"));
        for i in 0..TAILLE_JOURNAL + 50 { j.ajouter(Niveau::Information, format!("{i}"), false); }
        assert_eq!(j.entrees.len(), TAILLE_JOURNAL);
        assert_eq!(j.entrees.back().map(|e| e.texte.clone()), Some(format!("{}", TAILLE_JOURNAL + 49)));
        j.ajouter(Niveau::Information, "  ".into(), false);
        assert_eq!(j.entrees.len(), TAILLE_JOURNAL);
    }

    #[test]
    fn niveaux_distincts() {
        let couleurs = [Niveau::Information, Niveau::Avertissement, Niveau::Erreur].map(Niveau::couleur);
        assert!(couleurs[0] != couleurs[1] && couleurs[1] != couleurs[2] && couleurs[0] != couleurs[2]);
        let r = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(100.0, 100.0));
        assert_eq!(origine_consigne(r, 0.0), egui::pos2(18.0, 32.0));
    }
}
