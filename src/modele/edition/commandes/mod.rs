//! Commandes d'édition sérialisables pour l'interface et le pilote MCP.

mod modifier;
pub use modifier::decaler_polygone;
mod transformations;
#[cfg(test)]
mod tests;
use crate::modele::*;
use glam::DVec2;
use std::collections::{HashMap, HashSet};
#[derive(Deserialize, Debug)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Commande {
    Terrain { terrain: site::Terrain },
    LimitePropriete { limite: site::LimitePropriete },
    /// Coordonnées de l'origine du projet : E/O, N/S, cote NGF (mm).
    CoordonneesOrigine { coordonnees: [f64; 3] },
    Barre {
        barre: Barre,
    },
    /// Crée (identifiant vide : numéro suivant, contour détecté depuis les murs ; refusée
    /// si l'espace n'est pas fermé) ou modifie une pièce.
    Piece {
        piece: Piece,
    },
    /// Crée (identifiant vide ; type vide : type par défaut) ou modifie un composant.
    Composant {
        composant: Composant,
    },
    /// Crée (identifiant vide) ou modifie un arbre.
    Arbre {
        arbre: crate::modele::elements::arbre::Arbre,
    },
    /// Crée (identifiant vide) ou remplace un type de composant.
    TypeComposant {
        type_composant: TypeComposant,
    },
    /// Crée (identifiant vide) ou remplace un escalier.
    Escalier {
        escalier: Escalier,
    },
    /// Crée (identifiant vide) ou remplace un garde-corps.
    GardeCorps {
        garde_corps: GardeCorps,
    },
    ModifierVue {
        vue: Vue,
    },
    ModifierQuadrillage {
        quadrillage: documentation::Quadrillage,
    },
    Niveau {
        nom: String,
        altitude: f64,
    },
    Quadrillage {
        nom: String,
        debut: [f64; 2],
        fin: [f64; 2],
    },
    Annoter {
        annotation: documentation::Annotation,
    },
    Feuille {
        feuille: documentation::Feuille,
    },
    Nomenclature {
        nomenclature: documentation::Nomenclature,
    },
    ModifierNiveau {
        id: String,
        nom: String,
        altitude: f64,
    },
    Ouverture {
        mur: String,
        genre: GenreOuverture,
        type_ouverture: Option<String>,
        abscisse: f64,
        largeur: f64,
        hauteur: f64,
        allege: f64,
    },
    Deplacer {
        ids: Vec<String>,
        delta: [f64; 3],
    },
    CopierSurNiveau { ids:Vec<String>, niveau:String },
    Copier {
        ids: Vec<String>,
        delta: [f64; 3],
    },
    Tourner {
        ids: Vec<String>,
        centre: [f64; 2],
        angle: f64,
    },
    Aligner {
        ids: Vec<String>,
        reference: String,
    },
    /// Prolonge (ou raccourcit) la ligne de justification du mur `id` jusqu'à `ligne`
    /// (une arête du mur `reference`) ; sans `ligne`, jusqu'à la ligne de justification
    /// de `reference`.
    Ajuster {
        id: String,
        reference: String,
        fin: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ligne: Option<[[f64; 2]; 2]>,
    },
    /// Raccorde les lignes de justification des deux murs à leur intersection, en une
    /// seule transaction.
    AjusterAngle { id: String, reference: String, fin: bool, fin_reference: bool },
    ModifierTrajectoireMur {id:String,points:[[f64;2];3]},
    InverserMur { id: String },
    ActualiserType {
        assemblage: String,
    },
    Attacher {
        ids: Vec<String>,
        toiture: Option<String>,
    },
    PrioriteJonction {
        id: String,
        priorite: i32,
    },
    Supprimer {
        ids: Vec<String>,
    },
    /// Symétrie par rapport à la droite (a, b) ; la face extérieure des murs reste dehors.
    Symetrie {
        ids: Vec<String>,
        a: [f64; 2],
        b: [f64; 2],
    },
    /// Mise à l'échelle autour de `centre` (annotations, contours, terrains).
    Echelle {
        ids: Vec<String>,
        centre: [f64; 2],
        facteur: f64,
    },
    /// Réseau de `nombre` éléments (original compris) : copie k tournée de k·angle autour
    /// de `centre` puis déplacée de k·delta.
    Reseau {
        ids: Vec<String>,
        nombre: usize,
        delta: [f64; 3],
        centre: [f64; 2],
        angle: f64,
    },
    /// Décale un mur droit ou une ligne de `distance` (positive : à gauche du sens).
    Decaler {
        id: String,
        distance: f64,
        copie: bool,
    },
    /// Scinde un mur droit au point projeté, avec un écart éventuel.
    Scinder {
        id: String,
        point: [f64; 2],
        ecart: f64,
    },
    /// Démolit les éléments dans `phase` (absente : annule la démolition).
    Demolir {
        ids: Vec<String>,
        phase: Option<String>,
    },
    /// Reproduit le type et les paramètres de `source` sur `cibles` (même catégorie).
    Reproduire {
        source: String,
        cibles: Vec<String>,
    },
    /// Place l'ouverture `id` sur le mur `mur`, centrée sur la projection de `point`.
    ChangerHote {
        id: String,
        mur: String,
        point: [f64; 2],
    },
    /// Inverse le côté d'ouverture d'une menuiserie, ou le sens de ses paumelles.
    InverserOuverture {
        id: String,
        paumelles: bool,
    },
    /// Verrouille (`verrou` vrai) ou déverrouille les éléments.
    Verrouiller {
        ids: Vec<String>,
        verrou: bool,
    },
    Contour {
        id: String,
        contours: Vec<Vec<[f64; 2]>>,
    },
    Tremie {
        id: String,
        contour: Vec<[f64; 2]>,
    },
}
fn identite(_p: &Projet, prefixe: &str) -> String {
    format!("{prefixe}-{}", crate::modele::collaboration::uuid())
}

impl Projet {
    /// Répercute une modification de composition sur tous les murs du type, en gardant
    /// leur ligne d'implantation et l'identité de chaque assemblage. L'opération entière
    /// est annulée si une des reconstructions échoue.
    pub fn actualiser_types_murs_modifies(&mut self, avant: &Projet) -> Result<(), String> {
        let types: std::collections::HashSet<String> = self.types_murs.iter().filter(|t| {
            avant.type_mur(&t.id).is_some_and(|ancien| ancien.couches != t.couches)
        }).map(|t| t.id.clone()).collect();
        if types.is_empty() { return Ok(()); }
        let mut travail = self.clone();
        let assemblages: Vec<String> = travail.assemblages.iter().filter(|a| types.contains(&a.type_mur) && avant.assemblage(&a.id).is_some_and(|ancien| ancien.type_mur == a.type_mur)).map(|a| a.id.clone()).collect();
        for id in assemblages {
            let (axe, _) = avant.trajectoire_justification(&id).ok_or("Ligne d'implantation du mur introuvable.")?;
            let assemblage = travail.assemblage(&id).ok_or("Assemblage absent.")?;
            let type_mur = travail.type_mur(&assemblage.type_mur).ok_or("Type absent.")?;
            let reference = travail.implantations.get(&id).copied().unwrap_or(crate::modele::murs::assemblage::Reference::Axe);
            let decalage = type_mur.decalages_couches().first().copied().unwrap_or(0.0) - type_mur.decalage_reference(reference);
            let premiere = assemblage.couches.first().cloned().ok_or("Assemblage vide.")?;
            travail.murs.iter_mut().find(|m| m.id == premiere).ok_or("Couche introuvable.")?.axe = assemblage::decaler_axe(&axe, decalage);
            travail.executer_interne(Commande::ActualiserType { assemblage: id })?;
        }
        let murs: Vec<String> = travail.murs.iter().filter(|m| m.assemblage.is_none() && types.contains(&m.type_mur) && avant.murs.iter().any(|a| a.id == m.id && a.type_mur == m.type_mur)).map(|m| m.id.clone()).collect();
        for id in murs {
            let (axe, _) = avant.trajectoire_justification(&id).ok_or("Ligne d'implantation du mur introuvable.")?;
            let mur = travail.murs.iter().find(|m| m.id == id).ok_or("Mur introuvable.")?;
            let type_mur = travail.type_mur(&mur.type_mur).ok_or("Type absent.")?;
            let reference = travail.implantations.get(&id).copied().unwrap_or(crate::modele::murs::assemblage::Reference::Axe);
            let axe = assemblage::decaler_axe(&axe, -type_mur.decalage_reference(reference));
            travail.murs.iter_mut().find(|m| m.id == id).unwrap().axe = axe;
            travail.eclater_mur(&id)?;
        }
        *self = travail;
        Ok(())
    }
    pub fn identifiants(&self) -> HashSet<String> {
        self.niveaux
            .iter()
            .map(|x| x.id.clone())
            .chain(self.murs.iter().map(|x| x.id.clone()))
            .chain(self.assemblages.iter().map(|x| x.id.clone()))
            .chain(self.dalles.iter().map(|x| x.id.clone()))
            .chain(self.terrains.iter().map(|x| x.id.clone()))
            .chain(self.limites_propriete.iter().map(|x| x.id.clone()))
            .chain(self.barres.iter().map(|x| x.id.clone()))
            .chain(self.pieces.iter().map(|x| x.id.clone()))
            .chain(self.composants.iter().map(|x| x.id.clone()))
            .chain(self.arbres.iter().map(|x| x.id.clone()))
            .chain(self.escaliers.iter().map(|x| x.id.clone()))
            .chain(self.garde_corps.iter().map(|x| x.id.clone()))
            .chain(self.ouvertures.iter().map(|x| x.id.clone()))
            .chain(self.importes.iter().map(|x| x.id.clone()))
            .chain(self.vues.iter().map(|x| x.id.clone()))
            .chain(self.quadrillages.iter().map(|x| x.id.clone()))
            .chain(self.annotations.iter().map(|x| x.id.clone()))
            .chain(self.feuilles.iter().map(|x| x.id.clone()))
            .chain(self.nomenclatures.iter().map(|x| x.id.clone()))
            .collect()
    }
    /// Copie de travail : une erreur ne laisse jamais de modification partielle.
    pub fn executer(&self, c: Commande) -> Result<(Projet, Vec<String>), String> {
        let mut p = self.clone();
        let c = self.filtrer_verrouilles(c)?;
        let ids = p.executer_interne(c)?;
        super::verrous::verifier(self, &p)?;
        p.detacher_couches_disjointes(self);
        p.recaler_menuiseries(self)?;
        p.actualiser_pieces();
        p.verifier()?;
        Ok((p, ids))
    }
    pub fn dependances(&self, ids: &[String]) -> Result<HashSet<String>, String> {
        self.dependances_avec(ids, false)
    }
    /// `couche_seule` : une couche d'assemblage peut être transformée seule (déplacer,
    /// tourner) ; elle se détache ensuite si elle quitte le mur composé.
    fn dependances_avec(&self, ids: &[String], couche_seule: bool) -> Result<HashSet<String>, String> {
        let connus = self.identifiants();
        if ids.is_empty() || ids.iter().any(|id| !connus.contains(id)) {
            return Err("Sélection vide ou identifiant absent.".into());
        }
        let mut out: HashSet<String> = ids.iter().cloned().collect();
        for a in &self.assemblages {
            if out.contains(&a.id) {
                out.extend(a.couches.clone());
            }
        }
        for m in &self.murs {
            if !couche_seule && out.contains(&m.id) && m.assemblage.as_ref().is_some_and(|a| !out.contains(a)) {
                return Err("Sélectionnez l'assemblage entier ou dissociez la couche avant cette opération.".into());
            }
        }
        for o in &self.ouvertures {
            if out.contains(&o.mur) {
                out.insert(o.id.clone());
            }
        }
        // garde-corps hébergés : suivent leur escalier
        for g in &self.garde_corps {
            if g.escalier.as_ref().is_some_and(|e| out.contains(e)) {
                out.insert(g.id.clone());
            }
        }
        Ok(out)
    }
    fn executer_interne(&mut self, c: Commande) -> Result<Vec<String>, String> {
        match c {
            // éléments du site : créés dans la phase Existant
            Commande::Terrain { mut terrain } => {
                terrain.triangles()?;
                if terrain.id.is_empty() { terrain.id = identite(self, "terrain"); }
                let id = terrain.id.clone();
                if let Some(t) = self.terrains.iter_mut().find(|t| t.id == id) { *t = terrain; } else { self.terrains.push(terrain); self.ranger_existant(&id); }
                Ok(vec![id])
            }
            Commande::LimitePropriete { mut limite } => {
                limite.verifier()?;
                if limite.id.is_empty() { limite.id = identite(self, "limite-propriete"); }
                let id = limite.id.clone();
                if let Some(l) = self.limites_propriete.iter_mut().find(|l| l.id == id) { *l = limite; } else { self.limites_propriete.push(limite); self.ranger_existant(&id); }
                Ok(vec![id])
            }
            Commande::CoordonneesOrigine { coordonnees } => {
                if !coordonnees.iter().all(|x| x.is_finite()) { return Err("Coordonnées de l'origine non valides.".into()); }
                self.coordonnees_origine = coordonnees;
                Ok(Vec::new())
            }
            Commande::ModifierQuadrillage { quadrillage } => {
                let actuel = self
                    .quadrillages
                    .iter_mut()
                    .find(|g| g.id == quadrillage.id)
                    .ok_or("Quadrillage absent.")?;
                let id = quadrillage.id.clone();
                *actuel = quadrillage;
                Ok(vec![id])
            }
            Commande::ModifierVue { vue } => {
                if vue.nom.trim().is_empty()
                    || self
                        .vues
                        .iter()
                        .any(|v| v.id != vue.id && v.nom.eq_ignore_ascii_case(vue.nom.trim()))
                {
                    return Err("Le nom de la vue est vide ou déjà utilisé.".into());
                }
                let actuel = self
                    .vues
                    .iter_mut()
                    .find(|v| v.id == vue.id)
                    .ok_or("Vue absente.")?;
                if !matches!(actuel.genre, GenreVue::Coupe { .. })
                    || !matches!(vue.genre, GenreVue::Coupe { .. })
                {
                    return Err("Sélectionnez une coupe ou une élévation.".into());
                }
                let id = vue.id.clone();
                *actuel = vue;
                Ok(vec![id])
            }
            Commande::Escalier { mut escalier } => {
                if escalier.id.is_empty() {
                    escalier.id = identite(self, "escalier");
                }
                escalier.calcul(self)?;
                let id = escalier.id.clone();
                if let Some(e) = self.escaliers.iter_mut().find(|e| e.id == id) {
                    *e = escalier;
                } else if self.identifiants().contains(&id) {
                    return Err("Identifiant déjà utilisé.".into());
                } else {
                    self.escaliers.push(escalier);
                }
                Ok(vec![id])
            }
            Commande::GardeCorps { mut garde_corps } => {
                if garde_corps.id.is_empty() {
                    garde_corps.id = identite(self, "garde-corps");
                }
                let id = garde_corps.id.clone();
                if let Some(g) = self.garde_corps.iter_mut().find(|g| g.id == id) {
                    *g = garde_corps;
                } else if self.identifiants().contains(&id) {
                    return Err("Identifiant déjà utilisé.".into());
                } else {
                    self.garde_corps.push(garde_corps);
                }
                Ok(vec![id])
            }
            Commande::Piece { mut piece } => {
                self.niveau(&piece.niveau).ok_or("Pièce : niveau introuvable.")?;
                if !piece.point.iter().all(|x| x.is_finite()) {
                    return Err("Pièce : point non valide.".into());
                }
                if piece.id.is_empty() {
                    piece.id = identite(self, "piece");
                    piece.contour = crate::modele::elements::piece::detecter(self, &piece.niveau, piece.point)?;
                }
                if piece.numero.trim().is_empty() {
                    piece.numero = self.numero_piece_suivant();
                }
                if piece.nom.trim().is_empty() {
                    piece.nom = "Pièce".into();
                }
                let id = piece.id.clone();
                if let Some(x) = self.pieces.iter_mut().find(|x| x.id == id) {
                    *x = piece;
                } else if self.identifiants().contains(&id) {
                    return Err("Identifiant déjà utilisé.".into());
                } else {
                    self.pieces.push(piece);
                }
                Ok(vec![id])
            }
            Commande::TypeComposant { mut type_composant } => {
                if type_composant.nom.trim().is_empty() || ![type_composant.largeur, type_composant.profondeur, type_composant.hauteur].iter().all(|x| x.is_finite() && *x > 0.0) {
                    return Err("Type de composant : nom et dimensions positives requis.".into());
                }
                if type_composant.id.is_empty() {
                    type_composant.id = identite(self, "type-composant");
                }
                let id = type_composant.id.clone();
                match self.types_composants.iter_mut().find(|t| t.id == id) {
                    Some(t) => *t = type_composant,
                    None => self.types_composants.push(type_composant),
                }
                Ok(vec![id])
            }
            Commande::Arbre { mut arbre } => {
                self.niveau(&arbre.niveau).ok_or("Arbre : niveau introuvable.")?;
                if !arbre.position.iter().chain([&arbre.decalage, &arbre.hauteur, &arbre.diametre]).all(|x| x.is_finite()) || arbre.hauteur <= 0.0 || arbre.diametre <= 0.0 {
                    return Err("Arbre : position ou dimensions non valides.".into());
                }
                if arbre.nom.trim().is_empty() {
                    arbre.nom = arbre.essence.nom().into();
                }
                if arbre.id.is_empty() {
                    arbre.id = identite(self, "arbre");
                }
                let id = arbre.id.clone();
                if let Some(x) = self.arbres.iter_mut().find(|x| x.id == id) {
                    *x = arbre;
                } else if self.identifiants().contains(&id) {
                    return Err("Identifiant déjà utilisé.".into());
                } else {
                    self.arbres.push(arbre);
                }
                Ok(vec![id])
            }
            Commande::Composant { mut composant } => {
                if composant.type_composant.is_empty() {
                    let defaut = crate::modele::elements::composant::type_table();
                    if !self.types_composants.iter().any(|t| t.id == defaut.id) {
                        self.types_composants.push(defaut.clone());
                    }
                    composant.type_composant = defaut.id;
                }
                let t = self.types_composants.iter().find(|t| t.id == composant.type_composant).ok_or("Type de composant introuvable.")?;
                if composant.nom.trim().is_empty() {
                    composant.nom = t.nom.clone();
                }
                self.niveau(&composant.niveau).ok_or("Composant : niveau introuvable.")?;
                if !composant.position.iter().chain([&composant.decalage, &composant.rotation]).all(|x| x.is_finite()) {
                    return Err("Composant : position non valide.".into());
                }
                if composant.id.is_empty() {
                    composant.id = identite(self, "composant");
                }
                let id = composant.id.clone();
                if let Some(x) = self.composants.iter_mut().find(|x| x.id == id) {
                    *x = composant;
                } else if self.identifiants().contains(&id) {
                    return Err("Identifiant déjà utilisé.".into());
                } else {
                    self.composants.push(composant);
                }
                Ok(vec![id])
            }
            Commande::Barre { mut barre } => {
                if barre.id.is_empty() {
                    barre.id = identite(self, "structure");
                }
                let id = barre.id.clone();
                if let Some(b) = self.barres.iter_mut().find(|b| b.id == id) {
                    *b = barre;
                } else if self.identifiants().contains(&id) {
                    return Err("Identifiant déjà utilisé.".into());
                } else {
                    self.barres.push(barre);
                }
                Ok(vec![id])
            }
            Commande::Quadrillage { nom, debut, fin } => {
                let id = identite(self, "quadrillage");
                self.quadrillages.push(documentation::Quadrillage {
                    id: id.clone(),
                    nom,
                    debut,
                    fin,
                });
                Ok(vec![id])
            }
            Commande::Annoter { mut annotation } => {
                if annotation.id.is_empty() {
                    annotation.id = identite(self, "annotation");
                }
                let id = annotation.id.clone();
                if let Some(a) = self.annotations.iter_mut().find(|a| a.id == id) {
                    *a = annotation;
                } else {
                    self.annotations.push(annotation);
                }
                Ok(vec![id])
            }
            Commande::Feuille { mut feuille } => {
                if feuille.id.is_empty() {
                    feuille.id = identite(self, "feuille");
                }
                let id = feuille.id.clone();
                if let Some(a) = self.feuilles.iter_mut().find(|a| a.id == id) {
                    *a = feuille;
                } else {
                    self.feuilles.push(feuille);
                }
                Ok(vec![id])
            }
            Commande::Nomenclature { mut nomenclature } => {
                if nomenclature.id.is_empty() {
                    nomenclature.id = identite(self, "nomenclature");
                }
                let id = nomenclature.id.clone();
                if let Some(a) = self.nomenclatures.iter_mut().find(|a| a.id == id) {
                    *a = nomenclature;
                } else {
                    self.nomenclatures.push(nomenclature);
                }
                Ok(vec![id])
            }
            Commande::Niveau { nom, altitude } => {
                if nom.trim().is_empty()
                    || !altitude.is_finite()
                    || self
                        .niveaux
                        .iter()
                        .any(|n| n.nom.eq_ignore_ascii_case(nom.trim()))
                {
                    return Err("Nom de niveau vide/déjà utilisé ou altitude non valide.".into());
                }
                let id = identite(self, "niveau");
                self.niveaux.push(Niveau {
                    id: id.clone(),
                    nom: nom.trim().into(),
                    altitude,
                    origine: None,
                });
                Ok(vec![id])
            }
            Commande::ModifierNiveau { id, nom, altitude } => {
                if nom.trim().is_empty()
                    || !altitude.is_finite()
                    || self
                        .niveaux
                        .iter()
                        .any(|n| n.id != id && n.nom.eq_ignore_ascii_case(nom.trim()))
                {
                    return Err("Nom ou altitude non valide.".into());
                }
                let n = self
                    .niveaux
                    .iter_mut()
                    .find(|n| n.id == id)
                    .ok_or("Niveau absent.")?;
                n.nom = nom.trim().into();
                n.altitude = altitude;
                Ok(vec![id])
            }
            Commande::Ouverture {
                mur,
                genre,
                type_ouverture,
                abscisse,
                largeur,
                hauteur,
                allege,
            } => {
                let (axe, _, _) = solides::hote(self, &mur)?;
                // un type impose ses cotes
                let type_choisi = type_ouverture.as_ref().map(|t| self.types_ouvertures.iter().find(|x| &x.id == t).ok_or("Type d'ouverture absent.")).transpose()?;
                let (largeur, hauteur) = type_choisi.map_or((largeur, hauteur), |t| (t.largeur, t.hauteur));
                let nom_type = type_choisi.map(|t| t.nom.clone());
                if ![abscisse, largeur, hauteur, allege]
                    .iter()
                    .all(|x| x.is_finite())
                    || largeur <= 0.0
                    || hauteur <= 0.0
                    || abscisse - largeur / 2.0 < 0.0
                    || abscisse + largeur / 2.0 > axe.longueur()
                {
                    return Err("L’ouverture doit tenir dans la longueur du mur, avec des dimensions positives.".into());
                }
                let id = identite(self, "ouverture");
                let nom = nom_type.unwrap_or_else(|| match genre {
                    GenreOuverture::Porte => "Porte",
                    GenreOuverture::Fenetre => "Fenêtre",
                    GenreOuverture::Libre => "Ouverture libre",
                }
                .into());
                self.ouvertures.push(Ouverture {
                    id: id.clone(),
                    nom,
                    genre,
                    mur,
                    type_ouverture,
                    abscisse,
                    largeur,
                    hauteur,
                    allege,
                    representation: None,
                    valeurs: Default::default(),
            materiaux: Default::default(),
                    retournee: false,
                    inversee: false,
                    origine: None,
                });
                Ok(vec![id])
            }
            Commande::Attacher { ids, toiture } => {
                for id in &ids {
                    if !self.murs.iter().any(|m| m.id == *id) && self.assemblage(id).is_none() {
                        return Err("Sélectionnez des murs ou assemblages.".into());
                    }
                    if let Some(t) = &toiture {
                        self.attaches_toitures.insert(id.clone(), t.clone());
                    } else {
                        self.attaches_toitures.remove(id);
                    }
                }
                Ok(ids)
            }
            Commande::PrioriteJonction { id, priorite } => {
                if !self.murs.iter().any(|m| m.id == id) {
                    return Err("Sélectionnez une couche ou un mur indépendant.".into());
                }
                self.priorites_jonction.insert(id.clone(), priorite);
                Ok(vec![id])
            }
            Commande::ActualiserType { assemblage } => {
                let a = self
                    .assemblage(&assemblage)
                    .cloned()
                    .ok_or("Assemblage absent.")?;
                let t = self.type_mur(&a.type_mur).cloned().ok_or("Type absent.")?;
                let anciens: Vec<Mur> = self
                    .couches_assemblage(&a.id)
                    .into_iter()
                    .cloned()
                    .collect();
                let premier = anciens.first().ok_or("Assemblage vide.")?;
                let (axe, _, _) = crate::modele::volumes::solides::hote(self, &a.id)?;
                if t.couches.is_empty() { return Err("Le type de mur doit contenir au moins une couche.".into()); }
                // Une ouverture hébergée par une couche retirée est rattachée à la
                // dernière couche conservée, sans l'étendre à tout l'assemblage.
                let supprimees: std::collections::HashSet<&str> = anciens.iter().skip(t.couches.len()).map(|m| m.id.as_str()).collect();
                let nouvel_hote = &anciens[t.couches.len().min(anciens.len()) - 1].id;
                for ouverture in &mut self.ouvertures {
                    if supprimees.contains(ouverture.mur.as_str()) { ouverture.mur = nouvel_hote.clone(); }
                }
                let mut nouveaux = Vec::new();
                for (k, (c, d)) in t.couches.iter().zip(t.decalages_couches()).enumerate() {
                    let mut m = anciens.get(k).unwrap_or(premier).clone();
                    m.id = anciens
                        .get(k)
                        .map_or_else(crate::modele::collaboration::uuid, |m| m.id.clone());
                    if k >= anciens.len() && self.identifiants().contains(&m.id) {
                        return Err("Identifiant de couche déjà utilisé.".into());
                    }
                    m.type_mur = self.type_couche(c);
                    m.axe = assemblage::decaler_axe(&axe, d);
                    m.assemblage = Some(a.id.clone());
                    nouveaux.push(m);
                }
                self.murs.retain(|m| m.assemblage.as_ref() != Some(&a.id));
                self.assemblages
                    .iter_mut()
                    .find(|x| x.id == a.id)
                    .unwrap()
                    .couches = nouveaux.iter().map(|m| m.id.clone()).collect();
                self.murs.extend(nouveaux);
                self.completer_materiaux();
                Ok(vec![a.id])
            }
            Commande::Aligner { ids, reference } => {
                let (a, b, _) = self
                    .axe_element(&reference)
                    .ok_or("Référence : mur droit requis.")?;
                let a = DVec2::from_array(a);
                let u = (DVec2::from_array(b) - a).normalize();
                for id in &ids {
                    if *id == reference {
                        continue;
                    }
                    let (x, y, _) = self
                        .axe_element(id)
                        .ok_or("Alignement : mur droit requis.")?;
                    let x = DVec2::from_array(x);
                    let v = (DVec2::from_array(y) - x).normalize();
                    let angle = v.perp_dot(u).atan2(v.dot(u)).to_degrees();
                    let set = self.dependances(std::slice::from_ref(id))?;
                    self.transformer(&set, [0.0; 3], x.to_array(), angle)?;
                    let d = u.perp() * (a - x).dot(u.perp());
                    self.transformer(&set, [d.x, d.y, 0.0], [0.0; 2], 0.0)?;
                }
                Ok(ids)
            }
            Commande::InverserMur { id } => {
                self.inverser_mur(&id)?;
                Ok(vec![id])
            }
            Commande::ModifierTrajectoireMur{id,points}=>{
                self.modifier_trajectoire_courbe(&id,points)?;
                Ok(vec![id])
            }
            Commande::AjusterAngle { id, reference, fin, fin_reference } => {
                if id == reference { return Err("Choisissez un mur différent de la limite.".into()); }
                let (ra, rb) = self.ligne_ajustable(&reference).ok_or("Référence droite requise.")?;
                let (ia, ib) = self.ligne_ajustable(&id).ok_or("Mur droit ou poutre requis.")?;
                let point = self.cible_justification(&id, [ra, rb], fin)?;
                self.cible_justification(&reference, [ia, ib], fin_reference)?;
                let poutre = |p: &Projet, x: &str| p.barres.iter().any(|b| b.id == x);
                if !poutre(self, &id) && !poutre(self, &reference) {
                    let z1 = self.axe_element(&id).ok_or("Mur absent")?.2;
                    let z2 = self.axe_element(&reference).ok_or("Référence absente")?.2;
                    if (z1-z2).abs()>1e-6 { return Err("Les murs doivent être au même niveau pour former un angle.".into()); }
                }
                self.deplacer_extremite_ajustable(&id, fin, point)?;
                self.deplacer_extremite_ajustable(&reference, fin_reference, point)?;
                Ok(vec![id, reference])
            }
            Commande::Ajuster { id, reference, fin, ligne } => {
                if id == reference && ligne.is_none() { return Err("Choisissez un mur différent de la limite.".into()); }
                let ligne = match ligne {
                    Some(l) => l,
                    None => { let (a, b) = self.ligne_ajustable(&reference).ok_or("Référence droite requise.")?; [a, b] }
                };
                let point = self.cible_justification(&id, ligne, fin)?;
                self.deplacer_extremite_ajustable(&id, fin, point)?;
                Ok(vec![id])
            }
            Commande::Deplacer { ids, delta } => {
                let set = self.dependances_avec(&ids, true)?;
                self.transformer(&set, delta, [0.0; 2], 0.0)?;
                Ok(ids)
            }
            Commande::Tourner { ids, centre, angle } => {
                let set = self.dependances_avec(&ids, true)?;
                self.transformer(&set, [0.0; 3], centre, angle)?;
                Ok(ids)
            }
            Commande::CopierSurNiveau {ids,niveau} => {
                let cible=self.niveau(&niveau).ok_or("Niveau de destination introuvable")?.altitude;
                for id in &ids {
                    let admis=self.murs.iter().any(|x|&x.id==id) || self.assemblage(id).is_some()
                        || self.dalles.iter().any(|x|&x.id==id) || self.barres.iter().any(|x|&x.id==id)
                        || self.composants.iter().any(|x|&x.id==id) || self.pieces.iter().any(|x|&x.id==id)
                        || self.escaliers.iter().any(|x|&x.id==id) || self.garde_corps.iter().any(|x|&x.id==id)
                        || self.importes.iter().any(|x|&x.id==id && x.niveau.is_some());
                    if !admis {return Err("La sélection contient un élément qui ne peut pas être copié sur un niveau ; les ouvertures suivent leur mur hôte.".into());}
                }
                let niveaux=self.niveaux.clone();
                let altitude=|id:&str|niveaux.iter().find(|n|n.id==id).map(|n|n.altitude).ok_or("Niveau d'origine introuvable".to_string());
                let copies=self.executer_interne(Commande::Copier{ids,delta:[0.;3]})?;
                let ensemble=self.dependances(&copies)?;
                for m in self.murs.iter_mut().filter(|m|ensemble.contains(&m.id)) {
                    let origine=altitude(&m.niveau)?;
                    if let Haut::Niveau{niveau:haut,decalage}=&mut m.haut {
                        let ancien=altitude(haut)?;let voulu=cible+(ancien-origine);
                        if let Some(n)=niveaux.iter().find(|n|(n.altitude-voulu).abs()<0.001) {*haut=n.id.clone();}
                        else {*decalage+=cible-origine;}
                    }
                    m.niveau=niveau.clone();
                }
                for d in self.dalles.iter_mut().filter(|d|ensemble.contains(&d.id)) {d.niveau=niveau.clone();}
                for b in self.barres.iter_mut().filter(|b|ensemble.contains(&b.id)) {b.niveau=niveau.clone();}
                for b in self.composants.iter_mut().filter(|b|ensemble.contains(&b.id)) {b.niveau=niveau.clone();}
                for b in self.pieces.iter_mut().filter(|b|ensemble.contains(&b.id)) {b.niveau=niveau.clone();}
                for e in self.escaliers.iter_mut().filter(|e|ensemble.contains(&e.id)) {
                    let origine=altitude(&e.niveau)?;
                    if let Some(haut)=&e.niveau_haut {
                        let ancien=altitude(haut)?;let voulu=cible+(ancien-origine);
                        e.niveau_haut=niveaux.iter().find(|n|(n.altitude-voulu).abs()<0.001).map(|n|n.id.clone());
                        if e.niveau_haut.is_none() {e.hauteur=ancien-origine-e.decalage_bas;}
                    }
                    e.niveau=niveau.clone();
                }
                for g in self.garde_corps.iter_mut().filter(|g|ensemble.contains(&g.id)) {g.niveau=niveau.clone();}
                for e in self.importes.iter_mut().filter(|e|ensemble.contains(&e.id)) {
                    let ancien=e.niveau.as_deref().ok_or("Un élément importé n'a pas de niveau")?;
                    let dz=cible-altitude(ancien)?;
                    for p in &mut e.maillage.sommets {p[2]+=dz;}
                    e.niveau=Some(niveau.clone());
                }
                self.arrondir();
                Ok(copies)
            }
            Commande::Copier { ids, delta } => {
                let set = self.dependances(&ids)?;
                if self.niveaux.iter().any(|n| set.contains(&n.id))
                    || self
                        .vues
                        .iter()
                        .any(|n| set.contains(&n.id) && matches!(n.genre, GenreVue::Plan { .. }))
                    || self.feuilles.iter().any(|n| set.contains(&n.id))
                    || self.nomenclatures.iter().any(|n| set.contains(&n.id))
                {
                    return Err("Copie réservée aux éléments du modèle.".into());
                }
                if self
                    .ouvertures
                    .iter()
                    .any(|o| set.contains(&o.id) && !set.contains(&o.mur))
                {
                    return Err(
                        "Copiez le mur hôte avec son ouverture, ou placez une nouvelle ouverture."
                            .into(),
                    );
                }
                let mut map = HashMap::new();
                let mut connus = self.identifiants();
                let mut ordre: Vec<_> = set.iter().collect();
                ordre.sort();
                for old in ordre {
                    let id = (1u64..)
                        .map(|n| format!("copie-{n}"))
                        .find(|n| !connus.contains(n))
                        .unwrap();
                    connus.insert(id.clone());
                    map.insert(old.clone(), id);
                }
                let mut murs = Vec::new();
                for x in &self.murs {
                    if set.contains(&x.id) {
                        let mut x = x.clone();
                        x.id = map[&x.id].clone();
                        x.origine = None;
                        x.assemblage = x.assemblage.as_ref().map(|a| map[a].clone());
                        murs.push(x);
                    }
                }
                let mut assems = Vec::new();
                for x in &self.assemblages {
                    if set.contains(&x.id) {
                        let mut x = x.clone();
                        x.id = map[&x.id].clone();
                        x.origine = None;
                        x.couches = x.couches.iter().map(|id| map[id].clone()).collect();
                        assems.push(x);
                    }
                }
                let mut baies = Vec::new();
                for x in &self.ouvertures {
                    if set.contains(&x.id) {
                        let mut x = x.clone();
                        x.id = map[&x.id].clone();
                        x.mur = map[&x.mur].clone();
                        x.origine = None;
                        baies.push(x);
                    }
                }
                let mut dalles = Vec::new();
                for x in &self.dalles {
                    if set.contains(&x.id) {
                        let mut x = x.clone();
                        x.id = map[&x.id].clone();
                        x.origine = None;
                        dalles.push(x);
                    }
                }
                let mut imports = Vec::new();
                for x in &self.importes {
                    if set.contains(&x.id) {
                        let mut x = x.clone();
                        x.id = map[&x.id].clone();
                        x.origine = None;
                        imports.push(x);
                    }
                }
                let barres: Vec<_> = self
                    .barres
                    .iter()
                    .filter(|b| set.contains(&b.id))
                    .map(|b| {
                        let mut b = b.clone();
                        b.id = map[&b.id].clone();
                        b
                    })
                    .collect();
                let vues: Vec<_> = self
                    .vues
                    .iter()
                    .filter(|v| set.contains(&v.id))
                    .map(|v| {
                        let mut v = v.clone();
                        v.id = map[&v.id].clone();
                        v.nom = format!("{} (copie)", v.nom);
                        v
                    })
                    .collect();
                self.vues.extend(vues);
                let annotations: Vec<_> = self
                    .annotations
                    .iter()
                    .filter(|a| map.contains_key(&a.vue) && !set.contains(&a.id))
                    .cloned()
                    .collect();
                for mut annotation in annotations {
                    annotation.id = identite(self, "annotation");
                    annotation.vue = map[&annotation.vue].clone();
                    remapper_annotation(&mut annotation, &map);
                    self.annotations.push(annotation);
                }
                let annotations: Vec<_> = self
                    .annotations
                    .iter()
                    .filter(|a| set.contains(&a.id))
                    .cloned()
                    .collect();
                for mut annotation in annotations {
                    annotation.id = map[&annotation.id].clone();
                    if let Some(id) = map.get(&annotation.vue) {
                        annotation.vue = id.clone();
                    }
                    remapper_annotation(&mut annotation, &map);
                    self.annotations.push(annotation);
                }
                let quadrillages: Vec<_> = self
                    .quadrillages
                    .iter()
                    .filter(|g| set.contains(&g.id))
                    .map(|g| {
                        let mut g = g.clone();
                        g.id = map[&g.id].clone();
                        g.nom = format!("{} (copie)", g.nom);
                        g
                    })
                    .collect();
                self.quadrillages.extend(quadrillages);
                let contextes: Vec<_> = self
                    .contextes
                    .iter()
                    .filter_map(|(id, c)| map.get(id).map(|nouveau| (nouveau.clone(), c.clone())))
                    .collect();
                self.contextes.extend(contextes);
                self.barres.extend(barres);
                let pieces: Vec<_> = self.pieces.iter().filter(|x| set.contains(&x.id)).map(|x| Piece { id: map[&x.id].clone(), ..x.clone() }).collect();
                for mut x in pieces {
                    x.numero = self.numero_piece_suivant();
                    self.pieces.push(x);
                }
                let composants: Vec<_> = self.composants.iter().filter(|x| set.contains(&x.id)).map(|x| Composant { id: map[&x.id].clone(), ..x.clone() }).collect();
                self.composants.extend(composants);
                let escaliers: Vec<_> = self.escaliers.iter().filter(|e| set.contains(&e.id)).map(|e| Escalier { id: map[&e.id].clone(), ..e.clone() }).collect();
                let garde_corps: Vec<_> = self
                    .garde_corps
                    .iter()
                    .filter(|g| set.contains(&g.id))
                    .map(|g| GardeCorps { id: map[&g.id].clone(), escalier: g.escalier.as_ref().map(|e| map.get(e).unwrap_or(e).clone()), ..g.clone() })
                    .collect();
                self.escaliers.extend(escaliers);
                self.garde_corps.extend(garde_corps);
                self.murs.extend(murs);
                self.assemblages.extend(assems);
                self.ouvertures.extend(baies);
                self.dalles.extend(dalles);
                let terrains: Vec<_> = self.terrains.iter().filter(|t| set.contains(&t.id)).map(|t| { let mut t = t.clone(); t.id = map[&t.id].clone(); t }).collect();
                let limites: Vec<_> = self.limites_propriete.iter().filter(|l| set.contains(&l.id)).map(|l| { let mut l = l.clone(); l.id = map[&l.id].clone(); l }).collect();
                self.terrains.extend(terrains);
                self.limites_propriete.extend(limites);
                self.importes.extend(imports);
                let attaches: Vec<_> = self
                    .attaches_toitures
                    .iter()
                    .filter(|(id, _)| map.contains_key(*id))
                    .map(|(id, t)| (map[id].clone(), map.get(t).unwrap_or(t).clone()))
                    .collect();
                self.attaches_toitures.extend(attaches);
                let priorites: Vec<_> = self
                    .priorites_jonction
                    .iter()
                    .filter(|(id, _)| map.contains_key(*id))
                    .map(|(id, n)| (map[id].clone(), *n))
                    .collect();
                self.priorites_jonction.extend(priorites);
                let implantations: Vec<_> = self.implantations.iter().filter(|(id, _)| map.contains_key(*id)).map(|(id, r)| (map[id].clone(), *r)).collect();
                self.implantations.extend(implantations);
                self.transformer(&map.values().cloned().collect(), delta, [0.0; 2], 0.0)?;
                Ok(ids.iter().map(|id| map[id].clone()).collect())
            }
            Commande::Symetrie { ids, a, b } => self.symetrie(ids, a, b),
            Commande::Echelle { ids, centre, facteur } => self.echelle(ids, centre, facteur),
            Commande::Reseau { ids, nombre, delta, centre, angle } => self.reseau(ids, nombre, delta, centre, angle),
            Commande::Decaler { id, distance, copie } => self.decaler(id, distance, copie),
            Commande::Scinder { id, point, ecart } => self.scinder(id, point, ecart),
            Commande::Demolir { ids, phase } => self.demolir(ids, phase),
            Commande::Reproduire { source, cibles } => self.reproduire(source, cibles),
            Commande::ChangerHote { id, mur, point } => self.changer_hote(id, mur, point),
            Commande::InverserOuverture { id, paumelles } => self.inverser_ouverture(id, paumelles),
            Commande::Verrouiller { ids, verrou } => {
                let connus = self.identifiants();
                for id in &ids {
                    if !connus.contains(id) {
                        return Err(format!("Élément introuvable : {id}"));
                    }
                    if verrou {
                        self.verrous.insert(id.clone(), true);
                    } else {
                        self.verrous.remove(id);
                    }
                }
                Ok(ids)
            }
            Commande::Supprimer { ids } => {
                let set = self.dependances(&ids)?;
                for n in &self.niveaux {
                    if set.contains(&n.id)
                        && (self.murs.iter().any(|m| {
                            m.niveau == n.id
                                || matches!(&m.haut,Haut::Niveau {niveau,..} if niveau==&n.id)
                        }) || self.dalles.iter().any(|d| d.niveau == n.id)
                            || self.barres.iter().any(|b| b.niveau == n.id)
                            || self.pieces.iter().any(|b| b.niveau == n.id)
                            || self.composants.iter().any(|b| b.niveau == n.id)
                            || self.arbres.iter().any(|b| b.niveau == n.id)
                            || self.escaliers.iter().any(|e| e.niveau == n.id || e.niveau_haut.as_ref() == Some(&n.id))
                            || self.garde_corps.iter().any(|g| g.niveau == n.id)
                            || self
                                .importes
                                .iter()
                                .any(|i| i.niveau.as_ref() == Some(&n.id))
                            || self.vues.iter().any(
                                |v| matches!(&v.genre,GenreVue::Plan {niveau,bas_niveau,..} if niveau==&n.id || bas_niveau==&n.id) || v.limite_haute_niveau.as_ref()==Some(&n.id),
                            ))
                    {
                        return Err("Ce niveau est encore utilisé.".into());
                    }
                }
                self.murs.retain(|x| !set.contains(&x.id));
                self.assemblages.retain(|x| !set.contains(&x.id));
                self.ouvertures.retain(|x| !set.contains(&x.id));
                self.dalles.retain(|x| !set.contains(&x.id));
                self.barres.retain(|x| !set.contains(&x.id));
                self.pieces.retain(|x| !set.contains(&x.id));
                self.composants.retain(|x| !set.contains(&x.id));
                self.arbres.retain(|x| !set.contains(&x.id));
                self.escaliers.retain(|x| !set.contains(&x.id));
                self.garde_corps.retain(|x| !set.contains(&x.id));
                self.importes.retain(|x| !set.contains(&x.id));
                self.terrains.retain(|x| !set.contains(&x.id));
                self.limites_propriete.retain(|x| !set.contains(&x.id));
                self.niveaux.retain(|x| !set.contains(&x.id));
                self.vues.retain(|x| !set.contains(&x.id));
                for vue in &mut self.vues { vue.elements_masques.retain(|id| !set.contains(id)); }
                self.attaches_toitures
                    .retain(|id, t| !set.contains(id) && !set.contains(t));
                self.priorites_jonction.retain(|id, _| !set.contains(id));
                self.implantations.retain(|id, _| !set.contains(id));
                self.contextes.retain(|id, _| !set.contains(id));
                self.annotations
                    .retain(|x| !set.contains(&x.id) && !set.contains(&x.vue));
                self.quadrillages.retain(|x| !set.contains(&x.id));
                self.feuilles.retain(|x| !set.contains(&x.id));
                self.nomenclatures.retain(|x| !set.contains(&x.id));
                for f in &mut self.feuilles {
                    f.vues.retain(|v| !set.contains(&v.vue));
                    f.nomenclatures.retain(|n| !set.contains(&n.nomenclature));
                }
                Ok(Vec::new())
            }
            Commande::Contour { id, contours } => {
                if contours.is_empty() {
                    return Err("Contour vide.".into());
                }
                let contours = contours
                    .iter()
                    .map(|c| edition::contour_valide(c))
                    .collect::<Result<Vec<_>, _>>()?;
                for i in 0..contours.len() {
                    for j in 0..i {
                        if croisement(&contours[i], &contours[j]) {
                            return Err("Les contours se croisent ou se touchent.".into());
                        }
                    }
                }
                let d = self
                    .dalles
                    .iter_mut()
                    .find(|d| d.id == id)
                    .ok_or("Dalle absente.")?;
                if d.contours != contours {
                    let pentes: Vec<Vec<Option<f64>>> = contours.iter().map(|c| {
                        (0..c.len()).map(|i| {
                            let (a, b) = (c[i], c[(i + 1) % c.len()]);
                            d.contours.iter().enumerate().find_map(|(ci, ancien)| {
                                (0..ancien.len()).find_map(|k| {
                                    (ancien[k] == a && ancien[(k + 1) % ancien.len()] == b)
                                        .then(|| d.pente(ci, k)).flatten()
                                })
                            })
                        }).collect()
                    }).collect();
                    d.contours = contours;
                    d.pentes = pentes;
                }
                Ok(vec![id])
            }
            Commande::Tremie { id, contour } => {
                let c = edition::contour_valide(&contour)?;
                let d = self
                    .dalles
                    .iter_mut()
                    .find(|d| d.id == id)
                    .ok_or("Dalle absente.")?;
                // La trémie est strictement intérieure et ne croise aucun contour existant.
                if c.iter().any(|&p| {
                    d.contours
                        .iter()
                        .filter(|c| geometrie::dedans(p, c))
                        .count()
                        % 2
                        == 0
                }) {
                    return Err("La trémie doit être à l'intérieur de la dalle.".into());
                }
                for existant in &d.contours {
                    if existant.iter().any(|&p| geometrie::dedans(p, &c))
                        || croisement(existant, &c)
                    {
                        return Err("La trémie touche ou recouvre un contour existant.".into());
                    }
                }
                d.contours.push(c);
                Ok(vec![id])
            }
        }
    }
}
fn remapper_annotation(annotation: &mut documentation::Annotation, ids: &HashMap<String, String>) {
    use documentation::{AnnotationForme as F, PointCote};
    match &mut annotation.forme {
        F::Etiquette { element, .. } => {
            if let Some(id) = ids.get(element) {
                *element = id.clone();
            }
        }
        F::Cote { debut, fin, .. } => {
            for p in [debut, fin] {
                if let PointCote::Mur { id, .. } = p {
                    if let Some(nouveau) = ids.get(id) {
                        *id = nouveau.clone();
                    }
                }
            }
        }
        _ => {}
    }
}
pub fn croisement(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
    for i in 0..a.len() {
        for j in 0..b.len() {
            let (p, q, r, s) = (
                DVec2::from_array(a[i]),
                DVec2::from_array(a[(i + 1) % a.len()]),
                DVec2::from_array(b[j]),
                DVec2::from_array(b[(j + 1) % b.len()]),
            );
            let orient = |p: DVec2, q: DVec2, r: DVec2| (q - p).perp_dot(r - p);
            let overlap = p.min(q).cmple(r.max(s)).all() && r.min(s).cmple(p.max(q)).all();
            if overlap
                && orient(p, q, r) * orient(p, q, s) <= 0.0
                && orient(r, s, p) * orient(r, s, q) <= 0.0
            {
                return true;
            }
        }
    }
    false
}

