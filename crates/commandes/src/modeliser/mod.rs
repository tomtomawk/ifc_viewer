//! Modéliser : commandes qui créent ou remplacent un élément du modèle (site, escaliers,
//! garde-corps, pièces, composants, arbres, barres, ouvertures, type de mur composé,
//! contours et trémies de dalle).
use crate::socle::interne::*;
use fusion::ExecutionFusion;

mod fusion;

/// Exécution des commandes de modélisation.
pub(crate) trait ExecutionModeliser {
    fn executer_modeliser(&mut self, c: Commande) -> Result<Vec<String>, String>;
}

impl ExecutionModeliser for Projet {
    fn executer_modeliser(&mut self, c: Commande) -> Result<Vec<String>, String> {
        match c {
            Commande::Fusionner { projet, decalage } => self.fusionner(*projet, decalage),
            // éléments du site : créés dans la phase Existant
            Commande::Terrain { mut terrain } => {
                terrain.triangles()?;
                // type par défaut choisi dans un projet qui n'a pas encore de types de terrain
                if terrain.type_terrain.as_ref().is_some_and(|t| self.types_terrains.iter().all(|x| &x.id != t)) {
                    self.fixer_types_terrains();
                }
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
            // lien IFC : hors phases et sous-projets, toujours affiché s'il est visible
            Commande::Lien { mut lien } => {
                lien.verifier()?;
                if lien.id.is_empty() { lien.id = identite(self, "lien"); }
                let id = lien.id.clone();
                if let Some(l) = self.liens.iter_mut().find(|l| l.id == id) { *l = lien; } else { self.liens.push(lien); }
                Ok(vec![id])
            }
            Commande::CoordonneesOrigine { coordonnees } => {
                if !coordonnees.iter().all(|x| x.is_finite()) { return Err("Coordonnées de l'origine non valides.".into()); }
                self.coordonnees_origine = coordonnees;
                Ok(Vec::new())
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
                    piece.contour = modele::elements::piece::detecter(self, &piece.niveau, piece.point)?;
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
                    let defaut = modele::elements::composant::type_table();
                    if !self.types_composants.iter().any(|t| t.id == defaut.id) {
                        self.types_composants.push(defaut.clone());
                    }
                    composant.type_composant = defaut.id;
                } else if !self.types_composants.iter().any(|t| t.id == composant.type_composant)
                    && let Some(t) = modele::elements::composant::type_par_defaut(&composant.type_composant)
                {
                    // type livré avec le code (végétaux) : entre au projet au premier placement
                    self.types_composants.push(t);
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
                let (axe, _, _) = modele::volumes::solides::hote(self, &a.id)?;
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
                        .map_or_else(noyau::uuid::uuid, |m| m.id.clone());
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
            Commande::Contour { id, contours } => {
                if contours.is_empty() {
                    return Err("Contour vide.".into());
                }
                let contours = contours
                    .iter()
                    .map(|c| creation::contour_valide(c))
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
                    // remblai des fouilles : suit les arêtes conservées, comme les pentes
                    let mut remblais: Vec<Vec<bool>> = contours.iter().map(|c| {
                        (0..c.len()).map(|i| {
                            let (a, b) = (c[i], c[(i + 1) % c.len()]);
                            d.contours.iter().enumerate().any(|(ci, ancien)| {
                                (0..ancien.len()).any(|k| ancien[k] == a && ancien[(k + 1) % ancien.len()] == b && d.remblaye(ci, k))
                            })
                        }).collect()
                    }).collect();
                    if !remblais.iter().flatten().any(|r| *r) {
                        remblais.clear();
                    }
                    // terre-plein dont les contours gardent leurs sommets (trait déplacé,
                    // décalé) : talus et remblais restent sur leurs arêtes
                    let memes_aretes = d.sorte.talus() && contours.len() == d.contours.len() && contours.iter().zip(&d.contours).all(|(x, y)| x.len() == y.len());
                    d.contours = contours;
                    if !memes_aretes {
                        d.pentes = pentes;
                        d.remblais = remblais;
                    }
                }
                Ok(vec![id])
            }
            Commande::Tremie { id, contour } => {
                let c = creation::contour_valide(&contour)?;
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
            _ => unreachable!("commande hors du métier : la répartition est faite par socle/execution.rs"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Commande, Execution};
    use modele::liens::LienIfc;

    /// Lien IFC : création, remplacement (même identifiant), refus d'un lien non
    /// valide, suppression et verrouillage par les commandes communes.
    #[test]
    fn lien_cree_modifie_supprime() {
        let p = modele::Projet::nouveau("Essai");
        let lien = LienIfc { id: String::new(), nom: "Structure".into(), chemin: "/projets/structure.ifc".into(), decalage: [0.0; 3], rotation: 0.0, visible: true };
        let (p, ids) = p.executer(Commande::Lien { lien: lien.clone() }).unwrap();
        assert_eq!(p.liens.len(), 1);
        let id = ids[0].clone();
        let (p, _) = p.executer(Commande::Lien { lien: LienIfc { id: id.clone(), rotation: 90.0, visible: false, ..lien.clone() } }).unwrap();
        assert_eq!((p.liens.len(), p.liens[0].rotation, p.liens[0].visible), (1, 90.0, false));
        assert!(p.executer(Commande::Lien { lien: LienIfc { chemin: String::new(), ..lien } }).is_err());
        let (verrouille, _) = p.executer(Commande::Verrouiller { ids: vec![id.clone()], verrou: true }).unwrap();
        assert!(verrouille.executer(Commande::Supprimer { ids: vec![id.clone()] }).is_err(), "lien verrouillé");
        let (p, _) = p.executer(Commande::Supprimer { ids: vec![id] }).unwrap();
        assert!(p.liens.is_empty());
        // le JSON du pilote
        let c: Commande = serde_json::from_str(r#"{"action":"lien","lien":{"id":"","nom":"Fluides","chemin":"/f.ifc"}}"#).unwrap();
        assert_eq!(p.executer(c).unwrap().0.liens[0].nom, "Fluides");
    }
}
