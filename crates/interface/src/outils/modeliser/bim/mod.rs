//! Outils de création BIM : état des outils (`EtatBim`), placements (ouverture, arbre, composant,
//! pièce, structure), suppression, Créer à l'identique, contexte de la sélection.
//! Panneau d'options : `panneau.rs` ; widgets : `widgets.rs`.

pub use crate::*;
use commandes::Execution;
use config::options::Dimensions;

mod identique;
mod panneau;
mod poteau;
mod widgets;


pub(crate) struct EtatBim {
    pub selection: Vec<String>,
    pub(crate) delta: [f64; 3],
    abscisse: f64,
    pub(crate) barre: modele::Barre,
    niveau_haut_poteau: Option<String>,
    decalage_haut_poteau: f64,
    /// Réglages des prochaines poutres (outil Poutre, n° 6 à 10).
    pub(crate) poutre: crate::outils::modeliser::poutres::ReglagesPoutre,
    pub(crate) depart_structure: Option<[f64; 2]>,
    /// Composant à placer (type vide : type par défaut) et nom des pièces créées.
    pub(crate) composant: modele::Composant,
    /// Arbre à placer (essence, hauteur, houppier, décalage).
    pub(crate) arbre: modele::elements::arbre::Arbre,
    pub(crate) altitude_limite: Option<f64>,
    pub(crate) nom_piece: String,
    /// Famille d'étiquette des pièces créées ; vide : standard.
    pub(crate) etiquette_piece: String,
    pub(crate) barre_editee: Option<modele::Barre>,
    pub(crate) annotation_identique: Option<modele::documentation::Annotation>,
    /// Suppression demandée depuis le panneau : exécutée à l'image suivante.
    pub(crate) supprimer: bool,
    pub documentation: bool,
    pub nomenclature: usize,
    pub texte: String,
    pub points: [[f64; 2]; 2],
    pub(crate) attache_toiture_active: bool,
    pub(crate) murs_a_attacher: Vec<String>,
    pub(crate) priorite: i32,
    pub revision: u64,
    pub(crate) tableau_cache: Option<(u64, String, Result<Vec<Vec<String>>, String>)>,
    /// Origine du projet sélectionnée (trait invisible) : ses coordonnées dans Propriétés.
    pub(crate) origine: bool,
}
impl Default for EtatBim {
    fn default() -> Self {
        Self {
            origine: false,
            selection: Vec::new(),
            delta: [1000.0, 0.0, 0.0],
            abscisse: 1500.0,
            poutre: Default::default(),
            barre: modele::Barre {
                id: String::new(),
                nom: "Structure".into(),
                genre: modele::GenreBarre::Poteau,
                niveau: String::new(),
                debut: [0.0, 0.0, 0.0],
                fin: [0.0, 0.0, 2800.0],
                largeur: 300.0,
                hauteur: 300.0,
                rotation: 0.0,
                pose: Default::default(),
                origine: None,
            },
            niveau_haut_poteau: None,
            decalage_haut_poteau: 0.0,
            depart_structure: None,
            composant: modele::Composant { id: String::new(), nom: String::new(), type_composant: String::new(), niveau: String::new(), position: [0.0; 2], decalage: 0.0, rotation: 0.0 },
            arbre: modele::elements::arbre::Arbre { id: String::new(), nom: String::new(), niveau: String::new(), position: [0.0; 2], decalage: 0.0, essence: Default::default(), hauteur: 10_000.0, diametre: 7_000.0, asset: None },
            nom_piece: "Pièce".into(),
            altitude_limite: None,
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
    /// Une porte ou une fenêtre se place toujours avec une famille du projet.
    fn verifier_type_ouverture(&self) -> Result<(), String> {
        use crate::zones::ruban::Outil;
        let libre = self.ruban_edition.texte_option(Outil::Ouverture, 0) == "Libre";
        if libre || self.ruban_edition.ouverture_type_identique.is_some() {
            Ok(())
        } else {
            Err("Aucune famille de ce genre dans le projet : chargez-en une (sélecteur de type > Charger une famille…).".into())
        }
    }

    fn commande_ouverture(&self, mur: String, abscisse: f64) -> commandes::Commande {
        use crate::zones::ruban::Outil;
        let fenetre = self.ruban_edition.texte_option(Outil::Ouverture, 0) == "Fenêtre";
        commandes::Commande::Ouverture {
            mur,
            abscisse,
            type_ouverture: self.ruban_edition.ouverture_type_identique.clone(),
            genre: if fenetre {
                modele::GenreOuverture::Fenetre
            } else if self.ruban_edition.texte_option(Outil::Ouverture, 0) == "Libre" {
                modele::GenreOuverture::Libre
            } else {
                modele::GenreOuverture::Porte
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
    pub(crate) fn placer_ouverture(&mut self, id: Id, point: [f64; 2]) {
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
            let (axe, _, _) = modele::volumes::solides::hote(p, &id)?;
            self.verifier_type_ouverture()?;
            self.executer_bim(self.commande_ouverture(id, axe.abscisse(point)))?;
            Ok(())
        })();
        if let Err(e) = resultat {
            self.error = Some(e);
        }
    }
    /// Place le composant de l'outil au point `position`, au niveau de la vue.
    pub(crate) fn placer_arbre(&mut self, position: [f64; 2]) -> Result<(), String> {
        let mut a = self.bim.arbre.clone();
        a.id.clear();
        a.nom.clear();
        a.position = position;
        a.niveau = self.niveau_vue_native().ok_or("Arbre : aucun niveau.")?;
        self.executer_bim(commandes::Commande::Arbre { arbre: a })?;
        Ok(())
    }

    pub(crate) fn placer_composant(&mut self, position: [f64; 2]) -> Result<(), String> {
        let mut c = self.bim.composant.clone();
        c.id.clear();
        c.nom.clear();
        c.position = position;
        c.niveau = self.niveau_vue_native().ok_or("Composant : aucun niveau.")?;
        let p = &self.natif.as_ref().ok_or("Aucun projet natif.")?.0;
        // type par défaut : chargé au premier placement
        if !p.types_composants.iter().any(|t| t.id == c.type_composant) {
            if c.type_composant != modele::elements::composant::type_table().id {
                return Err("Type de composant introuvable.".into());
            }
            c.type_composant.clear();
        }
        self.executer_bim(commandes::Commande::Composant { composant: c })?;
        Ok(())
    }

    /// Crée une pièce au point `point` (contour détecté depuis les murs du niveau de la vue).
    pub(crate) fn placer_piece(&mut self, point: [f64; 2]) -> Result<(), String> {
        let niveau = self.niveau_vue_native().ok_or("Pièce : aucun niveau.")?;
        let piece = modele::Piece { id: String::new(), nom: self.bim.nom_piece.clone(), numero: String::new(), niveau, point, contour: Vec::new(), etiquette: self.bim.etiquette_piece.clone(), donnees: Default::default(), origine: None };
        self.executer_bim(commandes::Commande::Piece { piece })?;
        Ok(())
    }

    /// Réutilise les paramètres saisis, sans modifier le modèle tant que le placement n'est pas valide.
    pub(crate) fn placer_structure(
        &mut self,
        debut: [f64; 2],
        fin: Option<[f64; 2]>,
    ) -> Result<(), String> {
        if self.bim.barre.genre == modele::GenreBarre::Poteau {
            let barre = self.poteau_regle(debut)?;
            self.executer_bim(commandes::Commande::Barre { barre })?;
            return Ok(());
        }
        let mut b = self.bim.barre.clone();
        b.id.clear();
        b.debut[0] = debut[0];
        b.debut[1] = debut[1];
        let fin = fin.ok_or("Choisissez l'arrivée de la poutre.")?;
        b.fin = [fin[0], fin[1], b.debut[2]];
        self.executer_bim(commandes::Commande::Barre { barre: b })?;
        Ok(())
    }

    /// Supprime la sélection tout de suite (couches et ouvertures hébergées comprises),
    /// comme Revit ; Ctrl+Z la rétablit.
    pub(crate) fn demander_suppression(&mut self) {
        if self.supprimer_dans_jeu() {
            return;
        }
        let ids = self.selection_bim();
        if ids.is_empty() {
            self.status = "Supprimer : sélectionner d'abord un ou plusieurs éléments.".into();
            return;
        }
        let set = self.natif.as_ref().and_then(|(p, _)| p.dependances(&ids).ok().map(|s| (commandes::annotations_a_detacher(p, &s).len(), s)));
        let (n, m) = set.map_or((ids.len(), 0), |(m, s)| (s.len(), m));
        match self.executer_bim(commandes::Commande::Supprimer { ids }) {
            Ok(_) => {
                self.bim.selection.clear();
                let detachees = if m > 0 { format!(", {m} annotation(s) détachée(s)") } else { String::new() };
                self.status = format!("{n} élément(s) supprimé(s){detachees} — Ctrl+Z pour annuler.");
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Phase, sous-projet et variante de la sélection, modifiables d'ici : « Varié » quand
    /// les éléments diffèrent. Retourne le projet modifié.
    pub(crate) fn contexte_selection(&self, ui: &mut egui::Ui, ids: &[String]) -> Option<modele::Projet> {
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
}
