//! Commandes du document BIM ; interface et pilote utilisent les mêmes transactions.
use super::*;
use crate::options::Dimensions;
impl ViewerApp {
    pub(super) fn parcourir_historique(&mut self, retablir: bool) {
        self.parcourir_historique_n(retablir, 1);
    }

    pub(super) fn parcourir_historique_n(&mut self, retablir: bool, nombre: usize) {
        self.historique.geste(false);
        self.bim.barre_editee = None;
        let resultat = (|| {
            let cible = self.historique.cible_n(retablir, nombre)?;
            let actuel = self
                .natif
                .as_ref()
                .ok_or("Aucun document natif.")?
                .0
                .clone();
            // appliquer_projet reconstruit avant toute mutation ; préserver la pile.
            let mut historique = std::mem::take(&mut self.historique);
            let resultat = self
                .appliquer_projet(cible)
                .and_then(|_| historique.confirmer_n(&actuel, retablir, nombre));
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
                crate::modele::enregistrement::effacer_recuperation(path);
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

    pub(super) fn enregistrer_sous(&mut self) {
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
        match crate::modele::enregistrement::enregistrer_natif(p, &chemin) {
            Ok(()) => {
                crate::modele::enregistrement::effacer_recuperation(ancien);
                let _ = self.historique.enregistrer(p);
                self.finaliser_enregistrement_sous(chemin);
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub(super) fn finaliser_enregistrement_sous(&mut self, chemin: PathBuf) {
        crate::modele::enregistrement::effacer_recuperation(&chemin);
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

    pub(super) fn importer_dialogue(&mut self, ctx: &egui::Context) {
        let Some(source) = rfd::FileDialog::new()
            .set_title("Importer en nouveau projet natif")
            .add_filter("IFC / Revit", &["ifc", "rvt", "rfa", "rte"])
            .pick_file()
        else {
            return;
        };
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

    pub(super) fn importer_natif(&mut self, source: PathBuf, sortie: PathBuf, ctx: &egui::Context) {
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
                        crate::modele::depuis_rvt::convertir(&source, &progress)?
                    } else {
                        crate::modele::depuis_ifc::convertir(&source, &progress)?
                    };
                    let scene = scene::depuis_natif(&p, &sortie)?;
                    crate::modele::enregistrement::enregistrer_natif(&p, &sortie)?;
                    let rapport = crate::modele::rapport::markdown(
                        &p,
                        &mesures,
                        debut.elapsed().as_secs_f64(),
                    );
                    crate::modele::enregistrement::ecrire_atomique(
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

pub(super) struct EtatBim {
    pub selection: Vec<String>,
    delta: [f64; 3],
    centre: [f64; 2],
    abscisse: f64,
    pub(super) barre: crate::modele::Barre,
    pub(super) depart_structure: Option<[f64; 2]>,
    barre_editee: Option<crate::modele::Barre>,
    annotation_identique: Option<crate::modele::documentation::Annotation>,
    /// Suppression demandée depuis le panneau : exécutée à l'image suivante.
    pub(super) supprimer: bool,
    pub documentation: bool,
    pub nomenclature: usize,
    pub texte: String,
    pub points: [[f64; 2]; 2],
    reference: String,
    fin: bool,
    preview_type: Option<crate::modele::Projet>,
    toiture: String,
    priorite: i32,
    pub revision: u64,
    tableau_cache: Option<(u64, String, Result<Vec<Vec<String>>, String>)>,
}
impl Default for EtatBim {
    fn default() -> Self {
        Self {
            selection: Vec::new(),
            delta: [1000.0, 0.0, 0.0],
            centre: [0.0; 2],
            abscisse: 1500.0,
            barre: crate::modele::Barre {
                id: String::new(),
                nom: "Structure".into(),
                genre: crate::modele::GenreBarre::Poteau,
                niveau: String::new(),
                debut: [0.0, 0.0, 0.0],
                fin: [0.0, 0.0, 2800.0],
                largeur: 300.0,
                hauteur: 300.0,
                rotation: 0.0,
            },
            depart_structure: None,
            barre_editee: None,
            annotation_identique: None,
            supprimer: false,
            documentation: false,
            nomenclature: 0,
            texte: "Texte".into(),
            points: [[0.0, 0.0], [3000.0, 0.0]],
            reference: String::new(),
            fin: true,
            preview_type: None,
            toiture: String::new(),
            priorite: 30,
            revision: 0,
            tableau_cache: None,
        }
    }
}
impl ViewerApp {
    pub(super) fn cle_native(&self, id: Id) -> Option<String> {
        let p = &self.natif.as_ref()?.0;
        p.murs
            .iter()
            .map(|x| ("mur", &x.id))
            .chain(p.assemblages.iter().map(|x| ("assemblage", &x.id)))
            .chain(p.dalles.iter().map(|x| ("dalle", &x.id)))
            .chain(p.barres.iter().map(|x| ("barre", &x.id)))
                .chain(p.escaliers.iter().map(|x| ("escalier", &x.id)))
                .chain(p.garde_corps.iter().map(|x| ("garde_corps", &x.id)))
            .chain(p.ouvertures.iter().map(|x| ("ouverture", &x.id)))
            .chain(p.importes.iter().map(|x| ("importe", &x.id)))
            .chain(p.niveaux.iter().map(|x| ("niveau", &x.id)))
            .find(|(genre, cle)| scene::id_natif(genre, cle) == id)
            .map(|(_, cle)| cle.clone())
    }
    pub(super) fn executer_bim(
        &mut self,
        c: crate::modele::commandes::Commande,
    ) -> Result<Vec<String>, String> {
        let original = &self.natif.as_ref().ok_or("Aucun projet natif.")?.0;
        let connus = original.identifiants();
        let (mut p, ids) = original.executer(c)?;
        for id in &ids {
            if !connus.contains(id)
                && !p.contextes.contains_key(id)
                && (p.barres.iter().any(|b| &b.id == id)
                    || p.escaliers.iter().any(|e| &e.id == id)
                    || p.garde_corps.iter().any(|g| &g.id == id)
                    || p.ouvertures.iter().any(|o| &o.id == id))
            {
                p.contextes.insert(
                    id.clone(),
                    crate::modele::ContexteElement {
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
    pub(super) fn selection_bim(&self) -> Vec<String> {
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
    fn commande_ouverture(&self, mur: String, abscisse: f64) -> crate::modele::commandes::Commande {
        use crate::ruban::Outil;
        let fenetre = self.ruban_edition.texte_option(Outil::Ouverture, 0) == "Fenêtre";
        crate::modele::commandes::Commande::Ouverture {
            mur,
            abscisse,
            type_ouverture: self.ruban_edition.ouverture_type_identique.clone(),
            genre: if fenetre {
                crate::modele::GenreOuverture::Fenetre
            } else if self.ruban_edition.texte_option(Outil::Ouverture, 0) == "Libre" {
                crate::modele::GenreOuverture::Libre
            } else {
                crate::modele::GenreOuverture::Porte
            },
            largeur: self.ruban_edition.nombre_option(Outil::Ouverture, 1),
            hauteur: self.ruban_edition.nombre_option(Outil::Ouverture, 2),
            allege: if fenetre || self.ruban_edition.texte_option(Outil::Ouverture, 0) == "Libre" {
                self.ruban_edition.nombre_option(Outil::Ouverture, 3)
            } else {
                0.0
            },
        }
    }
    pub(super) fn placer_ouverture(&mut self, id: Id, point: [f64; 2]) {
        let resultat = (|| {
            let mut id = self.cle_native(id).ok_or("Sélectionnez un mur.")?;
            let p = &self.natif.as_ref().unwrap().0;
            if let Some(a) = p
                .murs
                .iter()
                .find(|m| m.id == id)
                .and_then(|m| m.assemblage.clone())
            {
                id = a;
            }
            let (axe, _, _) = crate::modele::solides::hote(p, &id)?;
            self.executer_bim(self.commande_ouverture(id, axe.abscisse(point)))?;
            Ok(())
        })();
        if let Err(e) = resultat {
            self.error = Some(e);
        }
    }
    /// Réutilise les paramètres saisis, sans modifier le modèle tant que le placement n'est pas valide.
    pub(super) fn placer_structure(
        &mut self,
        debut: [f64; 2],
        fin: Option<[f64; 2]>,
    ) -> Result<(), String> {
        let mut b = self.bim.barre.clone();
        b.id.clear();
        b.debut[0] = debut[0];
        b.debut[1] = debut[1];
        match b.genre {
            crate::modele::GenreBarre::Poteau => {
                b.fin[0] = debut[0];
                b.fin[1] = debut[1];
            }
            crate::modele::GenreBarre::Poutre => {
                let fin = fin.ok_or("Choisissez l'arrivée de la poutre.")?;
                b.fin[0] = fin[0];
                b.fin[1] = fin[1];
                b.fin[2] = b.debut[2];
            }
        }
        self.executer_bim(crate::modele::commandes::Commande::Barre { barre: b })?;
        Ok(())
    }

    /// Supprime la sélection tout de suite (couches et ouvertures hébergées comprises),
    /// comme Revit ; Ctrl+Z la rétablit.
    pub(super) fn demander_suppression(&mut self) {
        if self.supprimer_dans_jeu() {
            return;
        }
        let ids = self.selection_bim();
        if ids.is_empty() {
            self.status = "Supprimer : sélectionner d'abord un ou plusieurs éléments.".into();
            return;
        }
        let n = self.natif.as_ref().and_then(|(p, _)| p.dependances(&ids).ok()).map_or(ids.len(), |s| s.len());
        match self.executer_bim(crate::modele::commandes::Commande::Supprimer { ids }) {
            Ok(_) => {
                self.bim.selection.clear();
                self.status = format!("{n} élément(s) supprimé(s) — Ctrl+Z pour annuler.");
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub(super) fn creer_identique(&mut self) -> Result<(), String> {
        let ids = self.selection_bim();
        if ids.len() != 1 {
            return Err("Sélectionnez un seul élément pour créer à l'identique.".into());
        }
        let (p, _) = self.natif.as_ref().ok_or("Ouvrez un projet natif.")?;
        let id = &ids[0];
        let mur = p.murs.iter().find(|m| &m.id == id).or_else(|| {
            p.assemblage(id)
                .and_then(|a| a.couches.first())
                .and_then(|id| p.murs.iter().find(|m| &m.id == id))
        });
        if let Some(mur) = mur {
            self.ruban_edition.mur_identique(p, mur)?;
        } else if let Some(d) = p.dalles.iter().find(|d| &d.id == id) {
            self.ruban_edition.surface_identique(p, d)?;
        } else if let Some(b) = p.barres.iter().find(|b| &b.id == id) {
            if let Some(c) = p.contextes.get(id) {
                self.ruban_edition.phase_creation = c.phase.clone();
                self.ruban_edition.sous_projet_creation = c.sous_projet.clone();
                self.ruban_edition.variante_creation = c.variante.clone();
            }
            self.bim.barre = b.clone();
            self.bim.barre.id.clear();
            self.ruban_edition
                .activer_structure(b.genre == crate::modele::GenreBarre::Poutre);
        } else if let Some(o) = p.ouvertures.iter().find(|o| &o.id == id) {
            if let Some(c) = p.contextes.get(id) {
                self.ruban_edition.phase_creation = c.phase.clone();
                self.ruban_edition.sous_projet_creation = c.sous_projet.clone();
                self.ruban_edition.variante_creation = c.variante.clone();
            }
            self.ruban_edition.ouverture_identique(o)?;
        } else if let Some(annotation) = p.annotations.iter().find(|a| &a.id == id) {
            use crate::modele::documentation::{AnnotationForme as F, PointCote};
            use crate::ruban::Outil;
            if self.vue_native_id().as_ref() != Some(&annotation.vue) {
                return Err("Ouvrez la vue de l'annotation pour la créer à l'identique.".into());
            }
            let outil = match &annotation.forme {
                F::Texte { point, texte } => {
                    self.bim.points[0] = *point;
                    self.bim.texte = texte.clone();
                    Outil::Texte
                }
                F::Ligne { debut, fin } => {
                    self.bim.points = [*debut, *fin];
                    Outil::Ligne
                }
                F::Polyligne { .. } => Outil::Polyligne,
                F::Spline { .. } => Outil::Spline,
                F::Cercle { .. } => Outil::Cercle,
                F::Arc { .. } => Outil::Arc,
                F::Zone { contour } => {
                    if let (Some(a), Some(b)) = (contour.first(), contour.get(2)) {
                        self.bim.points = [*a, *b];
                    }
                    Outil::Zone
                }
                F::Cote { debut, fin, .. } => {
                    if let PointCote::Fixe { point } = debut {
                        self.bim.points[0] = *point;
                    }
                    if let PointCote::Fixe { point } = fin {
                        self.bim.points[1] = *point;
                    }
                    Outil::Cote
                }
                F::Etiquette { point, .. } => {
                    self.bim.points[0] = *point;
                    Outil::Etiquette
                }
            };
            self.bim.annotation_identique = Some(annotation.clone());
            self.ruban_edition.activer_outil(outil);
            self.onglet_ruban = OngletRuban::Annoter;
            self.select(None, false);
            self.status = "Créer à l'identique : placez la nouvelle annotation.".into();
            return Ok(());
        } else if let Some(vue) = p
            .vues
            .iter()
            .find(|v| &v.id == id && matches!(v.genre, crate::modele::GenreVue::Coupe { .. }))
        {
            self.ruban_edition.coupe_identique(vue)?;
            self.onglet_ruban = OngletRuban::Annoter;
            self.select(None, false);
            self.status = "Créer à l'identique : tracez la coupe ou l'élévation.".into();
            return Ok(());
        } else if let Some(g) = p.quadrillages.iter().find(|g| &g.id == id) {
            self.bim.points = [g.debut, g.fin];
            self.ruban_edition.quadrillage_identique(&g.nom);
        } else {
            return Err("Créer à l'identique est disponible pour les murs, éléments horizontaux, poteaux, poutres, quadrillages, coupes, élévations et ouvertures natives.".into());
        }
        self.onglet_ruban = OngletRuban::Modeliser;
        self.select(None, false);
        self.status = "Créer à l'identique : placez le nouvel élément.".into();
        Ok(())
    }

    pub(super) fn contexte_selection(&self, ui: &mut egui::Ui, ids: &[String]) {
        let Some((p, _)) = &self.natif else { return };
        let mut contextes = Vec::new();
        for id in ids {
            if let Some(c) = p.contextes.get(id) {
                contextes.push((
                    c.phase.as_deref().unwrap_or("Projet").into(),
                    c.sous_projet.as_deref().unwrap_or("A").into(),
                    c.variante
                        .as_ref()
                        .and_then(|id| p.variantes.iter().find(|v| &v.id == id))
                        .map_or_else(|| "Principale".into(), |v| v.libelle()),
                ));
                continue;
            }
            let hote = p
                .ouvertures
                .iter()
                .find(|o| &o.id == id)
                .map(|o| o.mur.as_str())
                .unwrap_or(id);
            let mur = p.murs.iter().find(|m| m.id == hote).or_else(|| {
                p.assemblage(hote)
                    .and_then(|a| a.couches.first())
                    .and_then(|id| p.murs.iter().find(|m| &m.id == id))
            });
            let dalle = p.dalles.iter().find(|d| &d.id == id);
            let (phase, sous, variante) = if let Some(m) = mur {
                (&m.phase, &m.sous_projet, &m.variante)
            } else if let Some(d) = dalle {
                (&d.phase, &d.sous_projet, &d.variante)
            } else {
                contextes.push((
                    "Projet".to_string(),
                    "A".to_string(),
                    "Principale".to_string(),
                ));
                continue;
            };
            contextes.push((
                phase.as_deref().unwrap_or("Projet").to_string(),
                sous.as_deref().unwrap_or("A").to_string(),
                variante
                    .as_ref()
                    .and_then(|id| p.variantes.iter().find(|v| &v.id == id))
                    .map_or_else(|| "Principale".to_string(), |v| v.libelle()),
            ));
        }
        let valeur = |i: usize| {
            let noms: Vec<_> = contextes
                .iter()
                .map(|c| match i {
                    0 => &c.0,
                    1 => &c.1,
                    _ => &c.2,
                })
                .collect();
            if noms.iter().all(|n| *n == noms[0]) {
                noms[0].as_str()
            } else {
                "Multiple"
            }
        };
        let largeur = ((ui.available_width() - 8.0) / 3.0).max(30.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (i, nom) in ["Phase", "Sous-projet", "Variante"].iter().enumerate() {
                ui.vertical(|ui| {
                    ui.set_width(largeur);
                    ui.add(egui::Label::new(RichText::new(*nom).small()).truncate());
                    ui.add(egui::Label::new(valeur(i)).truncate())
                        .on_hover_text(valeur(i));
                });
            }
        });
    }

    pub(super) fn panneau_bim(&mut self, ui: &mut egui::Ui) -> bool {
        use crate::modele::commandes::Commande;
        use crate::ruban::Outil;
        if self.natif.is_none() {
            return false;
        }
        let outil = self.ruban_edition.outil();
        if let Some(annotation) = &self.bim.annotation_identique {
            use crate::modele::documentation::AnnotationForme as F;
            let attendu = match annotation.forme {
                F::Texte { .. } => Outil::Texte,
                F::Ligne { .. } => Outil::Ligne,
                F::Polyligne { .. } => Outil::Polyligne,
                F::Spline { .. } => Outil::Spline,
                F::Cercle { .. } => Outil::Cercle,
                F::Arc { .. } => Outil::Arc,
                F::Zone { .. } => Outil::Zone,
                F::Cote { .. } => Outil::Cote,
                F::Etiquette { .. } => Outil::Etiquette,
            };
            if self.onglet_ruban != OngletRuban::Annoter || outil != Some(attendu) {
                self.bim.annotation_identique = None;
            }
        }
        let ids = self.selection_bim();
        let mut commande = None;
        if self.onglet_ruban == OngletRuban::Modifier {
            if ids.len() == 1 {
                let p = &self.natif.as_ref().unwrap().0;
                if let Some(original) = p.vues.iter().find(|v| {
                    v.id == ids[0] && matches!(v.genre, crate::modele::GenreVue::Coupe { .. })
                }) {
                    let mut vue = original.clone();
                    ui.collapsing("Propriétés de la coupe / élévation", |ui| {
                        let mut change = ui.text_edit_singleline(&mut vue.nom).changed();
                        if let crate::modele::GenreVue::Coupe {
                            debut,
                            fin,
                            profondeur,
                            ..
                        } = &mut vue.genre
                        {
                            for (nom, point) in [("Départ", debut), ("Arrivée", fin)] {
                                ui.horizontal(|ui| {
                                    ui.label(nom);
                                    for x in point {
                                        change |= ui
                                            .add(egui::DragValue::new(x).speed(10.0).dimensions())
                                            .changed();
                                    }
                                });
                            }
                            ui.horizontal(|ui| {
                                ui.label("Profondeur");
                                change |= ui
                                    .add(
                                        egui::DragValue::new(profondeur)
                                            .speed(10.0)
                                            .range(1.0..=200000.0)
                                            .dimensions(),
                                    )
                                    .changed();
                            });
                        }
                        if change {
                            commande = Some(Commande::ModifierVue { vue });
                        }
                    });
                }
                if let Some(original) = p.quadrillages.iter().find(|g| g.id == ids[0]) {
                    let mut quadrillage = original.clone();
                    ui.collapsing("Propriétés du quadrillage", |ui| {
                        let mut change = ui.text_edit_singleline(&mut quadrillage.nom).changed();
                        for (nom, point) in [
                            ("Départ", &mut quadrillage.debut),
                            ("Arrivée", &mut quadrillage.fin),
                        ] {
                            ui.horizontal(|ui| {
                                ui.label(nom);
                                for x in point {
                                    change |= ui
                                        .add(egui::DragValue::new(x).speed(10.0).dimensions())
                                        .changed();
                                }
                            });
                        }
                        if change {
                            commande = Some(Commande::ModifierQuadrillage { quadrillage });
                        }
                    });
                }
                if let Some(original) = p.annotations.iter().find(|a| a.id == ids[0]) {
                    let mut annotation = original.clone();
                    ui.collapsing("Propriétés de l'annotation", |ui| {
                        use crate::modele::documentation::{AnnotationForme as F, PointCote};
                        let mut change = false;
                        match &mut annotation.forme {
                            F::Polyligne { points, fermee } | F::Spline { points, fermee } => {
                                ui.label(format!("{} points", points.len()));
                                change |= ui.checkbox(fermee, "Fermée").changed();
                            }
                            F::Cercle { centre, rayon } => {
                                ui.horizontal(|ui| {
                                    ui.label("Centre");
                                    for x in centre.iter_mut() {
                                        change |= ui.add(egui::DragValue::new(x).speed(10.0).dimensions()).changed();
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Rayon");
                                    change |= ui.add(egui::DragValue::new(rayon).speed(10.0).range(1.0..=f64::MAX).dimensions()).changed();
                                });
                            }
                            F::Arc { .. } => {
                                ui.label("Arc par trois points");
                            }
                            F::Texte { point, texte } => {
                                change |= ui.text_edit_singleline(texte).changed();
                                ui.horizontal(|ui| {
                                    ui.label("Position");
                                    for x in point {
                                        change |= ui
                                            .add(egui::DragValue::new(x).speed(10.0).dimensions())
                                            .changed();
                                    }
                                });
                            }
                            F::Etiquette { point, .. } => {
                                ui.horizontal(|ui| {
                                    ui.label("Position");
                                    for x in point {
                                        change |= ui
                                            .add(egui::DragValue::new(x).speed(10.0).dimensions())
                                            .changed();
                                    }
                                });
                            }
                            F::Ligne { debut, fin } => {
                                for (nom, point) in [("Départ", debut), ("Arrivée", fin)] {
                                    ui.horizontal(|ui| {
                                        ui.label(nom);
                                        for x in point {
                                            change |= ui
                                                .add(
                                                    egui::DragValue::new(x)
                                                        .speed(10.0)
                                                        .dimensions(),
                                                )
                                                .changed();
                                        }
                                    });
                                }
                            }
                            F::Cote {
                                debut,
                                fin,
                                decalage,
                            } => {
                                ui.horizontal(|ui| {
                                    ui.label("Décalage");
                                    change |= ui
                                        .add(
                                            egui::DragValue::new(decalage).speed(10.0).dimensions(),
                                        )
                                        .changed();
                                });
                                for (nom, point) in [("Départ", debut), ("Arrivée", fin)] {
                                    if let PointCote::Fixe { point } = point {
                                        ui.horizontal(|ui| {
                                            ui.label(nom);
                                            for x in point {
                                                change |= ui
                                                    .add(
                                                        egui::DragValue::new(x)
                                                            .speed(10.0)
                                                            .dimensions(),
                                                    )
                                                    .changed();
                                            }
                                        });
                                    }
                                }
                            }
                            F::Zone { contour } => {
                                for (i, point) in contour.iter_mut().enumerate() {
                                    ui.horizontal(|ui| {
                                        ui.label(format!("Sommet {}", i + 1));
                                        for x in point {
                                            change |= ui
                                                .add(
                                                    egui::DragValue::new(x)
                                                        .speed(10.0)
                                                        .dimensions(),
                                                )
                                                .changed();
                                        }
                                    });
                                }
                            }
                        }
                        if change {
                            commande = Some(Commande::Annoter { annotation });
                        }
                    });
                }
                if let Some(original) = p.barres.iter().find(|b| b.id == ids[0]) {
                    if self
                        .bim
                        .barre_editee
                        .as_ref()
                        .is_none_or(|b| b.id != original.id)
                    {
                        self.bim.barre_editee = Some(original.clone());
                    }
                    ui.collapsing("Propriétés structurelles", |ui| {
                        let b = self.bim.barre_editee.as_mut().unwrap();
                        barre_ui(ui, b, &p.niveaux);
                        if ui.button("Appliquer les propriétés").clicked() {
                            commande = Some(Commande::Barre { barre: b.clone() });
                        }
                    });
                } else {
                    self.bim.barre_editee = None;
                }
            } else {
                self.bim.barre_editee = None;
            }
            if ids.len() == 1
                && self
                    .natif
                    .as_ref()
                    .unwrap()
                    .0
                    .dalles
                    .iter()
                    .any(|d| d.id == ids[0])
            {
                ui.collapsing("Sommets du contour", |ui| {
                    let d = self
                        .natif
                        .as_ref()
                        .unwrap()
                        .0
                        .dalles
                        .iter()
                        .find(|d| d.id == ids[0])
                        .unwrap();
                    let mut contours = d.contours.clone();
                    let mut change = false;
                    for (k, c) in contours.iter_mut().enumerate() {
                        ui.push_id(k, |ui| {
                            let mut retrait = None;
                            for (i, p) in c.iter_mut().enumerate() {
                                ui.push_id(i, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(format!("{}", i + 1));
                                        for v in p {
                                            change |= ui
                                                .add(
                                                    egui::DragValue::new(v)
                                                        .speed(10.0)
                                                        .dimensions(),
                                                )
                                                .changed();
                                        }
                                        if ui.button("−").clicked() {
                                            retrait = Some(i);
                                        }
                                    });
                                });
                            }
                            if let Some(i) = retrait {
                                c.remove(i);
                                change = true;
                            }
                            if ui.button("Ajouter un sommet au dernier côté").clicked()
                                && c.len() >= 2
                            {
                                let a = c[0];
                                let b = *c.last().unwrap();
                                c.push([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]);
                                change = true;
                            }
                        });
                    }
                    if change {
                        commande = Some(Commande::Contour {
                            id: ids[0].clone(),
                            contours,
                        });
                    }
                    ui.label("Modifier le contour réinitialise les pentes par arête.");
                });
            }
        } else if self.onglet_ruban == OngletRuban::Modeliser {
            match outil {
                Some(Outil::Niveau) => {
                    ui.heading("Niveaux");
                    let p = &self.natif.as_ref().unwrap().0;
                    for n in &p.niveaux {
                        ui.push_id(&n.id, |ui| {
                            let mut nom = n.nom.clone();
                            let mut altitude = n.altitude;
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut nom);
                                ui.add(
                                    egui::DragValue::new(&mut altitude).speed(10.0).dimensions(),
                                );
                            });
                            if nom != n.nom || altitude != n.altitude {
                                commande = Some(Commande::ModifierNiveau {
                                    id: n.id.clone(),
                                    nom,
                                    altitude,
                                });
                            }
                        });
                    }
                    ui.label(RichText::new("Nom et altitude : valider par Entrée ou en cliquant ailleurs.").small().weak());
                    // nouveau niveau au-dessus du plus haut, à la hauteur du dernier étage
                    // (3 m s'il n'y en a qu'un)
                    let mut alts: Vec<f64> = p.niveaux.iter().map(|n| n.altitude).collect();
                    alts.sort_by(f64::total_cmp);
                    let etage = match alts.as_slice() {
                        [.., a, b] if b - a > 1.0 => b - a,
                        _ => 3000.0,
                    };
                    let altitude = alts.last().map_or(0.0, |h| h + etage);
                    let nom = (1..).map(|k| format!("Niveau {k}")).find(|n| !p.niveaux.iter().any(|x| &x.nom == n)).unwrap_or_default();
                    if ui.button("Ajouter un niveau").on_hover_text(format!("« {nom} » à {}", crate::options::unite().afficher(altitude))).clicked() {
                        commande = Some(Commande::Niveau { nom, altitude });
                    }
                }
                Some(Outil::Poteau | Outil::Poutre) => {
                    use crate::modele::GenreBarre;
                    let genre = if outil == Some(Outil::Poteau) {
                        GenreBarre::Poteau
                    } else {
                        GenreBarre::Poutre
                    };
                    let b = &mut self.bim.barre;
                    if b.genre != genre {
                        b.genre = genre;
                        b.fin = if genre == GenreBarre::Poteau {
                            [b.debut[0], b.debut[1], b.debut[2] + 2800.0]
                        } else {
                            [b.debut[0] + 3000.0, b.debut[1], b.debut[2]]
                        };
                    }
                    let niveaux = &self.natif.as_ref().unwrap().0.niveaux;
                    if b.niveau.is_empty() {
                        if let Some(n) = niveaux.first() {
                            b.niveau = n.id.clone();
                        }
                    }
                    ui.label(if genre == GenreBarre::Poteau { "Cliquez dans la vue pour placer le poteau." } else { "Cliquez le départ puis l'arrivée dans la vue ; Échap ou clic droit annule le départ." });
                    barre_ui(ui, b, niveaux);
                    if ui.button("Créer l'élément").clicked() {
                        self.bim.depart_structure = None;
                        let mut b = b.clone();
                        b.id.clear();
                        commande = Some(Commande::Barre { barre: b });
                    }
                }
                Some(Outil::Quadrillage) => {
                    points_ui(ui, &mut self.bim.points);
                    if ui.button("Créer le quadrillage droit").clicked() {
                        commande = Some(Commande::Quadrillage {
                            nom: self.ruban_edition.texte_option(Outil::Quadrillage, 0),
                            debut: self.bim.points[0],
                            fin: self.bim.points[1],
                        });
                    }
                }
                Some(Outil::Ouverture) => {
                    // type à placer (et éditeur de familles), en tête comme dans Revit
                    if self.ruban_edition.proprietes_ouverture(ui, self.natif.as_ref().map(|(p, _)| p)) {
                        if let Some(t) = self.ruban_edition.demande_famille.take() {
                            let genre = if self.ruban_edition.texte_option(Outil::Ouverture, 0) == "Porte" { crate::modele::GenreOuverture::Porte } else { crate::modele::GenreOuverture::Fenetre };
                            self.ouvrir_editeur_famille_type(t, genre);
                        }
                    }
                    ui.label("Cliquez sur un mur pour placer l’ouverture, ou sélectionnez son hôte et saisissez l'abscisse.");
                    ui.add(
                        egui::DragValue::new(&mut self.bim.abscisse)
                            .speed(10.0)
                            .dimensions(),
                    );
                    if ui
                        .add_enabled(ids.len() == 1, egui::Button::new("Placer"))
                        .clicked()
                    {
                        commande = Some(self.commande_ouverture(ids[0].clone(), self.bim.abscisse));
                    }
                }
                _ => return false,
            }
        } else if self.onglet_ruban == OngletRuban::Annoter
            && matches!(
                outil,
                Some(Outil::Texte | Outil::Cote | Outil::Zone | Outil::Etiquette)
            )
        {
            use crate::modele::documentation::{Annotation, AnnotationForme as F, PointCote};
            let Some(vue) = self.vue_native_id() else {
                ui.label("Ouvrez un plan ou une coupe native pour annoter.");
                return true;
            };
            points_ui(ui, &mut self.bim.points);
            if outil == Some(Outil::Texte) {
                ui.text_edit_singleline(&mut self.bim.texte);
            }
            if ui.button("Utiliser le dernier point cliqué").clicked() {
                if let Some(pt) = self.point_clique {
                    let p = &self.natif.as_ref().unwrap().0;
                    let v = p.vues.iter().find(|v| v.id == vue).unwrap();
                    let (o, u, h) = crate::modele::documentation::repere(p, v);
                    let d = pt * 1000.0 - o;
                    self.bim.points[0] = [d.dot(u), d.dot(h)];
                }
            }
            if ui.button("Ajouter à la vue").clicked() {
                let [a, b] = self.bim.points;
                let forme = match outil.unwrap() {
                    Outil::Texte => F::Texte {
                        point: a,
                        texte: self.bim.texte.clone(),
                    },
                    Outil::Cote => {
                        let lie = ids
                            .first()
                            .filter(|id| self.natif.as_ref().unwrap().0.axe_element(id).is_some());
                        let debut = lie.map_or(PointCote::Fixe { point: a }, |id| PointCote::Mur {
                            id: id.clone(),
                            fin: false,
                        });
                        let fin = lie.map_or(PointCote::Fixe { point: b }, |id| PointCote::Mur {
                            id: id.clone(),
                            fin: true,
                        });
                        F::Cote {
                            debut,
                            fin,
                            decalage: 800.0,
                        }
                    }
                    Outil::Etiquette => F::Etiquette {
                        element: ids.first().cloned().unwrap_or_default(),
                        point: a,
                    },
                    Outil::Zone => F::Zone {
                        contour: vec![a, [b[0], a[1]], b, [a[0], b[1]]],
                    },
                    _ => F::Ligne { debut: a, fin: b },
                };
                let mut forme = forme;
                let mut hauteur = 2.5;
                if let Some(modele) = &self.bim.annotation_identique {
                    hauteur = modele.hauteur;
                    match &modele.forme {
                        F::Etiquette { element, .. } => {
                            forme = F::Etiquette {
                                element: element.clone(),
                                point: a,
                            }
                        }
                        F::Cote {
                            debut,
                            fin,
                            decalage,
                        } => {
                            forme = F::Cote {
                                debut: if matches!(debut, PointCote::Fixe { .. }) {
                                    PointCote::Fixe { point: a }
                                } else {
                                    debut.clone()
                                },
                                fin: if matches!(fin, PointCote::Fixe { .. }) {
                                    PointCote::Fixe { point: b }
                                } else {
                                    fin.clone()
                                },
                                decalage: *decalage,
                            };
                        }
                        F::Zone { contour } if !contour.is_empty() => {
                            let origine = contour[0];
                            forme = F::Zone {
                                contour: contour
                                    .iter()
                                    .map(|p| [p[0] - origine[0] + a[0], p[1] - origine[1] + a[1]])
                                    .collect(),
                            };
                        }
                        _ => {}
                    }
                }
                commande = Some(Commande::Annoter {
                    annotation: Annotation {
                        id: String::new(),
                        vue,
                        hauteur,
                        forme,
                    },
                });
            }
            ui.label("Une cote créée avec un mur sélectionné suit ses extrémités.");
        } else {
            return false;
        }
        if let Some(c) = commande {
            if let Err(e) = self.executer_bim(c) {
                self.error = Some(e);
            }
        }
        if let Some(e) = &self.error {
            ui.colored_label(Color32::DARK_RED, e);
        }
        self.onglet_ruban != OngletRuban::Modifier
    }
}
impl ViewerApp {
    /// Barre d'options de Modifier, en haut de la vue : réglages de l'outil actif
    /// (Déplacer, Copier, Tourner, Aligner, Ajuster) et commandes propres à la sélection
    /// (jonctions, mise à jour depuis le type, trémie). Propriétés ne garde que les
    /// informations de la sélection.
    pub(super) fn options_modifier(&mut self, ui: &mut egui::Ui) {
        use crate::modele::commandes::Commande;
        use crate::ruban::Outil;
        let outil = self.ruban_edition.outil();
        let ids = self.selection_bim();
        let Some((p, _)) = &self.natif else { return };
        let murs = ids.iter().any(|id| p.murs.iter().any(|m| &m.id == id) || p.assemblage(id).is_some());
        let dalle = ids.len() == 1 && p.dalles.iter().any(|d| d.id == ids[0]);
        let fenetre = (ids.len() == 1).then(|| p.ouvertures.iter().find(|o| o.id == ids[0] && o.genre != crate::modele::GenreOuverture::Libre)).flatten().map(|o| o.id.clone());
        let mut editer_famille = false;
        let escalier = (ids.len() == 1).then(|| p.escaliers.iter().find(|e| e.id == ids[0])).flatten().map(|e| e.id.clone());
        let mut garde_corps: Option<Vec<usize>> = None;
        // couche d'assemblage : la sortir ou prendre l'assemblage ; mur indépendant : le
        // rattacher à un assemblage du même niveau
        let mur = (ids.len() == 1).then(|| p.murs.iter().find(|m| m.id == ids[0])).flatten().cloned();
        let assemblages: Vec<(String, String)> = mur.as_ref().filter(|m| m.assemblage.is_none()).map(|m| {
            p.assemblages.iter().filter(|a| p.couches_assemblage(&a.id).first().is_some_and(|c| c.niveau == m.niveau)).map(|a| (a.id.clone(), a.nom.clone())).collect()
        }).unwrap_or_default();
        let mut dissocier = false;
        let mut associer: Option<String> = None;
        let mut selection = None;
        let mut commande = None;
        ui.horizontal(|ui| {
            if escalier.is_some() {
                ui.label("Garde-corps");
                for (nom, cotes) in [("Deux côtés", vec![0, 1]), ("Gauche", vec![0]), ("Droite", vec![1])] {
                    if ui.button(nom).on_hover_text("Garde-corps qui suivent les marches (gauche / droite du sens de montée), réglages de l'outil Garde-corps").clicked() {
                        garde_corps = Some(cotes);
                    }
                }
            }
            if fenetre.is_some() && ui.button("Éditer la famille…").on_hover_text("Éditeur de familles : plans, contraintes, volumes, vides et symboles ; enregistré dans le type").clicked() {
                editer_famille = true;
            }
            if let Some(m) = &mur {
                if let Some(a) = &m.assemblage {
                    if ui.button("Dissocier").on_hover_text("La couche devient un mur indépendant (elle garde sa place et ses réglages)").clicked() {
                        dissocier = true;
                    }
                    if ui.button("Assemblage").on_hover_text("Sélectionner l'assemblage entier").clicked() {
                        selection = Some(crate::scene::id_natif("assemblage", a));
                    }
                } else if !assemblages.is_empty() {
                    egui::ComboBox::from_id_salt("associer_mur").width(170.0).selected_text("Associer à un assemblage…").show_ui(ui, |ui| {
                        for (id, nom) in &assemblages {
                            if ui.selectable_label(false, nom).clicked() {
                                associer = Some(id.clone());
                            }
                        }
                    });
                }
            }
            match outil {
                Some(Outil::Deplacer | Outil::Copier) => {
                    for (nom, v) in ["X", "Y", "Z"].into_iter().zip(&mut self.bim.delta) {
                        ui.horizontal(|ui| {
                            ui.label(nom);
                            ui.add(egui::DragValue::new(v).speed(10.0).dimensions());
                        });
                    }
                    if ui
                        .add_enabled(!ids.is_empty(), egui::Button::new("Appliquer"))
                        .clicked()
                    {
                        commande = Some(if outil == Some(Outil::Copier) {
                            Commande::Copier {
                                ids: ids.clone(),
                                delta: self.bim.delta,
                            }
                        } else {
                            Commande::Deplacer {
                                ids: ids.clone(),
                                delta: self.bim.delta,
                            }
                        });
                    }
                }
                Some(Outil::Tourner) => {
                    ui.label("Centre de rotation dans le plan du projet");
                    for v in &mut self.bim.centre {
                        ui.add(egui::DragValue::new(v).speed(10.0).dimensions());
                    }
                    if ui
                        .add_enabled(!ids.is_empty(), egui::Button::new("Tourner"))
                        .clicked()
                    {
                        commande = Some(Commande::Tourner {
                            ids: ids.clone(),
                            centre: self.bim.centre,
                            angle: self.ruban_edition.nombre_option(Outil::Tourner, 0),
                        });
                    }
                }
                Some(Outil::Aligner | Outil::Ajuster) => {
                    ui.label("Référence : axe d'un mur droit");
                    egui::ComboBox::from_id_salt("reference_bim")
                        .selected_text(&self.bim.reference)
                        .show_ui(ui, |ui| {
                            let p = &self.natif.as_ref().unwrap().0;
                            for (id, nom) in p
                                .murs
                                .iter()
                                .filter(|m| m.assemblage.is_none())
                                .map(|m| (&m.id, &m.nom))
                                .chain(p.assemblages.iter().map(|m| (&m.id, &m.nom)))
                            {
                                ui.selectable_value(&mut self.bim.reference, id.clone(), nom);
                            }
                        });
                    if outil == Some(Outil::Ajuster) {
                        ui.checkbox(&mut self.bim.fin, "Ajuster la fin (sinon le début)");
                    }
                    if ui
                        .add_enabled(
                            !ids.is_empty() && !self.bim.reference.is_empty(),
                            egui::Button::new("Appliquer à l'axe"),
                        )
                        .clicked()
                    {
                        commande = Some(if outil == Some(Outil::Aligner) {
                            Commande::Aligner {
                                ids: ids.clone(),
                                reference: self.bim.reference.clone(),
                            }
                        } else {
                            Commande::Ajuster {
                                id: ids[0].clone(),
                                reference: self.bim.reference.clone(),
                                fin: self.bim.fin,
                            }
                        });
                    }
                }
                _ => {}
            }
            if murs {
                ui.menu_button("Jonctions et haut des murs", |ui| {
                    egui::ComboBox::from_id_salt("toit_attache")
                        .selected_text(if self.bim.toiture.is_empty() {
                            "Toiture"
                        } else {
                            &self.bim.toiture
                        })
                        .show_ui(ui, |ui| {
                            for d in self
                                .natif
                                .as_ref()
                                .unwrap()
                                .0
                                .dalles
                                .iter()
                                .filter(|d| d.sorte == crate::modele::SorteDalle::Toiture)
                            {
                                ui.selectable_value(&mut self.bim.toiture, d.id.clone(), &d.nom);
                            }
                        });
                    if ui
                        .add_enabled(
                            !self.bim.toiture.is_empty(),
                            egui::Button::new("Attacher à la sous-face"),
                        )
                        .clicked()
                    {
                        commande = Some(Commande::Attacher {
                            ids: ids.clone(),
                            toiture: Some(self.bim.toiture.clone()),
                        });
                    }
                    if ui.button("Détacher").clicked() {
                        commande = Some(Commande::Attacher {
                            ids: ids.clone(),
                            toiture: None,
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label("Priorité de jonction");
                        ui.add(egui::DragValue::new(&mut self.bim.priorite));
                    });
                    if ui
                        .add_enabled(ids.len() == 1, egui::Button::new("Appliquer à la couche"))
                        .clicked()
                    {
                        commande = Some(Commande::PrioriteJonction {
                            id: ids[0].clone(),
                            priorite: self.bim.priorite,
                        });
                    }
                    ui.label("La priorité la plus élevée conserve son volume au croisement.");
                });
            }
            if ids.len() == 1 && self.natif.as_ref().unwrap().0.assemblage(&ids[0]).is_some() {
                ui.menu_button("Mise à jour depuis le type", |ui| {
                if ui
                    .button("Préparer la mise à jour depuis le type")
                    .clicked()
                {
                    match self
                        .natif
                        .as_ref()
                        .unwrap()
                        .0
                        .executer(Commande::ActualiserType {
                            assemblage: ids[0].clone(),
                        }) {
                        Ok((p, _)) => self.bim.preview_type = Some(p),
                        Err(e) => self.error = Some(e),
                    }
                }
                if let Some(p) = &self.bim.preview_type {
                    for m in p.couches_assemblage(&ids[0]) {
                        ui.label(format!(
                            "{} · {}",
                            m.nom,
                            crate::options::unite()
                                .afficher(p.type_mur(&m.type_mur).map_or(0.0, |t| t.epaisseur()))
                        ));
                    }
                    if ui.button("Appliquer cette composition").clicked() {
                        commande = Some(Commande::ActualiserType {
                            assemblage: ids[0].clone(),
                        });
                        self.bim.preview_type = None;
                    }
                }
                });
            }
            if dalle {
                ui.menu_button("Contour / trémie rectangulaire", |ui| {
                    points_ui(ui, &mut self.bim.points);
                    let [a, b] = self.bim.points;
                    let contour = vec![a, [b[0], a[1]], b, [a[0], b[1]]];
                    if ui.button("Ajouter une trémie").clicked() {
                        commande = Some(Commande::Tremie {
                            id: ids[0].clone(),
                            contour: contour.clone(),
                        });
                    }
                    if ui
                        .button("Remplacer le contour (retire les pentes)")
                        .clicked()
                    {
                        commande = Some(Commande::Contour {
                            id: ids[0].clone(),
                            contours: vec![contour],
                        });
                    }
                });
            }
        });
        if let Some(c) = commande {
            if let Err(e) = self.executer_bim(c) {
                self.error = Some(e);
            }
        }
        if let Some(m) = mur.filter(|_| dissocier || associer.is_some()) {
            let mut p = self.natif.as_ref().unwrap().0.clone();
            let r = match &associer {
                Some(a) => p.associer(&m.id, a),
                None => p.dissocier(&m.id).map(|_| ()),
            };
            if let Err(e) = r.and_then(|()| self.appliquer_projet(p)) {
                self.error = Some(e);
            }
        }
        if let Some(sel) = selection {
            self.select(Some(sel), true);
        }
        if let (Some(id), Some(cotes)) = (escalier, garde_corps) {
            if let Err(e) = self.garde_corps_escalier(&id, &cotes) {
                self.error = Some(e);
            }
        }
        if let Some(id) = fenetre.filter(|_| editer_famille) {
            self.ouvrir_editeur_famille(&id);
        }
    }

    /// Barre d'options affichée : Modifier, projet natif, outil à réglages ou sélection
    /// avec des commandes propres.
    pub(super) fn a_options_modifier(&self) -> bool {
        use crate::ruban::Outil;
        let Some((p, _)) = &self.natif else { return false };
        if self.onglet_ruban != OngletRuban::Modifier {
            return false;
        }
        if matches!(self.ruban_edition.outil(), Some(Outil::Deplacer | Outil::Copier | Outil::Tourner | Outil::Aligner | Outil::Ajuster)) {
            return true;
        }
        let ids = self.selection_bim();
        ids.iter().any(|id| p.murs.iter().any(|m| &m.id == id) || p.assemblage(id).is_some())
            || (ids.len() == 1 && p.dalles.iter().any(|d| d.id == ids[0]))
            || (ids.len() == 1 && p.escaliers.iter().any(|e| e.id == ids[0]))
            || (ids.len() == 1 && p.ouvertures.iter().any(|o| o.id == ids[0] && o.genre != crate::modele::GenreOuverture::Libre))
    }

    pub(super) fn vue_native_id(&self) -> Option<String> {
        let p = &self.natif.as_ref()?.0;
        let num = self
            .scene
            .as_ref()?
            .navigateur
            .as_ref()?
            .vues
            .get(self.vue_active?)?
            .revit_id;
        p.vues
            .iter()
            .find(|v| scene::numero_vue(&v.id) == num)
            .map(|v| v.id.clone())
    }
    pub(super) fn annotations_natives(&self) -> Vec<crate::trace2d::Primitive> {
        let Some(id) = self.vue_native_id() else {
            return Vec::new();
        };
        let p = &self.natif.as_ref().unwrap().0;
        let Some(v) = p.vues.iter().find(|v| v.id == id) else {
            return Vec::new();
        };
        let origin = self.scene.as_ref().map_or(glam::DVec3::ZERO, |s| s.origin);
        let selection: HashSet<u64> = self
            .bim
            .selection
            .iter()
            .map(|id| scene::id_natif("annotation", id))
            .collect();
        crate::modele::documentation::annotations(p, v)
            .into_iter()
            .map(|mut a| {
                if a.element.is_some_and(|id| selection.contains(&id)) {
                    a.style.couleur = [230, 120, 20, 255];
                }
                a.translater(-origin)
            })
            .collect()
    }
}

fn barre_ui(ui: &mut egui::Ui, b: &mut crate::modele::Barre, niveaux: &[crate::modele::Niveau]) {
    use crate::modele::GenreBarre;
    ui.horizontal(|ui| {
        ui.label("Nom");
        ui.text_edit_singleline(&mut b.nom);
    });
    egui::ComboBox::from_id_salt("niveau_structure")
        .selected_text(
            niveaux
                .iter()
                .find(|n| n.id == b.niveau)
                .map_or("Choisir un niveau", |n| n.nom.as_str()),
        )
        .show_ui(ui, |ui| {
            for n in niveaux {
                ui.selectable_value(&mut b.niveau, n.id.clone(), &n.nom);
            }
        });
    for (i, nom) in ["X départ", "Y départ", "Z départ (relatif au niveau)"]
        .iter()
        .enumerate()
    {
        ui.horizontal(|ui| {
            ui.label(*nom);
            ui.add(
                egui::DragValue::new(&mut b.debut[i])
                    .speed(10.0)
                    .dimensions(),
            );
        });
    }
    if b.genre == GenreBarre::Poteau {
        b.fin[0] = b.debut[0];
        b.fin[1] = b.debut[1];
        ui.horizontal(|ui| {
            ui.label("Z sommet (relatif au niveau)");
            ui.add(egui::DragValue::new(&mut b.fin[2]).speed(10.0).dimensions());
        });
        ui.horizontal(|ui| {
            ui.label("Rotation");
            ui.add(
                egui::DragValue::new(&mut b.rotation)
                    .speed(1.0)
                    .suffix(" °"),
            );
        });
    } else {
        b.fin[2] = b.debut[2];
        for (i, nom) in ["X arrivée", "Y arrivée"].iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(*nom);
                ui.add(egui::DragValue::new(&mut b.fin[i]).speed(10.0).dimensions());
            });
        }
    }
    for (nom, valeur) in [
        ("Largeur", &mut b.largeur),
        (
            if b.genre == GenreBarre::Poteau {
                "Profondeur"
            } else {
                "Hauteur de section"
            },
            &mut b.hauteur,
        ),
    ] {
        ui.horizontal(|ui| {
            ui.label(nom);
            ui.add(
                egui::DragValue::new(valeur)
                    .speed(5.0)
                    .range(1.0..=100000.0)
                    .dimensions(),
            );
        });
    }
}

fn points_ui(ui: &mut egui::Ui, points: &mut [[f64; 2]; 2]) {
    for (k, p) in points.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("Point {}", k + 1));
            for v in p {
                ui.add(egui::DragValue::new(v).speed(10.0).dimensions());
            }
        });
    }
}
impl ViewerApp {
    pub(super) fn export_ifc_dialogue(&mut self) {
        let Some((p, _)) = &self.natif else { return };
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("projet.ifc")
            .add_filter("IFC4", &["ifc"])
            .save_file()
        else {
            return;
        };
        match crate::modele::export_ifc::exporter(p)
            .and_then(|s| crate::modele::enregistrement::ecrire_atomique(&path, s.as_bytes()))
        {
            Ok(()) => self.status = format!("IFC4 exporté : {}", path.display()),
            Err(e) => self.error = Some(e),
        }
    }
    pub(super) fn export_pdf_dialogue(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("dessin.pdf")
            .add_filter("PDF", &["pdf"])
            .save_file()
        else {
            return;
        };
        match self
            .dessin_vue()
            .and_then(|d| crate::modele::documentation::pdf(&d.svg("Dessin")))
            .and_then(|s| crate::modele::enregistrement::ecrire_atomique(&path, &s))
        {
            Ok(()) => self.status = format!("PDF exporté : {}", path.display()),
            Err(e) => self.error = Some(e),
        }
    }
    /// Fenêtre des nomenclatures du projet : créer, régler (nom, catégorie, regrouper),
    /// tableau des quantités, export CSV.
    pub(super) fn fenetre_documentation(&mut self, ctx: &egui::Context) {
        use crate::modele::{commandes::Commande, documentation as d};
        if !self.bim.documentation {
            return;
        }
        let Some((p, _)) = &self.natif else { return };
        let mut ouvert = true;
        let mut commande = None;
        let mut export = None;
        egui::Window::new("Nomenclatures").id(egui::Id::unique("fenetre_nomenclatures")).open(&mut ouvert).default_width(640.0).show(ctx,|ui|{
            if ui.button("Nouvelle nomenclature").clicked() {commande=Some(Commande::Nomenclature {nomenclature:d::Nomenclature {id:String::new(),nom:"Quantités".into(),categorie:String::new(),regrouper:true,colonnes:Vec::new(),parametres:Vec::new()}});self.bim.nomenclature=p.nomenclatures.len();}
            ui.separator();
            if p.nomenclatures.is_empty() { ui.label(RichText::new("Aucune nomenclature : « Nouvelle nomenclature » pour commencer.").weak()); }
            if !p.nomenclatures.is_empty() {
                self.bim.nomenclature=self.bim.nomenclature.min(p.nomenclatures.len()-1);
                egui::ComboBox::from_id_salt("nomenclature_projet").selected_text(&p.nomenclatures[self.bim.nomenclature].nom).show_ui(ui,|ui|{for (k,n) in p.nomenclatures.iter().enumerate(){ui.selectable_value(&mut self.bim.nomenclature,k,&n.nom);}});
                let mut n=p.nomenclatures[self.bim.nomenclature].clone();
                ui.horizontal(|ui|{ui.label("Nom");ui.text_edit_singleline(&mut n.nom);ui.label("Catégorie");ui.text_edit_singleline(&mut n.categorie);});
                ui.checkbox(&mut n.regrouper,"Regrouper par type");
                // colonnes : sans choix, catégorie, type ou élément, nombre, volume
                ui.horizontal_wrapped(|ui|{
                    ui.label("Colonnes");
                    let defaut=[d::Colonne::Categorie, if n.regrouper {d::Colonne::Type} else {d::Colonne::Element}, d::Colonne::Nombre, d::Colonne::Volume];
                    let actuelles: Vec<d::Colonne> = if n.colonnes.is_empty() { defaut.to_vec() } else { n.colonnes.clone() };
                    for c in d::Colonne::TOUTES {
                        let mut on = actuelles.contains(&c);
                        if ui.toggle_value(&mut on, c.titre()).changed() {
                            let mut v: Vec<d::Colonne> = d::Colonne::TOUTES.into_iter().filter(|x| if *x == c { on } else { actuelles.contains(x) }).collect();
                            if v.is_empty() { v.push(d::Colonne::Type); }
                            n.colonnes = v;
                        }
                    }
                });
                // paramètres des familles (type, occurrence) et partagés du projet
                let mut noms: std::collections::BTreeSet<String> = p.types_ouvertures.iter().filter_map(|t| t.modele.as_ref()).flat_map(|f| f.parametres.iter().map(|x| x.nom.clone())).collect();
                noms.extend(p.parametres_partages.keys().cloned());
                noms.extend(n.parametres.iter().cloned());
                if !noms.is_empty() {
                    ui.horizontal_wrapped(|ui|{
                        ui.label("Paramètres").on_hover_text("Valeurs des familles d'ouvertures : type, occurrence (par ouverture) et partagés (projet)");
                        for nom in &noms {
                            let mut on = n.parametres.contains(nom);
                            if ui.toggle_value(&mut on, nom).changed() {
                                if on { n.parametres.push(nom.clone()); } else { n.parametres.retain(|x| x != nom); }
                            }
                        }
                    });
                }
                if serde_json::to_string(&n).ok()!=serde_json::to_string(&p.nomenclatures[self.bim.nomenclature]).ok() {commande=Some(Commande::Nomenclature {nomenclature:n.clone()});}
                let cle=serde_json::to_string(&n).unwrap_or_default();
                if self.bim.tableau_cache.as_ref().is_none_or(|(r,c,_)|*r!=self.bim.revision || *c!=cle) {self.bim.tableau_cache=Some((self.bim.revision,cle,d::tableau(p,&n)));}
                match &self.bim.tableau_cache.as_ref().unwrap().2 {
                    Ok(lignes)=>{egui::ScrollArea::both().max_height(230.0).show(ui,|ui|{egui::Grid::new("quantites_projet").striped(true).show(ui,|ui|{for l in lignes {for c in l {ui.label(c);}ui.end_row();}});});}
                    Err(e)=>{ui.colored_label(Color32::DARK_RED,e);}
                }
                if ui.button("Exporter CSV…").clicked(){export=Some(self.bim.nomenclature);}
            }
        });
        self.bim.documentation = ouvert;
        if let Some(c) = commande {
            if let Err(e) = self.executer_bim(c) {
                self.error = Some(e);
            }
        }
        if let Some(k) = export {
            let Some(path) = rfd::FileDialog::new().set_file_name("nomenclature.csv").add_filter("csv", &["csv"]).save_file() else {
                return;
            };
            let p = &self.natif.as_ref().unwrap().0;
            let contenu = d::tableau(p, &p.nomenclatures[k]).map(|l| d::csv(&l).into_bytes());
            match contenu.and_then(|b| crate::modele::enregistrement::ecrire_atomique(&path, &b)) {
                Ok(()) => self.status = format!("Export : {}", path.display()),
                Err(e) => self.error = Some(e),
            }
        }
    }
}
