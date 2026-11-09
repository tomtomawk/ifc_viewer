//! Commandes du document BIM ; interface et pilote utilisent les mêmes transactions.

mod modifier;
pub use super::*;
use crate::config::options::Dimensions;
impl ViewerApp {
    pub(super) fn parcourir_historique(&mut self, retablir: bool) {
        self.parcourir_historique_n(retablir, 1);
    }

    pub(super) fn parcourir_historique_n(&mut self, retablir: bool, nombre: usize) {
        if self.famille_plane_active.is_some() { return; }
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
                crate::modele::echanges::enregistrement::effacer_recuperation(path);
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
        match crate::modele::echanges::enregistrement::enregistrer_natif(p, &chemin) {
            Ok(()) => {
                crate::modele::echanges::enregistrement::effacer_recuperation(ancien);
                let _ = self.historique.enregistrer(p);
                self.finaliser_enregistrement_sous(chemin);
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub(super) fn finaliser_enregistrement_sous(&mut self, chemin: PathBuf) {
        crate::modele::echanges::enregistrement::effacer_recuperation(&chemin);
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
                        crate::modele::echanges::depuis_rvt::convertir(&source, &progress)?
                    } else {
                        crate::modele::echanges::depuis_ifc::convertir(&source, &progress)?
                    };
                    let scene = scene::depuis_natif(&p, &sortie)?;
                    crate::modele::echanges::enregistrement::enregistrer_natif(&p, &sortie)?;
                    let rapport = crate::modele::echanges::rapport::markdown(
                        &p,
                        &mesures,
                        debut.elapsed().as_secs_f64(),
                    );
                    crate::modele::echanges::enregistrement::ecrire_atomique(
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
    abscisse: f64,
    pub(super) barre: crate::modele::Barre,
    pub(super) depart_structure: Option<[f64; 2]>,
    /// Composant à placer (type vide : type par défaut) et nom des pièces créées.
    pub(super) composant: crate::modele::Composant,
    /// Arbre à placer (essence, hauteur, houppier, décalage).
    pub(super) arbre: crate::modele::elements::arbre::Arbre,
    pub(super) nom_piece: String,
    /// Famille d'étiquette des pièces créées ; vide : standard.
    pub(super) etiquette_piece: String,
    barre_editee: Option<crate::modele::Barre>,
    annotation_identique: Option<crate::modele::documentation::Annotation>,
    /// Suppression demandée depuis le panneau : exécutée à l'image suivante.
    pub(super) supprimer: bool,
    pub documentation: bool,
    pub nomenclature: usize,
    pub texte: String,
    pub points: [[f64; 2]; 2],
    pub(super) attache_toiture_active: bool,
    pub(super) murs_a_attacher: Vec<String>,
    priorite: i32,
    pub revision: u64,
    tableau_cache: Option<(u64, String, Result<Vec<Vec<String>>, String>)>,
    /// Origine du projet sélectionnée (trait invisible) : ses coordonnées dans Propriétés.
    pub(super) origine: bool,
}
impl Default for EtatBim {
    fn default() -> Self {
        Self {
            origine: false,
            selection: Vec::new(),
            delta: [1000.0, 0.0, 0.0],
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
            composant: crate::modele::Composant { id: String::new(), nom: String::new(), type_composant: String::new(), niveau: String::new(), position: [0.0; 2], decalage: 0.0, rotation: 0.0 },
            arbre: crate::modele::elements::arbre::Arbre { id: String::new(), nom: String::new(), niveau: String::new(), position: [0.0; 2], decalage: 0.0, essence: Default::default(), hauteur: 10_000.0, diametre: 7_000.0, asset: None },
            nom_piece: "Pièce".into(),
            etiquette_piece: String::new(),
            barre_editee: None,
            annotation_identique: None,
            supprimer: false,
            documentation: false,
            nomenclature: 0,
            texte: "Texte".into(),
            points: [[0.0, 0.0], [3000.0, 0.0]],
            attache_toiture_active: false,
            murs_a_attacher: Vec::new(),
            priorite: 30,
            revision: 0,
            tableau_cache: None,
        }
    }
}
impl ViewerApp {
    pub(super) fn cle_native(&self, id: Id) -> Option<String> {
        let p = &self.natif.as_ref()?.0;
        elements_natifs(p)
            .find(|(genre, cle)| scene::id_natif(genre, cle) == id)
            .map(|(_, cle)| cle.clone())
    }
    /// Objets de la scène dont l'élément natif est verrouillé.
    pub(super) fn objets_verrouilles(&self) -> Vec<Id> {
        let Some((p, _)) = &self.natif else { return Vec::new() };
        if p.verrous.is_empty() {
            return Vec::new();
        }
        elements_natifs(p).filter(|(_, cle)| p.verrous.contains_key(*cle)).map(|(genre, cle)| scene::id_natif(genre, cle)).collect()
    }
    /// Bouton Verrou du ruban : verrouille la sélection, ou la déverrouille si elle l'est
    /// déjà entièrement.
    pub(super) fn basculer_verrou_selection(&mut self) {
        let ids = self.selection_bim();
        let tous = !ids.is_empty() && self.natif.as_ref().is_some_and(|(p, _)| ids.iter().all(|id| p.verrous.contains_key(id)));
        self.verrouiller_selection(!tous);
    }
    /// Verrouille ou déverrouille la sélection (ruban Modifier, CD / DC).
    pub(super) fn verrouiller_selection(&mut self, verrou: bool) {
        let ids = self.selection_bim();
        if ids.is_empty() {
            self.status = format!("{} : sélectionner d'abord un ou plusieurs éléments.", if verrou { "Verrouiller" } else { "Déverrouiller" });
            return;
        }
        let n = ids.len();
        match self.executer_bim(crate::modele::edition::commandes::Commande::Verrouiller { ids, verrou }) {
            Ok(_) => self.status = format!("{n} élément(s) {} — Ctrl+Z pour annuler.", if verrou { "verrouillé(s)" } else { "déverrouillé(s)" }),
            Err(e) => self.error = Some(e),
        }
    }
    /// Cadenas sur les éléments verrouillés ; un clic sur un cadenas déverrouille l'élément.
    /// Rend vrai si le clic a été consommé.
    pub(super) fn dessiner_cadenas(&mut self, ui: &egui::Ui, rect: egui::Rect, clic: Option<egui::Pos2>) -> bool {
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
            match self.executer_bim(crate::modele::edition::commandes::Commande::Verrouiller { ids: vec![id], verrou: false }) {
                Ok(_) => self.status = "Élément déverrouillé — Ctrl+Z pour annuler.".into(),
                Err(e) => self.error = Some(e),
            }
            return true;
        }
        false
    }
}

pub(super) fn elements_natifs(p: &crate::modele::Projet) -> impl Iterator<Item = (&'static str, &String)> {
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
    pub(super) fn executer_bim_suite(
        &mut self,
        premiere: crate::modele::edition::commandes::Commande,
        suite: impl FnOnce(Vec<String>) -> crate::modele::edition::commandes::Commande,
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
    pub(super) fn executer_bim(
        &mut self,
        c: crate::modele::edition::commandes::Commande,
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
    /// Une porte ou une fenêtre se place toujours avec une famille du projet.
    fn verifier_type_ouverture(&self) -> Result<(), String> {
        use crate::ui::ruban::Outil;
        let libre = self.ruban_edition.texte_option(Outil::Ouverture, 0) == "Libre";
        if libre || self.ruban_edition.ouverture_type_identique.is_some() {
            Ok(())
        } else {
            Err("Aucune famille de ce genre dans le projet : chargez-en une (sélecteur de type > Charger une famille…).".into())
        }
    }

    fn commande_ouverture(&self, mur: String, abscisse: f64) -> crate::modele::edition::commandes::Commande {
        use crate::ui::ruban::Outil;
        let fenetre = self.ruban_edition.texte_option(Outil::Ouverture, 0) == "Fenêtre";
        crate::modele::edition::commandes::Commande::Ouverture {
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
            let (axe, _, _) = crate::modele::volumes::solides::hote(p, &id)?;
            self.verifier_type_ouverture()?;
            self.executer_bim(self.commande_ouverture(id, axe.abscisse(point)))?;
            Ok(())
        })();
        if let Err(e) = resultat {
            self.error = Some(e);
        }
    }
    /// Place le composant de l'outil au point `position`, au niveau de la vue.
    pub(super) fn placer_arbre(&mut self, position: [f64; 2]) -> Result<(), String> {
        let mut a = self.bim.arbre.clone();
        a.id.clear();
        a.nom.clear();
        a.position = position;
        a.niveau = self.niveau_vue_native().ok_or("Arbre : aucun niveau.")?;
        self.executer_bim(crate::modele::edition::commandes::Commande::Arbre { arbre: a })?;
        Ok(())
    }

    pub(super) fn placer_composant(&mut self, position: [f64; 2]) -> Result<(), String> {
        let mut c = self.bim.composant.clone();
        c.id.clear();
        c.nom.clear();
        c.position = position;
        c.niveau = self.niveau_vue_native().ok_or("Composant : aucun niveau.")?;
        let p = &self.natif.as_ref().ok_or("Aucun projet natif.")?.0;
        // type par défaut : chargé au premier placement
        if !p.types_composants.iter().any(|t| t.id == c.type_composant) {
            if c.type_composant != crate::modele::elements::composant::type_table().id {
                return Err("Type de composant introuvable.".into());
            }
            c.type_composant.clear();
        }
        self.executer_bim(crate::modele::edition::commandes::Commande::Composant { composant: c })?;
        Ok(())
    }

    /// Crée une pièce au point `point` (contour détecté depuis les murs du niveau de la vue).
    pub(super) fn placer_piece(&mut self, point: [f64; 2]) -> Result<(), String> {
        let niveau = self.niveau_vue_native().ok_or("Pièce : aucun niveau.")?;
        let piece = crate::modele::Piece { id: String::new(), nom: self.bim.nom_piece.clone(), numero: String::new(), niveau, point, contour: Vec::new(), etiquette: self.bim.etiquette_piece.clone() };
        self.executer_bim(crate::modele::edition::commandes::Commande::Piece { piece })?;
        Ok(())
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
        self.executer_bim(crate::modele::edition::commandes::Commande::Barre { barre: b })?;
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
        match self.executer_bim(crate::modele::edition::commandes::Commande::Supprimer { ids }) {
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
            use crate::ui::ruban::Outil;
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
                F::CoteAngulaire { .. } => Outil::CoteAngulaire,
                F::CoteDiametre { .. } => Outil::CoteDiametre,
                F::Altimetrie { .. } => Outil::Altimetrie,
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

    /// Phase, sous-projet et variante de la sélection, modifiables d'ici : « Varié » quand
    /// les éléments diffèrent. Retourne le projet modifié.
    pub(super) fn contexte_selection(&self, ui: &mut egui::Ui, ids: &[String]) -> Option<crate::modele::Projet> {
        let (p, _) = self.natif.as_ref()?;
        // éléments portant le contexte : couches des assemblages, murs, dalles, autres
        let mut cibles: Vec<String> = Vec::new();
        for id in ids {
            if let Some(a) = p.assemblage(id) {
                cibles.extend(a.couches.iter().cloned());
            } else if let Some(o) = p.ouvertures.iter().find(|o| &o.id == id) {
                cibles.extend(p.assemblage(&o.mur).map_or_else(|| vec![o.mur.clone()], |a| a.couches.clone()));
            } else {
                cibles.push(id.clone());
            }
        }
        cibles.sort();
        cibles.dedup();
        // (phase, sous-projet, variante) de chaque cible
        let lire = |id: &String| -> (Option<String>, Option<String>, Option<String>) {
            if let Some(m) = p.murs.iter().find(|m| &m.id == id) {
                (m.phase.clone(), m.sous_projet.clone(), m.variante.clone())
            } else if let Some(d) = p.dalles.iter().find(|d| &d.id == id) {
                (d.phase.clone(), d.sous_projet.clone(), d.variante.clone())
            } else {
                let c = p.contextes.get(id).cloned().unwrap_or_default();
                (c.phase, c.sous_projet, c.variante)
            }
        };
        let valeurs: Vec<_> = cibles.iter().map(lire).collect();
        let mut resultat: Option<(usize, Option<String>)> = None;
        let largeur = ((ui.available_width() - 8.0) / 3.0).max(30.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (i, nom) in ["Phase", "Sous-projet", "Variante"].iter().enumerate() {
                ui.vertical(|ui| {
                    ui.set_width(largeur);
                    ui.add(egui::Label::new(RichText::new(*nom).small()).truncate());
                    let champ = |v: &(Option<String>, Option<String>, Option<String>)| match i { 0 => v.0.clone(), 1 => v.1.clone(), _ => v.2.clone() };
                    let mixte = valeurs.iter().any(|v| champ(v) != valeurs.first().map(champ).unwrap_or(None));
                    let actuel = valeurs.first().map(champ).unwrap_or(None);
                    let libelle = |id: &Option<String>| match (i, id) {
                        (0, None) => "Projet".to_string(),
                        (1, None) => "A".to_string(),
                        (_, None) => "Principale".to_string(),
                        (2, Some(id)) => p.variantes.iter().find(|x| &x.id == id).map_or(id.clone(), |x| x.libelle()),
                        (_, Some(v)) => v.clone(),
                    };
                    let choix: Vec<Option<String>> = match i {
                        0 => p.phases.iter().cloned().map(Some).collect(),
                        1 => p.sous_projets.iter().cloned().map(Some).collect(),
                        _ => std::iter::once(None).chain(p.variantes.iter().map(|v| Some(v.id.clone()))).collect(),
                    };
                    egui::ComboBox::from_id_salt(("contexte_selection", i))
                        .width(largeur)
                        .truncate()
                        .selected_text(if mixte { "Varié".to_string() } else { libelle(&actuel) })
                        .show_ui(ui, |ui| {
                            for c in &choix {
                                if ui.selectable_label(!mixte && *c == actuel, libelle(c)).clicked() {
                                    resultat = Some((i, c.clone()));
                                }
                            }
                        });
                });
            }
        });
        let (i, valeur) = resultat?;
        let mut q = p.clone();
        for id in &cibles {
            if let Some(m) = q.murs.iter_mut().find(|m| &m.id == id) {
                match i { 0 => m.phase = valeur.clone(), 1 => m.sous_projet = valeur.clone(), _ => m.variante = valeur.clone() }
            } else if let Some(d) = q.dalles.iter_mut().find(|d| &d.id == id) {
                match i { 0 => d.phase = valeur.clone(), 1 => d.sous_projet = valeur.clone(), _ => d.variante = valeur.clone() }
            } else {
                let c = q.contextes.entry(id.clone()).or_default();
                match i { 0 => c.phase = valeur.clone(), 1 => c.sous_projet = valeur.clone(), _ => c.variante = valeur.clone() }
            }
        }
        Some(q)
    }

    pub(super) fn panneau_bim(&mut self, ui: &mut egui::Ui) -> bool {
        use crate::modele::edition::commandes::Commande;
        use crate::ui::ruban::Outil;
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
                F::CoteAngulaire { .. } => Outil::CoteAngulaire,
                F::CoteDiametre { .. } => Outil::CoteDiametre,
                F::Altimetrie { .. } => Outil::Altimetrie,
            };
            if self.onglet_ruban != OngletRuban::Annoter || outil != Some(attendu) {
                self.bim.annotation_identique = None;
            }
        }
        let ids = self.selection_bim();
        let mut commande = None;
        let mut ouvrir_composant: Option<Option<String>> = None;
        let mut ouvrir_etiquette: Option<(Option<String>, Option<String>)> = None;
        if self.onglet_ruban == OngletRuban::Modifier {
            if ids.len() == 1 {
                let p = &self.natif.as_ref().unwrap().0;
                if let Some(original) = p.terrains.iter().find(|t| t.id == ids[0]) {
                    let mut terrain = original.clone();
                    ui.label(RichText::new("Terrain").strong());
                    let mut change = ui.text_edit_singleline(&mut terrain.nom).changed();
                    ui.label("Altitudes du contour");
                    for (i, point) in terrain.contour.iter_mut().enumerate() {
                        ui.horizontal(|ui| { ui.label(format!("Sommet {}", i + 1)); change |= ui.add(egui::DragValue::new(&mut point[2]).speed(10.0).suffix(" mm")).changed(); });
                    }
                    for (i, point) in terrain.points.iter_mut().enumerate() {
                        ui.horizontal(|ui| { ui.label(format!("Point {}", i + 1)); change |= ui.add(egui::DragValue::new(&mut point[2]).speed(10.0).suffix(" mm")).changed(); });
                    }
                    if change { commande = Some(Commande::Terrain { terrain }); }
                }
                if let Some(original) = p.limites_propriete.iter().find(|l| l.id == ids[0]) {
                    let mut limite = original.clone();
                    ui.label(RichText::new("Limite de propriété").strong());
                    let mut change = ui.text_edit_singleline(&mut limite.nom).changed();
                    ui.horizontal(|ui| { ui.label("Altitude"); change |= ui.add(egui::DragValue::new(&mut limite.altitude).speed(10.0).suffix(" mm")).changed(); });
                    if change { commande = Some(Commande::LimitePropriete { limite }); }
                }
                if let Some(original) = p.vues.iter().find(|v| {
                    v.id == ids[0] && matches!(v.genre, crate::modele::GenreVue::Coupe { .. })
                }) {
                    let mut vue = original.clone();
                    ui.label(RichText::new("Propriétés de la coupe / élévation").strong());
                    ui.scope(|ui| {
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
                    ui.label(RichText::new("Propriétés du quadrillage").strong());
                    ui.scope(|ui| {
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
                    ui.label(RichText::new("Propriétés de l'annotation").strong());
                    ui.scope(|ui| {
                        use crate::modele::documentation::{AnnotationForme as F, PointCote};
                        let mut change = false;
                        match &mut annotation.forme {
                            F::Polyligne { points, fermee, .. } | F::Spline { points, fermee } => {
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
                            F::CoteAngulaire { sommet, debut, fin, rayon } => {
                                if let Some((a, ..)) = crate::modele::documentation::angle_branches((*sommet).into(), (*debut).into(), (*fin).into()) {
                                    ui.label(format!("Angle : {:.2}°", a.to_degrees()));
                                }
                                ui.horizontal(|ui| {
                                    ui.label("Rayon de l'arc");
                                    change |= ui.add(egui::DragValue::new(rayon).speed(10.0).range(1.0..=f64::MAX).dimensions()).changed();
                                });
                            }
                            F::CoteDiametre { centre, point } => {
                                ui.label(format!("Diamètre : {}", crate::config::options::unite().afficher(2.0 * glam::DVec2::from(*centre).distance((*point).into()))));
                                ui.horizontal(|ui| {
                                    ui.label("Centre");
                                    for x in centre.iter_mut() {
                                        change |= ui.add(egui::DragValue::new(x).speed(10.0).dimensions()).changed();
                                    }
                                });
                            }
                            F::Altimetrie { relative_niveau, decimales, .. } => {
                                egui::ComboBox::from_label("Référence").selected_text(if *relative_niveau { "Niveau sous le point" } else { "Projet" }).show_ui(ui, |ui| {
                                    change |= ui.selectable_value(relative_niveau, false, "Projet").changed();
                                    change |= ui.selectable_value(relative_niveau, true, "Niveau sous le point").changed();
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Décimales (m)");
                                    change |= ui.add(egui::DragValue::new(decimales).range(0..=4)).changed();
                                });
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
                        // une polyligne ouverte perd son remplissage
                        if let F::Polyligne { fermee: false, remplissage, .. } = &mut annotation.forme {
                            *remplissage = None;
                        }
                        if change {
                            commande = Some(Commande::Annoter { annotation });
                        }
                    });
                }
                if let Some(original) = p.pieces.iter().find(|x| x.id == ids[0]) {
                    let mut x = original.clone();
                    ui.label(RichText::new("Pièce").strong());
                    let mut change = false;
                    egui::Grid::new("proprietes_piece").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                        ui.label("Nom");
                        change |= ui.text_edit_singleline(&mut x.nom).changed();
                        ui.end_row();
                        ui.label("Numéro");
                        change |= ui.text_edit_singleline(&mut x.numero).changed();
                        ui.end_row();
                        ui.label("Surface");
                        ui.label(if x.fermee() { format!("{:.2} m²", x.aire() / 1e6) } else { "Non fermée".into() });
                        ui.end_row();
                        ui.label("Périmètre");
                        ui.label(crate::config::options::unite().afficher(x.perimetre()));
                        ui.end_row();
                        ui.label("Étiquette");
                        change |= choix_famille_etiquette(ui, "etiquette_piece_selection", &mut x.etiquette, &p.familles_etiquettes);
                        ui.end_row();
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Nouvelle famille").on_hover_text("Nouvelle famille d'étiquette, sur la base de celle-ci, affectée à cette pièce").clicked() {
                            ouvrir_etiquette = Some((None, Some(x.id.clone())));
                        }
                    });
                    if change && !x.nom.trim().is_empty() && !x.numero.trim().is_empty() {
                        commande = Some(Commande::Piece { piece: x });
                    }
                }
                if let Some(original) = p.composants.iter().find(|x| x.id == ids[0]) {
                    let mut x = original.clone();
                    ui.label(RichText::new("Composant").strong());
                    let mut change = false;
                    egui::Grid::new("proprietes_composant").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                        ui.label("Nom");
                        change |= ui.text_edit_singleline(&mut x.nom).changed();
                        ui.end_row();
                        ui.label("Type");
                        change |= choix_type_composant(ui, "type_composant_selection", &mut x.type_composant, &p.types_composants);
                        ui.end_row();
                        ui.label("Position");
                        ui.horizontal(|ui| {
                            for v in &mut x.position {
                                change |= ui.add(egui::DragValue::new(v).speed(10.0).dimensions()).changed();
                            }
                        });
                        ui.end_row();
                        ui.label("Décalage");
                        change |= ui.add(egui::DragValue::new(&mut x.decalage).speed(10.0).dimensions()).changed();
                        ui.end_row();
                        ui.label("Rotation");
                        change |= ui.add(egui::DragValue::new(&mut x.rotation).speed(1.0).suffix(" °")).changed();
                        ui.end_row();
                    });
                    if change {
                        commande = Some(Commande::Composant { composant: x });
                    }
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
                    ui.label(RichText::new("Propriétés structurelles").strong());
                    ui.scope(|ui| {
                        let b = self.bim.barre_editee.as_mut().unwrap();
                        let avant = serde_json::to_string(&*b).ok();
                        barre_ui(ui, b, &p.niveaux);
                        if serde_json::to_string(&*b).ok() != avant {
                            commande = Some(Commande::Barre { barre: b.clone() });
                        }
                    });
                } else {
                    self.bim.barre_editee = None;
                }
            } else {
                self.bim.barre_editee = None;
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
                                // largeur relative : l'altitude reste visible quel que soit le panneau
                                let largeur = (ui.available_width() - 90.0).clamp(40.0, 180.0);
                                ui.add(egui::TextEdit::singleline(&mut nom).desired_width(largeur));
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
                    let nom = if p.niveaux.is_empty() { "RDC".to_string() } else { (1..).map(|k| format!("R+{k}")).find(|n| !p.niveaux.iter().any(|x| &x.nom == n)).unwrap_or_default() };
                    if ui.button("Ajouter un niveau").on_hover_text(format!("« {nom} » à {}", crate::config::options::unite().afficher(altitude))).clicked() {
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
                Some(Outil::Composant) => {
                    let p = &self.natif.as_ref().unwrap().0;
                    let mut types = p.types_composants.clone();
                    let defaut = crate::modele::elements::composant::type_table();
                    if !types.iter().any(|t| t.id == defaut.id) {
                        types.insert(0, defaut.clone());
                    }
                    let c = &mut self.bim.composant;
                    if !types.iter().any(|t| t.id == c.type_composant) {
                        c.type_composant = types[0].id.clone();
                    }
                    ui.label(RichText::new("Composant").strong());
                    egui::Grid::new("outil_composant").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                        ui.label("Type");
                        choix_type_composant(ui, "type_composant_outil", &mut c.type_composant, &types);
                        ui.end_row();
                        ui.label("Rotation");
                        ui.add(egui::DragValue::new(&mut c.rotation).speed(1.0).suffix(" °"));
                        ui.end_row();
                        ui.label("Décalage");
                        ui.add(egui::DragValue::new(&mut c.decalage).speed(10.0).dimensions());
                        ui.end_row();
                    });
                    ui.label("Cliquez dans la vue pour placer le composant (niveau de la vue) ; Espace : rotation de 90°.");
                    let choisi = c.type_composant.clone();
                    ui.horizontal(|ui| {
                        if ui.button("Modifier la famille").on_hover_text("Ouvre la famille du type dans l'éditeur ; « Charger dans le projet » met à jour tous ses composants").clicked() {
                            ouvrir_composant = Some(Some(choisi));
                        }
                        if ui.button("Nouvelle famille").on_hover_text("Nouvelle famille de composant dans l'éditeur, sur la base de la table").clicked() {
                            ouvrir_composant = Some(None);
                        }
                    });
                }
                Some(Outil::Arbre) => {
                    use crate::modele::elements::arbre::Essence;
                    let a = &mut self.bim.arbre;
                    ui.label(RichText::new("Arbre").strong());
                    egui::Grid::new("outil_arbre").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                        ui.label("Essence");
                        let avant = a.essence;
                        egui::ComboBox::from_id_salt("essence_arbre").selected_text(a.essence.nom()).show_ui(ui, |ui| {
                            for e in Essence::TOUTES {
                                ui.selectable_value(&mut a.essence, e, e.nom());
                            }
                        });
                        if a.essence != avant {
                            (a.hauteur, a.diametre) = a.essence.dimensions();
                        }
                        ui.end_row();
                        ui.label("Hauteur");
                        ui.add(egui::DragValue::new(&mut a.hauteur).speed(50.0).range(500.0..=60_000.0).dimensions());
                        ui.end_row();
                        ui.label("Houppier").on_hover_text("Diamètre du houppier");
                        ui.add(egui::DragValue::new(&mut a.diametre).speed(50.0).range(300.0..=40_000.0).dimensions());
                        ui.end_row();
                        ui.label("Décalage").on_hover_text("Hauteur du pied par rapport au niveau de la vue");
                        ui.add(egui::DragValue::new(&mut a.decalage).speed(10.0).dimensions());
                        ui.end_row();
                    });
                    ui.label("Cliquez dans la vue pour planter l'arbre (niveau de la vue).");
                }
                Some(Outil::Piece) => {
                    ui.label(RichText::new("Pièce").strong());
                    ui.horizontal(|ui| {
                        ui.label("Nom");
                        ui.text_edit_singleline(&mut self.bim.nom_piece);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Étiquette");
                        let familles = self.natif.as_ref().map(|(p, _)| p.familles_etiquettes.clone()).unwrap_or_default();
                        choix_famille_etiquette(ui, "etiquette_piece_outil", &mut self.bim.etiquette_piece, &familles);
                        if ui.button("Modifier").on_hover_text("Éditer cette famille d'étiquette").clicked() {
                            ouvrir_etiquette = Some((Some(self.bim.etiquette_piece.clone()), None));
                        }
                    });
                    ui.label("Cliquez dans un espace fermé par des murs du niveau de la vue : le contour suit les faces intérieures ; le numéro s'incrémente.");
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
                        if std::mem::take(&mut self.ruban_edition.demande_charger_famille)
                            && let Err(e) = self.charger_famille_dialogue(true)
                        {
                            self.error = Some(e);
                        }
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
                        match self.verifier_type_ouverture() {
                            Ok(()) => commande = Some(self.commande_ouverture(ids[0].clone(), self.bim.abscisse)),
                            Err(e) => self.error = Some(e),
                        }
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
        if let Some((famille, piece)) = ouvrir_etiquette {
            self.editer_famille_etiquette(famille, piece);
        }
        if let Some(t) = ouvrir_composant {
            self.ouvrir_editeur_composant(t);
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

/// Liste des types de composants ; vrai si le choix a changé.
fn choix_famille_etiquette(ui: &mut egui::Ui, salt: &str, choix: &mut String, familles: &[crate::modele::documentation::FamilleEtiquette]) -> bool {
    let avant = choix.clone();
    egui::ComboBox::from_id_salt(salt)
        .selected_text(familles.iter().find(|f| &f.id == choix).map_or("Étiquette standard", |f| f.nom.as_str()))
        .show_ui(ui, |ui| {
            ui.selectable_value(choix, String::new(), "Étiquette standard");
            for f in familles {
                ui.selectable_value(choix, f.id.clone(), &f.nom);
            }
        });
    *choix != avant
}

fn choix_type_composant(ui: &mut egui::Ui, salt: &str, choix: &mut String, types: &[crate::modele::TypeComposant]) -> bool {
    let avant = choix.clone();
    egui::ComboBox::from_id_salt(salt)
        .selected_text(types.iter().find(|t| &t.id == choix).map_or("Choisir un type", |t| t.nom.as_str()))
        .show_ui(ui, |ui| {
            for t in types {
                ui.selectable_value(choix, t.id.clone(), &t.nom);
            }
        });
    *choix != avant
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
