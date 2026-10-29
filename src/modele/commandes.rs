//! Commandes d'édition sérialisables pour l'interface et le pilote MCP.
use super::*;
use glam::DVec2;
use std::collections::{HashMap, HashSet};
#[derive(Deserialize, Debug)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Commande {
    Barre {
        barre: Barre,
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
    Ajuster {
        id: String,
        reference: String,
        fin: bool,
    },
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
    Contour {
        id: String,
        contours: Vec<Vec<[f64; 2]>>,
    },
    Tremie {
        id: String,
        contour: Vec<[f64; 2]>,
    },
}
fn identite(p: &Projet, prefixe: &str) -> String {
    let ids = p.identifiants();
    (1u64..)
        .map(|n| format!("{prefixe}-{n}"))
        .find(|id| !ids.contains(id))
        .unwrap()
}
impl Projet {
    pub fn identifiants(&self) -> HashSet<String> {
        self.niveaux
            .iter()
            .map(|x| x.id.clone())
            .chain(self.murs.iter().map(|x| x.id.clone()))
            .chain(self.assemblages.iter().map(|x| x.id.clone()))
            .chain(self.dalles.iter().map(|x| x.id.clone()))
            .chain(self.barres.iter().map(|x| x.id.clone()))
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
        let ids = p.executer_interne(c)?;
        p.recaler_menuiseries(self)?;
        p.verifier()?;
        Ok((p, ids))
    }
    pub fn dependances(&self, ids: &[String]) -> Result<HashSet<String>, String> {
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
            if out.contains(&m.id) && m.assemblage.as_ref().is_some_and(|a| !out.contains(a)) {
                return Err("Sélectionnez l'assemblage entier ou dissociez la couche avant cette opération.".into());
            }
        }
        for o in &self.ouvertures {
            if out.contains(&o.mur) {
                out.insert(o.id.clone());
            }
        }
        Ok(out)
    }
    fn executer_interne(&mut self, c: Commande) -> Result<Vec<String>, String> {
        match c {
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
                let nom = match genre {
                    GenreOuverture::Porte => "Porte",
                    GenreOuverture::Fenetre => "Fenêtre",
                    GenreOuverture::Libre => "Ouverture libre",
                }
                .into();
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
                let (debut, fin, _) = self
                    .axe_element(&a.id)
                    .ok_or("Actualisation réservée aux assemblages droits.")?;
                if anciens
                    .iter()
                    .skip(t.couches.len())
                    .any(|m| self.ouvertures.iter().any(|o| o.mur == m.id))
                {
                    return Err("Une couche supprimée porte une ouverture ; réhébergez-la avant de changer le type.".into());
                }
                let axe = Axe::Droit { debut, fin };
                let mut nouveaux = Vec::new();
                for (k, (c, d)) in t.couches.iter().zip(t.decalages_couches()).enumerate() {
                    let mut m = anciens.get(k).unwrap_or(premier).clone();
                    m.id = anciens
                        .get(k)
                        .map_or_else(|| format!("{}-type-{k}", a.id), |m| m.id.clone());
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
            Commande::Ajuster { id, reference, fin } => {
                if id == reference {
                    return Err("Deux murs distincts sont requis.".into());
                }
                let (a, b, _) = self.axe_element(&id).ok_or("Mur droit requis.")?;
                let (c, d, _) = self
                    .axe_element(&reference)
                    .ok_or("Référence droite requise.")?;
                let (a, b, c, d) = (
                    DVec2::from_array(a),
                    DVec2::from_array(b),
                    DVec2::from_array(c),
                    DVec2::from_array(d),
                );
                let (u, v) = (b - a, d - c);
                let den = u.perp_dot(v);
                if den.abs() < 1e-9 * u.length() * v.length() {
                    return Err("Murs parallèles : pas d'intersection.".into());
                }
                let point = a + u * ((c - a).perp_dot(v) / den);
                self.deplacer_extremite(&id, fin, point.to_array())?;
                Ok(vec![id])
            }
            Commande::Deplacer { ids, delta } => {
                let set = self.dependances(&ids)?;
                self.transformer(&set, delta, [0.0; 2], 0.0)?;
                Ok(ids)
            }
            Commande::Tourner { ids, centre, angle } => {
                let set = self.dependances(&ids)?;
                self.transformer(&set, [0.0; 3], centre, angle)?;
                Ok(ids)
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
                self.murs.extend(murs);
                self.assemblages.extend(assems);
                self.ouvertures.extend(baies);
                self.dalles.extend(dalles);
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
                self.transformer(&map.values().cloned().collect(), delta, [0.0; 2], 0.0)?;
                Ok(ids.iter().map(|id| map[id].clone()).collect())
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
                            || self
                                .importes
                                .iter()
                                .any(|i| i.niveau.as_ref() == Some(&n.id))
                            || self.vues.iter().any(
                                |v| matches!(&v.genre,GenreVue::Plan {niveau,..} if niveau==&n.id),
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
                self.importes.retain(|x| !set.contains(&x.id));
                self.niveaux.retain(|x| !set.contains(&x.id));
                self.vues.retain(|x| !set.contains(&x.id));
                self.attaches_toitures
                    .retain(|id, t| !set.contains(id) && !set.contains(t));
                self.priorites_jonction.retain(|id, _| !set.contains(id));
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
                let d = self
                    .dalles
                    .iter_mut()
                    .find(|d| d.id == id)
                    .ok_or("Dalle absente.")?;
                d.contours = contours;
                d.pentes.clear();
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
    fn transformer(
        &mut self,
        set: &HashSet<String>,
        delta: [f64; 3],
        centre: [f64; 2],
        angle: f64,
    ) -> Result<(), String> {
        if !delta
            .iter()
            .chain(&centre)
            .chain([&angle])
            .all(|v| v.is_finite())
        {
            return Err("Transformation non finie.".into());
        }
        if self.niveaux.iter().any(|n| set.contains(&n.id))
            || self.vues.iter().any(|n| {
                set.contains(&n.id) && (matches!(n.genre, GenreVue::Plan { .. }) || delta[2] != 0.0)
            })
            || (delta[2] != 0.0 && self.quadrillages.iter().any(|n| set.contains(&n.id)))
            || self.feuilles.iter().any(|n| set.contains(&n.id))
            || self.nomenclatures.iter().any(|n| set.contains(&n.id))
        {
            return Err("Transformation réservée aux éléments du modèle.".into());
        }
        let a = angle.to_radians();
        let (s, c) = a.sin_cos();
        let o = DVec2::from_array(centre);
        let point = |p: [f64; 2]| {
            let p = DVec2::from_array(p) - o;
            (o + DVec2::new(c * p.x - s * p.y, s * p.x + c * p.y) + DVec2::new(delta[0], delta[1]))
                .to_array()
        };
        let mesh = |m: &mut Maillage| {
            for p in &mut m.sommets {
                let xy = point([p[0], p[1]]);
                *p = [xy[0], xy[1], p[2] + delta[2]];
            }
        };
        let directions: HashMap<String, DVec2> = self
            .vues
            .iter()
            .filter_map(|v| match &v.genre {
                GenreVue::Coupe { debut, fin, .. } => Some((
                    v.id.clone(),
                    (DVec2::from_array(*fin) - DVec2::from_array(*debut)).normalize_or_zero(),
                )),
                _ => None,
            })
            .collect();
        for a in &mut self.annotations {
            if !set.contains(&a.id) || set.contains(&a.vue) {
                continue;
            }
            let direction = directions.get(&a.vue);
            let translation_locale = if let Some(u) = direction {
                let d = DVec2::new(delta[0], delta[1]);
                if angle != 0.0 || d.dot(u.perp()).abs() > 1e-6 {
                    return Err("Déplacez les annotations de coupe dans le plan de leur vue ; leur rotation doit être modifiée dans cette vue.".into());
                }
                Some([d.dot(*u), delta[2]])
            } else {
                if delta[2] != 0.0 {
                    return Err("Une annotation de plan doit rester dans son plan de vue.".into());
                }
                None
            };
            let point_annotation = |p: [f64; 2]| {
                if let Some(d) = translation_locale {
                    [p[0] + d[0], p[1] + d[1]]
                } else {
                    point(p)
                }
            };
            use documentation::{AnnotationForme as F, PointCote};
            match &mut a.forme {
                F::Texte { point: p, .. } | F::Etiquette { point: p, .. } => {
                    *p = point_annotation(*p)
                }
                F::Ligne { debut, fin } => {
                    *debut = point_annotation(*debut);
                    *fin = point_annotation(*fin);
                }
                F::Zone { contour } => {
                    for p in contour {
                        *p = point_annotation(*p);
                    }
                }
                F::Cote { debut, fin, .. } => {
                    for p in [debut, fin] {
                        match p {
                            PointCote::Fixe { point: p } => *p = point_annotation(*p),
                            PointCote::Mur { id, .. } if set.contains(id) => {},
                            PointCote::Mur { .. } => return Err("Sélectionnez le mur lié avec sa cote, ou modifiez le décalage dans les propriétés.".into()),
                        }
                    }
                }
            }
        }
        for g in &mut self.quadrillages {
            if set.contains(&g.id) {
                g.debut = point(g.debut);
                g.fin = point(g.fin);
            }
        }
        for v in &mut self.vues {
            if set.contains(&v.id) {
                if let GenreVue::Coupe { debut, fin, .. } = &mut v.genre {
                    *debut = point(*debut);
                    *fin = point(*fin);
                }
            }
        }
        for b in &mut self.barres {
            if set.contains(&b.id) {
                for p in [&mut b.debut, &mut b.fin] {
                    let xy = point([p[0], p[1]]);
                    p[0] = xy[0];
                    p[1] = xy[1];
                    p[2] += delta[2];
                }
                if b.genre == GenreBarre::Poteau {
                    b.rotation += angle;
                }
            }
        }
        for m in &mut self.murs {
            if set.contains(&m.id) {
                match &mut m.axe {
                    Axe::Droit { debut, fin } => {
                        *debut = point(*debut);
                        *fin = point(*fin);
                    }
                    Axe::Arc {
                        debut, fin, centre, ..
                    } => {
                        *debut = point(*debut);
                        *fin = point(*fin);
                        *centre = point(*centre);
                    }
                }
                m.decalage_bas += delta[2];
                if let Haut::Niveau { decalage, .. } = &mut m.haut {
                    *decalage += delta[2];
                }
            }
        }
        let hotes: HashMap<_, _> = self
            .ouvertures
            .iter()
            .filter(|o| set.contains(&o.id) && !set.contains(&o.mur))
            .map(|o| Ok((o.mur.clone(), solides::hote(self, &o.mur)?.0)))
            .collect::<Result<_, String>>()?;
        for b in &mut self.ouvertures {
            if set.contains(&b.id) {
                if !set.contains(&b.mur) {
                    if angle != 0.0 {
                        return Err("Une ouverture reste orientée par son hôte.".into());
                    }
                    let axe = &hotes[&b.mur];
                    let (p, u) = solides::repere(axe, b.abscisse);
                    let d = DVec2::new(delta[0], delta[1]);
                    if d.dot(u.perp()).abs() > 0.01 {
                        return Err("Déplacez l'ouverture le long du mur.".into());
                    }
                    let nouveau = axe.abscisse((p + d).to_array());
                    if nouveau - b.largeur / 2.0 < 0.0 || nouveau + b.largeur / 2.0 > axe.longueur()
                    {
                        return Err("Ouverture hors du mur.".into());
                    }
                    b.abscisse = nouveau;
                }
                b.allege += delta[2];
                if let Some(m) = &mut b.representation {
                    mesh(m);
                }
            }
        }
        for d in &mut self.dalles {
            if set.contains(&d.id) {
                for p in d.contours.iter_mut().flatten() {
                    *p = point(*p);
                }
                d.decalage += delta[2];
            }
        }
        for e in &mut self.importes {
            if set.contains(&e.id) {
                mesh(&mut e.maillage);
            }
        }
        Ok(())
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
fn croisement(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    fn projet() -> Projet {
        depuis_ifc::convertir(std::path::Path::new("samples/test_ifc4_murs.ifc"), &|_| {})
            .unwrap()
            .0
    }
    #[test]
    fn structures_creation_copie_rotation_suppression_et_validation() {
        let p = projet();
        let n = p.niveaux[0].id.clone();
        let poteau = Barre {
            id: String::new(),
            nom: "Poteau".into(),
            genre: GenreBarre::Poteau,
            niveau: n.clone(),
            debut: [100.0, 200.0, 0.0],
            fin: [100.0, 200.0, 3000.0],
            largeur: 300.0,
            hauteur: 400.0,
            rotation: 0.0,
        };
        let (p, ids) = p.executer(Commande::Barre { barre: poteau }).unwrap();
        let b = &p.barres[0];
        assert!((geometrie::volume(&solides::barre(&p, b).unwrap()) - 360_000_000.0).abs() < 0.1);
        let (p, _) = p
            .executer(Commande::Tourner {
                ids: ids.clone(),
                centre: [0.0, 0.0],
                angle: 90.0,
            })
            .unwrap();
        assert!((p.barres[0].rotation - 90.0).abs() < 1e-6);
        let (copie, copies) = p
            .executer(Commande::Copier {
                ids: ids.clone(),
                delta: [1000.0, 0.0, 0.0],
            })
            .unwrap();
        assert_eq!(copie.barres.len(), 2);
        let (retour, _) = copie.executer(Commande::Supprimer { ids: copies }).unwrap();
        assert_eq!(retour.json().unwrap(), p.json().unwrap());
        assert!(
            p.executer(Commande::Supprimer {
                ids: vec![n.clone()]
            })
            .is_err()
        );
        let poutre = Barre {
            id: String::new(),
            nom: "Poutre".into(),
            genre: GenreBarre::Poutre,
            niveau: n,
            debut: [0.0, 0.0, 2500.0],
            fin: [3000.0, 0.0, 2500.0],
            largeur: 200.0,
            hauteur: 400.0,
            rotation: 0.0,
        };
        let (p, _) = p
            .executer(Commande::Barre {
                barre: poutre.clone(),
            })
            .unwrap();
        assert!(
            (geometrie::volume(&solides::barre(&p, &p.barres[1]).unwrap()) - 240_000_000.0).abs()
                < 0.1
        );
        let mut invalide = poutre;
        invalide.fin[2] += 1.0;
        assert!(p.executer(Commande::Barre { barre: invalide }).is_err());
        let relu = Projet::lire_json(&p.json().unwrap()).unwrap();
        crate::scene::depuis_natif(&relu, std::path::Path::new("test.modele.json")).unwrap();
        let ifc = export_ifc::exporter(&relu).unwrap();
        assert!(ifc.contains("IFCCOLUMN(") && ifc.contains("IFCBEAM("));
    }
    #[test]
    fn modifier_deplacer_copier_et_supprimer_une_coupe() {
        let mut p = projet();
        let id = p
            .ajouter_vue_coupe([0.0, 0.0], [2000.0, 0.0], 6000.0, false, "Coupe", 100.0)
            .unwrap();
        let (moved, _) = p
            .executer(Commande::Deplacer {
                ids: vec![id.clone()],
                delta: [500.0, 1000.0, 0.0],
            })
            .unwrap();
        assert!(
            matches!(moved.vues.iter().find(|v| v.id == id).unwrap().genre, GenreVue::Coupe { debut, .. } if debut == [500.0,1000.0])
        );
        assert!(
            p.executer(Commande::Deplacer {
                ids: vec![id.clone()],
                delta: [0.0, 0.0, 10.0]
            })
            .is_err()
        );
        let (copie, ids) = moved
            .executer(Commande::Copier {
                ids: vec![id.clone()],
                delta: [0.0, 2000.0, 0.0],
            })
            .unwrap();
        assert!(
            matches!(copie.vues.iter().find(|v| v.id == ids[0]).unwrap().genre, GenreVue::Coupe { debut, .. } if debut == [500.0,3000.0])
        );
        let (retour, _) = copie.executer(Commande::Supprimer { ids }).unwrap();
        assert_eq!(retour.json().unwrap(), moved.json().unwrap());
    }
    #[test]
    fn annotations_et_quadrillages_se_copient_et_se_deplacent() {
        let p = Projet::lire_json(
            &std::fs::read_to_string("samples/maison_native.modele.json").unwrap(),
        )
        .unwrap();
        let texte = p
            .annotations
            .iter()
            .find(|a| a.id == "texte-sejour")
            .unwrap();
        let ancien = match &texte.forme {
            documentation::AnnotationForme::Texte { point, .. } => *point,
            _ => panic!("texte attendu"),
        };
        let (deplace, _) = p
            .executer(Commande::Deplacer {
                ids: vec![texte.id.clone()],
                delta: [100.0, 200.0, 0.0],
            })
            .unwrap();
        assert!(
            matches!(&deplace.annotations.iter().find(|a| a.id == texte.id).unwrap().forme, documentation::AnnotationForme::Texte { point, .. } if *point == [ancien[0] + 100.0, ancien[1] + 200.0])
        );
        let mut p = p;
        p.contextes.insert(
            "porte".into(),
            ContexteElement {
                phase: Some("Construction".into()),
                sous_projet: Some("Structure".into()),
                variante: None,
            },
        );
        let mut note = p
            .annotations
            .iter()
            .find(|a| a.id == "texte-sejour")
            .unwrap()
            .clone();
        note.id = "note-coupe".into();
        note.vue = "coupe".into();
        p.annotations.push(note);
        let (section, _) = p
            .executer(Commande::Deplacer {
                ids: vec!["note-coupe".into()],
                delta: [100.0, 0.0, 200.0],
            })
            .unwrap();
        assert!(
            matches!(&section.annotations.iter().find(|a| a.id == "note-coupe").unwrap().forme, documentation::AnnotationForme::Texte { point, .. } if *point == [ancien[0]+100.0, ancien[1]+200.0])
        );
        assert!(
            p.executer(Commande::Deplacer {
                ids: vec!["note-coupe".into()],
                delta: [0.0, 100.0, 0.0]
            })
            .is_err()
        );
        let (copie, ids) = p
            .executer(Commande::Copier {
                ids: vec!["coupe".into()],
                delta: [100.0, 0.0, 0.0],
            })
            .unwrap();
        assert!(copie.vues.iter().any(|v| v.id == ids[0]));
        assert!(
            copie
                .annotations
                .iter()
                .any(|a| a.vue == ids[0] && a.id != "note-coupe")
        );
        let (retour, _) = copie.executer(Commande::Supprimer { ids }).unwrap();
        assert_eq!(retour.json().unwrap(), p.json().unwrap());
        let (copie, ids) = p
            .executer(Commande::Copier {
                ids: vec!["rdc-m0".into(), "cote-largeur".into()],
                delta: [10000.0, 0.0, 0.0],
            })
            .unwrap();
        let porte = copie
            .ouvertures
            .iter()
            .find(|o| o.mur == ids[0] && o.genre == GenreOuverture::Porte)
            .unwrap();
        assert_eq!(
            copie
                .contextes
                .get(&porte.id)
                .unwrap()
                .sous_projet
                .as_deref(),
            Some("Structure")
        );
        let cote = copie.annotations.iter().find(|a| a.id == ids[1]).unwrap();
        assert!(
            matches!(&cote.forme, documentation::AnnotationForme::Cote { debut: documentation::PointCote::Mur { id, .. }, .. } if id == &ids[0])
        );
        let (retour, _) = copie.executer(Commande::Supprimer { ids }).unwrap();
        assert_eq!(retour.json().unwrap(), p.json().unwrap());
        let grid = &p.quadrillages[0];
        let (copie, ids) = p
            .executer(Commande::Copier {
                ids: vec![grid.id.clone()],
                delta: [100.0, 0.0, 0.0],
            })
            .unwrap();
        assert!(
            copie
                .quadrillages
                .iter()
                .any(|g| g.id == ids[0] && g.debut[0] == grid.debut[0] + 100.0)
        );
    }
    #[test]
    fn copier_assemblage_et_dependances_puis_supprimer_sans_perte() {
        let p = projet();
        let a = p.assemblages.first().unwrap();
        let avant = p.json().unwrap();
        let (copie, ids) = p
            .executer(Commande::Copier {
                ids: vec![a.id.clone()],
                delta: [7000.0, 0.0, 1000.0],
            })
            .unwrap();
        let id = &ids[0];
        let nouveau = copie.assemblage(id).unwrap();
        assert_eq!(nouveau.couches.len(), a.couches.len());
        assert!(nouveau.couches.iter().all(|id| !a.couches.contains(id)));
        assert_eq!(
            copie.ouvertures.iter().filter(|o| o.mur == *id).count(),
            p.ouvertures.iter().filter(|o| o.mur == a.id).count()
        );
        let (retour, _) = copie.executer(Commande::Supprimer { ids }).unwrap();
        assert_eq!(retour.json().unwrap(), avant);
    }
    #[test]
    fn erreurs_atomiques_et_niveaux_utilises() {
        let p = projet();
        let avant = p.json().unwrap();
        assert!(
            p.executer(Commande::Niveau {
                nom: "".into(),
                altitude: 0.0
            })
            .is_err()
        );
        assert!(
            p.executer(Commande::Supprimer {
                ids: vec![p.niveaux[0].id.clone()]
            })
            .is_err()
        );
        assert!(
            p.executer(Commande::Deplacer {
                ids: vec![p.assemblages[0].id.clone()],
                delta: [f64::NAN, 0.0, 0.0]
            })
            .is_err()
        );
        assert_eq!(p.json().unwrap(), avant);
    }
    #[test]
    fn deplacement_rotation_menuiseries_et_annulation() {
        let p = projet();
        let a = &p.assemblages[0];
        let (q, _) = p
            .executer(Commande::Deplacer {
                ids: vec![a.id.clone()],
                delta: [1000.0, 2000.0, 300.0],
            })
            .unwrap();
        let mut h = historique::Historique::default();
        h.enregistrer(&p).unwrap();
        h.pousser(&p, &q).unwrap();
        let (r, _) = q
            .executer(Commande::Tourner {
                ids: vec![a.id.clone()],
                centre: [1000.0, 2000.0],
                angle: 90.0,
            })
            .unwrap();
        h.pousser(&q, &r).unwrap();
        let retour = h.cible(false).unwrap();
        h.confirmer(&r, false).unwrap();
        assert_eq!(retour.json().unwrap(), q.json().unwrap());
        let retour = h.cible(false).unwrap();
        h.confirmer(&q, false).unwrap();
        assert_eq!(retour.json().unwrap(), p.json().unwrap());
        assert!(!h.modifie(&retour));
        let retabli = h.cible(true).unwrap();
        assert_eq!(retabli.json().unwrap(), q.json().unwrap());
    }
    #[test]
    fn tremie_interieure_et_exterieure() {
        let mut p = Projet::nouveau("Trémie");
        let params = edition::ParametresDalle {
            sorte: SorteDalle::Dalle,
            type_dalle: TypeCompose {
                id: "t".into(),
                nom: "Dalle".into(),
                couches: vec![Couche {
                    epaisseur: 200.0,
                    ..Default::default()
                }],
                couches_detaillees: true,
                origine: None,
            },
            pente: None,
            niveau: None,
            decalage: 0.0,
            phase: None,
            sous_projet: None,
            variante: None,
        };
        let id = p
            .ajouter_dalle(
                &[[0.0, 0.0], [6000.0, 0.0], [6000.0, 5000.0], [0.0, 5000.0]],
                &params,
            )
            .unwrap();
        let v = p.volume_dalle(&p.dalles[0]).unwrap();
        let contour = vec![
            [1000.0, 1000.0],
            [2000.0, 1000.0],
            [2000.0, 2000.0],
            [1000.0, 2000.0],
        ];
        let (q, _) = p
            .executer(Commande::Tremie {
                id: id.clone(),
                contour: contour.clone(),
            })
            .unwrap();
        assert!((q.volume_dalle(&q.dalles[0]).unwrap() / v - 29.0 / 30.0).abs() < 1e-8);
        assert!(q.executer(Commande::Tremie { id, contour }).is_err());
    }
}
