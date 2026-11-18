//! Transactions du document BIM : historique, enregistrer sous, import natif, verrous et cadenas,
//! exécution des commandes (`executer_bim`) ; interface et pilote utilisent les mêmes transactions.

use crate::*;
use commandes::Execution;
impl ViewerApp {
    pub(crate) fn parcourir_historique(&mut self, retablir: bool) {
        self.parcourir_historique_n(retablir, 1);
    }

    pub(crate) fn parcourir_historique_n(&mut self, retablir: bool, nombre: usize) {
        // onglet Famille : un document à part, avec sa propre pile
        if self.famille_active {
            if let Some(ed) = self.famille.as_mut() {
                match ed.parcourir_historique(retablir, nombre) {
                    Ok(()) => self.status = format!("Famille : {nombre} modification(s) {}.", if retablir { "rétablie(s)" } else { "annulée(s)" }),
                    Err(e) => self.error = Some(e),
                }
                return;
            }
        }
        if self.collaboration.is_some() {
            if nombre != 1 { self.error = Some("En collaboration, annulez une transaction à la fois.".into()); return; }
            let resultat = self.collaboration.as_mut().unwrap().annuler(retablir).and_then(|p|self.appliquer_projet_interne(p,true));
            if let Err(e)=resultat {self.error=Some(e);}
            return;
        }
        self.historique.geste(false);
        self.bim.barre_editee = None;
        let resultat = (|| {
            let actuel = &self.natif.as_ref().ok_or("Aucun document natif.")?.0;
            let cible = self.historique.cible_n(actuel, retablir, nombre)?;
            // appliquer_projet reconstruit avant toute mutation ; préserver la pile.
            let mut historique = std::mem::take(&mut self.historique);
            let resultat = self
                .appliquer_projet(cible)
                .and_then(|_| historique.confirmer_n(retablir, nombre));
            self.historique = historique;
            if resultat.is_ok() {
                if let Some((projet, _)) = &self.natif {
                    let ids = projet.identifiants();
                    self.bim.selection.retain(|id| ids.contains(id));
                }
                if let Some(scene) = &self.scene {
                    self.selection_multiple
                        .retain(|id| scene.model.file.entities.contains_key(id));
                }
            }
            self.natif_modifie = self
                .natif
                .as_ref()
                .is_some_and(|(p, _)| self.historique.modifie(p));
            if !self.natif_modifie
                && let Some((_, path)) = &self.natif
            {
                echanges::enregistrement::effacer_recuperation(path);
            }
            resultat
        })();
        match resultat {
            Ok(()) => {
                self.status = if nombre == 1 {
                    if retablir {
                        "Modification rétablie.".into()
                    } else {
                        "Modification annulée.".into()
                    }
                } else {
                    format!(
                        "{nombre} modifications {}.",
                        if retablir { "rétablies" } else { "annulées" }
                    )
                }
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub(crate) fn enregistrer_sous(&mut self) {
        if self.collaboration.is_some(){self.error=Some("La copie collaborative conserve son chemin local. Pour un export autonome, utilisez les commandes d'export ; pour un autre poste, associez une copie locale distincte.".into());return;}
        let Some((p, ancien)) = &self.natif else {
            return;
        };
        let Some(mut chemin) = rfd::FileDialog::new()
            .set_title("Enregistrer sous")
            .add_filter("Modèle natif", &["json"])
            .set_file_name(ancien.file_name().unwrap_or_default().to_string_lossy())
            .save_file()
        else {
            return;
        };
        if chemin.extension().is_none() {
            chemin.set_extension("modele.json");
        }
        match echanges::enregistrement::enregistrer_natif(p, &chemin) {
            Ok(()) => {
                echanges::enregistrement::effacer_recuperation(ancien);
                let _ = self.historique.enregistrer(p);
                self.finaliser_enregistrement_sous(chemin);
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub(crate) fn finaliser_enregistrement_sous(&mut self, chemin: PathBuf) {
        echanges::enregistrement::effacer_recuperation(&chemin);
        if let Some(scene) = &mut self.scene {
            scene.file_name = chemin
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
        }
        self.status = format!("{} — enregistré", chemin.display());
        self.natif.as_mut().unwrap().1 = chemin;
        self.natif_modifie = false;
        self.error = None;
    }

    pub(crate) fn importer_dialogue(&mut self, ctx: &egui::Context) {
        let Some(source) = rfd::FileDialog::new()
            .set_title("Importer")
            .add_filter("IFC / Revit", &["ifc", "rvt", "rfa", "rte"])
            .pick_file()
        else {
            return;
        };
        // IFC et projet natif ouvert : fusion dans le projet ou nouveau projet
        if self.natif.is_some() && source.extension().is_some_and(|e| e.eq_ignore_ascii_case("ifc")) {
            let (dans, nouveau) = ("Dans le projet ouvert".to_string(), "Nouveau projet".to_string());
            match rfd::MessageDialog::new().set_title("Importer un IFC")
                .set_description("Importer les éléments du fichier dans le projet ouvert (une transaction, Ctrl+Z pour annuler) ou créer un nouveau projet natif ?")
                .set_buttons(rfd::MessageButtons::YesNoCancelCustom(dans.clone(), nouveau.clone(), "Annuler".into())).show() {
                rfd::MessageDialogResult::Custom(c) if c == dans => return self.importer_dans(ctx, source),
                rfd::MessageDialogResult::Yes => return self.importer_dans(ctx, source),
                rfd::MessageDialogResult::Custom(c) if c == nouveau => {}
                rfd::MessageDialogResult::No => {}
                _ => return,
            }
        }
        let nom = source.file_stem().unwrap_or_default().to_string_lossy();
        let Some(mut sortie) = rfd::FileDialog::new()
            .set_title("Destination du projet natif")
            .set_file_name(format!("{nom}.modele.json"))
            .add_filter("Modèle natif", &["json"])
            .save_file()
        else {
            return;
        };
        if sortie.extension().is_none() {
            sortie.set_extension("modele.json");
        }
        self.importer_natif(source, sortie, ctx);
    }

    pub(crate) fn importer_natif(&mut self, source: PathBuf, sortie: PathBuf, ctx: &egui::Context) {
        if self.loading.is_some() {
            self.error = Some("Un chargement est déjà en cours.".into());
            return;
        }
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        self.loading = Some(rx);
        std::thread::Builder::new()
            .name("import-bim".into())
            .stack_size(64 << 20)
            .spawn(move || {
                let progress = |s| {
                    let _ = tx.send(LoadMsg::Progress(s));
                    ctx.request_repaint();
                };
                let resultat = (|| {
                    let debut = std::time::Instant::now();
                    if source.canonicalize().ok() == sortie.canonicalize().ok() && sortie.exists() {
                        return Err("La source ne peut pas être remplacée.".into());
                    }
                    if !sortie
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("json"))
                    {
                        return Err("Choisissez une destination .json.".into());
                    }
                    let (p, mesures) = if is_revit_file(&source) {
                        echanges::depuis_rvt::convertir(&source, &progress)?
                    } else {
                        echanges::depuis_ifc::convertir(&source, &progress)?
                    };
                    let scene = maillage::depuis_natif(&p, &sortie)?;
                    echanges::enregistrement::enregistrer_natif(&p, &sortie)?;
                    let rapport = echanges::rapport::markdown(
                        &p,
                        &mesures,
                        debut.elapsed().as_secs_f64(),
                    );
                    echanges::enregistrement::ecrire_atomique(
                        &sortie.with_extension("rapport.md"),
                        rapport.as_bytes(),
                    )?;
                    Ok((Box::new(scene), p, sortie))
                })();
                let _ = tx.send(LoadMsg::Natif(resultat));
                ctx.request_repaint();
            })
            .expect("thread d'import");
    }
}
impl ViewerApp {
    pub(crate) fn cle_native(&self, id: Id) -> Option<String> {
        let p = &self.natif.as_ref()?.0;
        elements_natifs(p)
            .find(|(genre, cle)| modele::identifiants::id_natif(genre, cle) == id)
            .map(|(_, cle)| cle.clone())
    }
    /// Objets de la scène dont l'élément natif est verrouillé.
    pub(crate) fn objets_verrouilles(&self) -> Vec<Id> {
        let Some((p, _)) = &self.natif else { return Vec::new() };
        if p.verrous.is_empty() {
            return Vec::new();
        }
        elements_natifs(p).filter(|(_, cle)| p.verrous.contains_key(*cle)).map(|(genre, cle)| modele::identifiants::id_natif(genre, cle)).collect()
    }
    /// Bouton Verrou du ruban : verrouille la sélection, ou la déverrouille si elle l'est
    /// déjà entièrement.
    pub(crate) fn basculer_verrou_selection(&mut self) {
        let ids = self.selection_bim();
        let tous = !ids.is_empty() && self.natif.as_ref().is_some_and(|(p, _)| ids.iter().all(|id| p.verrous.contains_key(id)));
        self.verrouiller_selection(!tous);
    }
    /// Verrouille ou déverrouille la sélection (ruban Modifier, CD / DC).
    pub(crate) fn verrouiller_selection(&mut self, verrou: bool) {
        let ids = self.selection_bim();
        if ids.is_empty() {
            self.status = format!("{} : sélectionner d'abord un ou plusieurs éléments.", if verrou { "Verrouiller" } else { "Déverrouiller" });
            return;
        }
        let n = ids.len();
        match self.executer_bim(commandes::Commande::Verrouiller { ids, verrou }) {
            Ok(_) => self.status = format!("{n} élément(s) {} — Ctrl+Z pour annuler.", if verrou { "verrouillé(s)" } else { "déverrouillé(s)" }),
            Err(e) => self.error = Some(e),
        }
    }
    /// Cadenas sur les éléments verrouillés ; un clic sur un cadenas déverrouille l'élément.
    /// Rend vrai si le clic a été consommé.
    pub(crate) fn dessiner_cadenas(&mut self, ui: &egui::Ui, rect: egui::Rect, clic: Option<egui::Pos2>) -> bool {
        let objets = self.objets_verrouilles();
        if clic.is_none() {
            self.cadenas_ecran.clear();
        }
        if objets.is_empty() || self.scene.is_none() {
            return false;
        }
        let vp = self.camera.view_proj(rect.width() / rect.height().max(1.0));
        let mut cible = None;
        for id in objets {
            let sous = self.objects_under(id);
            let s = self.scene.as_ref().unwrap();
            let (mut mn, mut mx) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for o in sous.iter().filter_map(|&i| s.objects.get(i as usize)) {
                mn = mn.min(o.min);
                mx = mx.max(o.max);
            }
            if mn.x > mx.x {
                continue;
            }
            let c = vp * ((mn + mx) * 0.5).extend(1.0);
            if c.w <= 0.0 {
                continue;
            }
            let q = egui::pos2(rect.left() + (c.x / c.w + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - c.y / c.w) * 0.5 * rect.height());
            if !rect.contains(q) {
                continue;
            }
            let survol = ui.ctx().pointer_hover_pos().is_some_and(|h| h.distance(q) < 11.0);
            icone_cadenas(ui.painter(), egui::Rect::from_center_size(q, egui::vec2(18.0, 18.0)), true, survol);
            if clic.is_none() {
                if let Some(cle) = self.cle_native(id) {
                    self.cadenas_ecran.push((cle, q));
                }
            }
            if clic.is_some_and(|h| h.distance(q) < 11.0) {
                cible = self.cle_native(id);
            }
        }
        if let Some(id) = cible {
            match self.executer_bim(commandes::Commande::Verrouiller { ids: vec![id], verrou: false }) {
                Ok(_) => self.status = "Élément déverrouillé — Ctrl+Z pour annuler.".into(),
                Err(e) => self.error = Some(e),
            }
            return true;
        }
        false
    }
}

pub(crate) fn elements_natifs(p: &modele::Projet) -> impl Iterator<Item = (&'static str, &String)> {
        p.murs
            .iter()
            .map(|x| ("mur", &x.id))
            .chain(p.assemblages.iter().map(|x| ("assemblage", &x.id)))
            .chain(p.dalles.iter().map(|x| ("dalle", &x.id)))
            .chain(p.terrains.iter().map(|x| ("terrain", &x.id)))
            .chain(p.limites_propriete.iter().map(|x| ("limite_propriete", &x.id)))
            .chain(p.barres.iter().map(|x| ("barre", &x.id))).chain(p.composants.iter().map(|x| ("composant", &x.id))).chain(p.arbres.iter().map(|x| ("arbre", &x.id)))
                .chain(p.escaliers.iter().map(|x| ("escalier", &x.id)))
                .chain(p.garde_corps.iter().map(|x| ("garde_corps", &x.id)))
            .chain(p.ouvertures.iter().map(|x| ("ouverture", &x.id)))
            .chain(p.importes.iter().map(|x| ("importe", &x.id)))
            .chain(p.niveaux.iter().map(|x| ("niveau", &x.id)))
}
impl ViewerApp {
    /// Deux commandes enchaînées (la seconde reçoit les identifiants rendus par la
    /// première), appliquées en une seule transaction : un seul Ctrl+Z.
    pub(crate) fn executer_bim_suite(
        &mut self,
        premiere: commandes::Commande,
        suite: impl FnOnce(Vec<String>) -> commandes::Commande,
    ) -> Result<(), String> {
        let original = &self.natif.as_ref().ok_or("Aucun projet natif.")?.0;
        let (p, ids) = original.executer(premiere)?;
        let (p, ids) = p.executer(suite(ids))?;
        self.appliquer_projet(p)?;
        self.bim.selection = ids;
        self.selection_multiple.clear();
        self.selected = None;
        self.status = "Modification appliquée — Ctrl+Z pour annuler.".into();
        Ok(())
    }
    pub(crate) fn executer_bim(
        &mut self,
        c: commandes::Commande,
    ) -> Result<Vec<String>, String> {
        let original = &self.natif.as_ref().ok_or("Aucun projet natif.")?.0;
        let connus = original.identifiants();
        let (mut p, ids) = original.executer(c)?;
        for id in &ids {
            if !connus.contains(id)
                && !p.contextes.contains_key(id)
                && (p.barres.iter().any(|b| &b.id == id)
                    || p.composants.iter().any(|b| &b.id == id)
                    || p.escaliers.iter().any(|e| &e.id == id)
                    || p.garde_corps.iter().any(|g| &g.id == id)
                    || p.ouvertures.iter().any(|o| &o.id == id))
            {
                p.contextes.insert(
                    id.clone(),
                    modele::ContexteElement {
                        phase: self
                            .ruban_edition
                            .phase_creation
                            .clone()
                            .or_else(|| Some("Projet".into())),
                        sous_projet: self
                            .ruban_edition
                            .sous_projet_creation
                            .clone()
                            .or_else(|| Some("A".into())),
                        variante: self.ruban_edition.variante_creation.clone(),
                    },
                );
            }
        }
        self.appliquer_projet(p)?;
        self.bim.selection = ids.clone();
        self.selection_multiple.clear();
        self.bim.barre_editee = None;
        self.selected = None;
        self.status = "Modification appliquée — Ctrl+Z pour annuler.".into();
        Ok(ids)
    }
    pub(crate) fn selection_bim(&self) -> Vec<String> {
        let mut ids = self.bim.selection.clone();
        if !ids.is_empty() {
            return ids;
        }
        if let Some(id) = self.selected.and_then(|id| self.cle_native(id)) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids
    }
}
