//! Parcours Collaborer / Ouvrir un projet partagé et commandes du ruban Sortie.
use super::*;
use crate::collaboration::{self as collab, partage, session::Session};

impl ViewerApp {
    pub(super) fn charger_partage(
        path: &std::path::Path,
        progress: &dyn Fn(String),
    ) -> Result<(Box<Scene>, modele::Projet, PathBuf, Session), String> {
        progress("Préparation de la copie locale du projet partagé…".into());
        let (p, local, s) = partage::ouvrir(path, &partage::racine_cache())?;
        let mut scene = scene::depuis_natif(&p, &local)?;
        scene.file_name = format!("{} · partagé", p.nom);
        Ok((Box::new(scene), p, local, s))
    }
    pub(super) fn collaborer_dialogue(&mut self, ctx: &egui::Context) {
        let Some((p, _)) = &self.natif else {
            return;
        };
        if let Some(mut path) = rfd::FileDialog::new()
            .set_title("Enregistrer le projet partagé sur le serveur")
            .add_filter("Projet partagé BIMedit", &[partage::EXTENSION])
            .set_file_name(format!("{}.bimpartage", p.nom))
            .save_file()
        {
            if path.extension().is_none() {
                path.set_extension(partage::EXTENSION);
            }
            if let Err(e) = self.creer_partage(path, ctx) {
                self.error = Some(e);
            }
        }
    }
    pub(super) fn creer_partage(
        &mut self,
        path: PathBuf,
        ctx: &egui::Context,
    ) -> Result<(), String> {
        if self.loading.is_some() || self.sauvegarde.is_some() {
            return Err("Attendre la fin de l'opération en cours".into());
        }
        if self.collaboration.is_some() {
            return Err("Ce projet est déjà associé à un partage".into());
        }
        if self.saisie_active || self.avant_saisie.is_some() {
            return Err("Valider la saisie avant de partager le projet".into());
        }
        let p = self.natif.as_ref().ok_or("Projet natif requis")?.0.clone();
        let document = self.document_actif;
        let ctx = ctx.clone();
        let (tx, rx) = channel();
        std::thread::Builder::new()
            .name("creer-partage".into())
            .stack_size(64 << 20)
            .spawn(move || {
                let progress = |s: String| {
                    let _ = tx.send(LoadMsg::Progress(s));
                    ctx.request_repaint();
                };
                let resultat = (|| {
                    progress("Enregistrement du projet sur le serveur…".into());
                    partage::creer(&p, &path)?;
                    Self::charger_partage(&path, &progress)
                })();
                let _ = tx.send(LoadMsg::Partage(resultat, Some(document)));
                ctx.request_repaint();
            })
            .map_err(|e| e.to_string())?;
        self.partage_en_cours = true;
        self.loading = Some(rx);
        self.error = None;
        self.status = "Création du projet partagé…".into();
        Ok(())
    }
    // Point d'entrée conservé pour le pilote et les outils d'administration.
    pub(super) fn associer_collaboration(&mut self, central: PathBuf) -> Result<(), String> {
        let (p, path) = self.natif.as_ref().ok_or("Projet natif requis")?;
        let c = Session::ouvrir(p, path.clone(), central)?;
        let travail = c.affichage()?;
        self.collaboration = Some(c);
        self.appliquer_projet_interne(travail, true)?;
        self.collaboration
            .as_mut()
            .unwrap()
            .synchroniser(None, false)
    }
    pub(super) fn rouvrir_collaboration(&mut self) {
        self.collaboration = None;
        let Some((p, path)) = &self.natif else {
            return;
        };
        let sidecar = path.with_extension("collaboration.json");
        if !sidecar.exists() {
            return;
        }
        let resultat = (|| {
            let copie = Session::lire_copie(&sidecar)?;
            Session::ouvrir(p, path.clone(), copie.central)
        })();
        match resultat {
            Ok(c) => {
                let travail = c.affichage();
                self.collaboration = Some(c);
                if let Err(e) = travail.and_then(|p| self.appliquer_projet_interne(p, true)) {
                    self.error = Some(e);
                }
            }
            Err(e) => self.error = Some(e),
        }
    }
    pub(super) fn recevoir_collaboration(&mut self) {
        if self.saisie_active || self.avant_saisie.is_some() {
            return;
        }
        if let Some(c) = &mut self.collaboration {
            match c.recevoir() {
                Ok(Some(_)) => {
                    let affichage=c.affichage();
                    if let Err(e) = affichage.and_then(|p|self.appliquer_projet_interne(p, true)) {
                        self.error = Some(e);
                    }
                }
                Ok(None) => (),
                Err(e) => self.error = Some(e),
            }
        }
    }
    pub(super) fn cles_collaboration(&self) -> Vec<String> {
        let ids = self.selection_bim();
        self.natif
            .as_ref()
            .and_then(|(p, _)| collab::objets(p).ok())
            .into_iter()
            .flat_map(|o| {
                o.into_iter()
                    .filter(|(_, v)| {
                        v.get("id")
                            .and_then(serde_json::Value::as_str)
                            .is_some_and(|id| ids.iter().any(|i| i == id))
                    })
                    .map(|(k, _)| k)
            })
            .collect()
    }
    pub(super) fn demander_synchro(&mut self) {
        if let Some(c) = &mut self.collaboration {
            if let Err(e) = c.synchroniser(None, false) {
                self.error = Some(e);
            }
        }
    }
    pub(super) fn menu_collaboration(&mut self, ui: &mut egui::Ui) {
        let cles = self.cles_collaboration();
        let Some(c) = &self.collaboration else {
            return;
        };
        ui.label(&c.etat);
        ui.label(format!("Session : {}", c.copie.session));
        if let Some((_, local)) = &self.natif {
            ui.label(format!("Copie locale : {}", local.display()));
        }
        ui.label(format!(
            "Révision {} · {} modification(s) en attente",
            c.copie.base.revision,
            c.copie.attente.len()
        ));
        if let Some(entree) = &c.copie.partage {
            ui.label(entree.display().to_string());
        }
        let libre = !c.synchronisation_en_cours();
        let mut action = None;
        ui.separator();
        if ui
            .add_enabled(
                libre && !cles.is_empty(),
                egui::Button::new("Réserver la sélection"),
            )
            .clicked()
        {
            action = Some((Some((cles.clone(), false, false)), false));
        }
        if ui
            .add_enabled(
                libre && !cles.is_empty(),
                egui::Button::new("Libérer la sélection"),
            )
            .clicked()
        {
            action = Some((Some((cles.clone(), false, true)), false));
        }
        for k in &cles {
            if let Some(b) = c.copie.base.baux.get(k).filter(|b| b.actif) {
                ui.label(format!("Réservé par {}", b.proprietaire));
            }
        }
        ui.add_enabled_ui(libre && !cles.is_empty(), |ui| {
            ui.menu_button("Reprise explicite…", |ui| {
                ui.label("Révoque la réservation de l'ancien détenteur.");
                if ui.button("Reprendre la sélection").clicked() {
                    action = Some((Some((cles.clone(), true, false)), false));
                }
            });
        });
        if ui
            .add_enabled(libre, egui::Button::new("Créer un instantané partagé"))
            .clicked()
        {
            action = Some((None, true));
        }
        ui.collapsing("Textes supprimés récupérables", |ui| {
            for (id, t) in &c.copie.base.textes {
                if t.supprime {
                    ui.label(id);
                    if let Ok(texte) = t.contenu() {
                        ui.label(&texte);
                        if ui.button(format!("Copier {id}")).clicked() {
                            ui.ctx().copy_text(texte);
                        }
                    }
                }
            }
        });
        if let Some((reservation, instantane)) = action {
            if let Err(e) = self
                .collaboration
                .as_mut()
                .unwrap()
                .synchroniser(reservation, instantane)
            {
                self.error = Some(e);
            }
            ui.close();
        }
    }
    pub(super) fn fenetres_collaboration(&mut self, ctx: &egui::Context) {
        if self.collaboration.is_none() {
            return;
        }
        let mut abandon = None;
        let mut locale = None;
        let c = self.collaboration.as_ref().unwrap();
        let libre = !c.synchronisation_en_cours();
        if !c.copie.conflits.is_empty() {
            egui::Window::new("Conflits collaboratifs — propositions conservées").show(ctx, |ui| {
                if c.copie.apercu_conflit.is_some() {
                    ui.label("Vos modifications locales restent affichées et sauvegardées dans la copie locale. Elles ne sont pas publiées tant que le conflit n'est pas résolu.");
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (id, conflits) in &c.copie.conflits {
                        ui.label(format!("Transaction {id}"));
                        for conflit in conflits {
                            ui.label(format!("{} : {}", conflit.cle, conflit.raison));
                            ui.columns(3, |cols| {
                                for (col, (titre, v)) in cols.iter_mut().zip([
                                    ("Base", &conflit.base),
                                    ("Proposition locale", &conflit.locale),
                                    ("État accepté", &conflit.acceptee),
                                ]) {
                                    col.label(titre);
                                    col.label(
                                        v.as_ref()
                                            .map(|v| {
                                                serde_json::to_string_pretty(v).unwrap_or_default()
                                            })
                                            .unwrap_or_else(|| "∅".into()),
                                    );
                                }
                            });
                        }
                        ui.collapsing("Transaction complète récupérable", |ui| {
                            if let Some(t) = c.copie.attente.iter().find(|t| &t.id == id) {
                                ui.label(serde_json::to_string_pretty(t).unwrap_or_default());
                            }
                        });
                        if ui
                            .add_enabled(libre, egui::Button::new("Conserver l'état accepté et archiver cette proposition"))
                            .clicked()
                        {
                            abandon = Some(id.clone());
                        }
                        if ui
                            .add_enabled(libre, egui::Button::new("Choisir la proposition locale : nouvelle transaction validée"))
                            .clicked()
                        {
                            locale = Some(id.clone());
                        }
                    }
                });
            });
        }
        if let Some(id) = abandon {
            let r = self
                .collaboration
                .as_mut()
                .unwrap()
                .abandonner(&id)
                .and_then(|p| self.appliquer_projet_interne(p, true));
            if let Err(e) = r {
                self.error = Some(e);
            }
        }
        if let Some(id) = locale {
            let r = self
                .collaboration
                .as_mut()
                .unwrap()
                .resoudre_locale(&id)
                .and_then(|p| self.appliquer_projet_interne(p, true));
            if let Err(e) = r {
                self.error = Some(e);
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}
