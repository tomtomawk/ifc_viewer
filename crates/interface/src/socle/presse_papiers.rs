//! Presse-papiers des projets natifs et collage sur un niveau du projet courant.
use crate::*;

const PREFIXE: &str = "BIMedit sélection\n";

pub(crate) struct CopiePresse {
    document: usize,
    ids: Vec<String>,
    fragment: Result<modele::Projet, String>,
}

impl Application {
    pub(crate) fn memoriser_copie_niveau(&mut self, ids: Vec<String>) -> bool {
        if ids.is_empty() {
            self.status = "Sélectionnez les éléments à copier.".into();
            return false;
        }
        let Some((source, _)) = &self.natif else {
            return false;
        };
        self.copie_presse = Some(CopiePresse {
            document: self.document_actif,
            fragment: commandes::copier_selection(source, &ids),
            ids,
        });
        self.status = "Sélection copiée. Ctrl+V pour coller.".into();
        true
    }

    pub(crate) fn copier_presse(&mut self, ctx: &egui::Context) {
        if !self.memoriser_copie_niveau(self.selection_bim()) {
            return;
        }
        if let Some(copie) = &self.copie_presse {
            match &copie.fragment {
                Ok(p) => match serde_json::to_string(p) {
                    Ok(json) => ctx.copy_text(format!("{PREFIXE}{json}")),
                    Err(e) => self.error = Some(e.to_string()),
                },
                Err(e) => self.status = e.clone(),
            }
        }
    }

    pub(crate) fn ouvrir_coller_niveau(&mut self) {
        let Some(copie) = &self.copie_presse else {
            self.status = "Copiez d’abord une sélection.".into();
            return;
        };
        if copie.document == self.document_actif {
            self.copie_niveau = Some(String::new());
            return;
        }
        match copie.fragment.clone() {
            Ok(fragment) => self.coller_fragment(fragment),
            Err(e) => self.error = Some(e),
        }
    }

    /// Les événements Copier/Coller de winit sont distincts des événements clavier.
    /// Le raccourci personnalisé peut continuer à utiliser la copie interne.
    pub(crate) fn coller_presse(&mut self, texte: &str) {
        let Some(json) = texte.strip_prefix(PREFIXE) else {
            return;
        };
        match serde_json::from_str::<modele::Projet>(json) {
            Ok(fragment) => {
                let meme_copie = self
                    .copie_presse
                    .as_ref()
                    .and_then(|c| c.fragment.as_ref().ok())
                    .is_some_and(|locale| locale.identite == fragment.identite);
                if meme_copie {
                    self.ouvrir_coller_niveau();
                } else {
                    self.coller_fragment(fragment);
                }
            }
            Err(e) => self.error = Some(format!("Sélection BIMedit illisible : {e}")),
        }
    }

    pub(crate) fn texte_est_selection_bim(texte: &str) -> bool {
        texte.starts_with(PREFIXE)
    }

    fn coller_fragment(&mut self, fragment: modele::Projet) {
        let Some((projet, _)) = &self.natif else {
            self.status = "Ouvrez un projet natif pour coller la sélection.".into();
            return;
        };
        let vue = self.vue_native_id();
        let resultat =
            commandes::coller_selection(projet, fragment, vue.as_deref()).and_then(|(p, ids)| {
                self.appliquer_projet(p)?;
                self.select(None, false);
                self.bim.selection = ids;
                self.status = format!(
                    "{} élément(s) collé(s). Ctrl+Z pour annuler.",
                    self.bim.selection.len()
                );
                Ok(())
            });
        if let Err(e) = resultat {
            self.error = Some(e);
        }
    }

    pub(crate) fn fenetre_coller_niveau(&mut self, ctx: &egui::Context) {
        let Some(mut niveau) = self.copie_niveau.take() else {
            return;
        };
        let Some(copie) = &self.copie_presse else {
            return;
        };
        if copie.document != self.document_actif {
            return;
        }
        let ids = copie.ids.clone();
        let Some((p, _)) = &self.natif else { return };
        let niveaux: Vec<_> = p
            .niveaux
            .iter()
            .map(|n| (n.id.clone(), n.nom.clone(), n.altitude))
            .collect();
        let mut ouvert = true;
        let mut valider = false;
        egui::Window::new("Coller sur un niveau")
            .collapsible(false)
            .resizable(false)
            .open(&mut ouvert)
            .show(ctx, |ui| {
                ui.label(format!(
                    "{} élément(s) sélectionné(s) · position en plan conservée",
                    ids.len()
                ));
                egui::ComboBox::from_id_salt("destination_copie_niveau")
                    .selected_text(
                        niveaux
                            .iter()
                            .find(|n| n.0 == niveau)
                            .map_or("Choisir un niveau", |n| n.1.as_str()),
                    )
                    .show_ui(ui, |ui| {
                        for (id, nom, z) in &niveaux {
                            ui.selectable_value(
                                &mut niveau,
                                id.clone(),
                                format!("{} · {}", nom, modele::unite::unite().afficher(*z)),
                            );
                        }
                    });
                valider = ui
                    .add_enabled(!niveau.is_empty(), egui::Button::new("Coller"))
                    .clicked();
            });
        if valider {
            match self.executer_bim(commandes::Commande::CopierSurNiveau { ids, niveau }) {
                Ok(copies) => {
                    self.status = format!(
                        "{} élément(s) collé(s) sur le niveau — Ctrl+Z pour annuler.",
                        copies.len()
                    )
                }
                Err(e) => self.error = Some(e),
            }
        } else if ouvert {
            self.copie_niveau = Some(niveau);
        }
    }
}
