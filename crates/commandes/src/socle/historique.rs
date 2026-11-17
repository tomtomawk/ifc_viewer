//! Transactions atomiques par document. Les instantanés sont gardés compressés et
//! bornés en mémoire (au moins `NIVEAUX_GARANTIS` niveaux, au plus `NIVEAUX_MAX`).
//! La même pile sert au projet et à chaque famille ouverte dans l'éditeur.
use modele::famille::modelee::FichierFamille;
use modele::Projet;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Niveaux d'annulation gardés au plus.
const NIVEAUX_MAX: usize = 100;
/// Niveaux d'annulation gardés quel que soit le poids du projet.
const NIVEAUX_GARANTIS: usize = 10;
/// Mémoire des instantanés compressés d'un document, au-delà des niveaux garantis
/// (octets) ; réglable dans les options (`definir_limite_memoire`).
static LIMITE_MEMOIRE: AtomicUsize = AtomicUsize::new(64 * 1024 * 1024);

/// Règle la mémoire de l'historique de chaque document (mégaoctets, au moins 1).
pub fn definir_limite_memoire(mo: usize) {
    LIMITE_MEMOIRE.store(mo.max(1) * 1024 * 1024, Ordering::Relaxed);
}

/// Instantané compressé (deflate rapide : environ 6 fois plus petit qu'en JSON, 4 ms
/// pour un projet de 1,2 Mo).
fn compresser(json: &str) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec(json.as_bytes(), 1)
}

fn decompresser(octets: &[u8]) -> Result<String, String> {
    let brut = miniz_oxide::inflate::decompress_to_vec(octets).map_err(|e| format!("Historique illisible : {:?}", e.status))?;
    String::from_utf8(brut).map_err(|e| e.to_string())
}

/// Empreinte d'un instantané : comparée pour savoir si le document diffère du fichier,
/// sans garder une copie entière de l'état enregistré.
fn empreinte(json: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    json.hash(&mut h);
    h.finish()
}

/// Document dont l'historique garde des instantanés JSON.
pub trait Instantane: Sized {
    fn instantane(&self) -> Result<String, String>;
    /// Relit un instantané, sans contrôle : un état intermédiaire peut être invalide.
    fn restaurer(json: &str) -> Result<Self, String>;
    fn libelle(avant: &str, apres: &str) -> String;
}

impl Instantane for Projet {
    fn instantane(&self) -> Result<String, String> {
        self.json()
    }
    fn restaurer(json: &str) -> Result<Self, String> {
        Projet::lire_json(json)
    }
    fn libelle(avant: &str, apres: &str) -> String {
        libelle(avant, apres)
    }
}

impl Instantane for FichierFamille {
    fn instantane(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| e.to_string())
    }
    fn restaurer(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }
    fn libelle(avant: &str, apres: &str) -> String {
        let famille = |t: &str| serde_json::from_str::<serde_json::Value>(t).ok().and_then(|v| v.get("famille").cloned());
        let (Some(a), Some(b)) = (famille(avant), famille(apres)) else {
            return "Modifier la famille".into();
        };
        let categories = [
            ("parametres", "paramètre"),
            ("plans", "plan de référence"),
            ("contraintes", "contrainte"),
            ("volumes", "volume"),
            ("extrusions", "extrusion"),
            ("symboles", "symbole 2D"),
        ];
        libelle_collections(&a, &b, &categories, "nom", "Modifier la famille")
    }
}

struct Entree {
    /// Instantané compressé (`compresser`).
    json: Vec<u8>,
    libelle: String,
}

pub struct Historique<T: Instantane = Projet> {
    avant: VecDeque<Entree>,
    apres: Vec<Entree>,
    /// Empreinte de l'état enregistré.
    sauvegarde: Option<u64>,
    geste: bool,
    debut_geste: Option<Entree>,
    document: PhantomData<fn() -> T>,
}

impl<T: Instantane> Default for Historique<T> {
    fn default() -> Self {
        Self { avant: VecDeque::new(), apres: Vec::new(), sauvegarde: None, geste: false, debut_geste: None, document: PhantomData }
    }
}

/// Décrit les collections modifiées, sans prétendre connaître une commande lorsque
/// plusieurs collections changent en même temps. Les collections sont comparées sur
/// leur texte brut ; seule la collection modifiée est lue en détail (un projet de 1,2 Mo
/// coûtait deux lectures complètes, 18 ms, à chaque commande).
fn libelle(avant: &str, apres: &str) -> String {
    const DEFAUT: &str = "Modifier le projet";
    type Brut<'a> = HashMap<&'a str, &'a serde_json::value::RawValue>;
    let (Ok(a), Ok(b)) = (serde_json::from_str::<Brut>(avant), serde_json::from_str::<Brut>(apres)) else {
        return DEFAUT.into();
    };
    // collection absente ou vide : identiques
    fn texte<'a>(m: &Brut<'a>, c: &str) -> Option<&'a str> {
        m.get(c).map(|r| r.get()).filter(|t| *t != "[]")
    }
    let changees: Vec<&(&str, &str)> = CATEGORIES_PROJET.iter().filter(|(c, _)| texte(&a, c) != texte(&b, c)).collect();
    let [(champ, nom)] = changees.as_slice().iter().map(|c| **c).collect::<Vec<_>>()[..] else { return DEFAUT.into() };
    libelle_brut(nom, texte(&a, champ), texte(&b, champ)).unwrap_or_else(|| DEFAUT.into())
}

/// Identifiant et nom d'un objet, le reste du texte étant seulement parcouru.
#[derive(serde::Deserialize)]
struct CleObjet<'a> {
    #[serde(borrow, default)]
    id: Option<&'a serde_json::value::RawValue>,
    #[serde(default)]
    nom: Option<String>,
}

/// `libelle_collections` d'une seule collection, sur le texte brut de ses objets : seuls
/// leur identifiant et leur nom sont lus, les objets sont comparés texte à texte.
fn libelle_brut(nom: &str, ta: Option<&str>, tb: Option<&str>) -> Option<String> {
    let lire = |t: Option<&str>| -> Option<Vec<String>> {
        let objets: Vec<&serde_json::value::RawValue> = t.map_or(Some(Vec::new()), |t| serde_json::from_str(t).ok())?;
        Some(objets.into_iter().map(|r| r.get().to_string()).collect())
    };
    let (va, vb) = (lire(ta)?, lire(tb)?);
    let (na, nb) = (va.len(), vb.len());
    let action = if nb > na { "Ajouter" } else if nb < na { "Supprimer" } else { "Modifier" };
    let (source, autre) = if nb < na { (&va, &vb) } else { (&vb, &va) };
    let cle = |t: &str| serde_json::from_str::<CleObjet>(t).ok().and_then(|o| Some((o.id?.get().to_string(), o.nom)));
    let index: HashMap<String, &str> = autre.iter().filter_map(|t| Some((cle(t)?.0, t.as_str()))).collect();
    let noms: Vec<String> = source.iter().filter_map(|t| {
        let (id, nom_objet) = cle(t)?;
        let precedent = index.get(&id);
        let garde = if na == nb { precedent.is_some_and(|p| *p != t) } else { precedent.is_none() };
        garde.then_some(nom_objet?).filter(|n| !n.trim().is_empty())
    }).collect();
    Some(match noms.as_slice() {
        [objet] => format!("{action} {nom} « {objet} »"),
        _ => format!("{action} {nom}"),
    })
}

const CATEGORIES_PROJET: [(&str, &str); 20] = [
        ("niveaux", "niveau"),
        ("murs", "mur"),
        ("assemblages", "assemblage"),
        ("dalles", "élément horizontal"),
        ("barres", "structure"),
        ("pieces", "pièce"),
        ("composants", "composant"),
        ("arbres", "arbre"),
        ("escaliers", "escalier"),
        ("garde_corps", "garde-corps"),
        ("ouvertures", "ouverture"),
        ("importes", "élément importé"),
        ("vues", "vue"),
        ("quadrillages", "quadrillage"),
        ("annotations", "annotation"),
        ("feuilles", "feuille"),
        ("nomenclatures", "nomenclature"),
        ("types_murs", "type de mur"),
        ("types_dalles", "type de dalle"),
        ("materiaux", "matériau"),
];

/// Libellé d'une étape : la seule collection modifiée et, si un seul objet change,
/// son nom ; les objets sont appariés par `cle`.
fn libelle_collections(a: &serde_json::Value, b: &serde_json::Value, categories: &[(&str, &str)], cle: &str, defaut: &str) -> String {
    let mut changements = Vec::new();
    for &(champ, nom) in categories {
        let va = a.get(champ).and_then(|v| v.as_array());
        let vb = b.get(champ).and_then(|v| v.as_array());
        let na = va.map_or(0, Vec::len);
        let nb = vb.map_or(0, Vec::len);
        if na != nb || (na > 0 && va != vb) {
            changements.push((nom, na, nb, va, vb));
        }
    }
    match changements.as_slice() {
        [(nom, na, nb, va, vb)] => {
            let action = if nb > na {
                "Ajouter"
            } else if nb < na {
                "Supprimer"
            } else {
                "Modifier"
            };
            let (source, autre) = if nb < na { (va, vb) } else { (vb, va) };
            // objets de l'autre état par identifiant (une recherche linéaire par objet
            // coûtait 78 ms pour 1 920 murs)
            let index: HashMap<String, &serde_json::Value> = autre.into_iter().flat_map(|v| v.iter()).filter_map(|o| Some((o.get(cle)?.to_string(), o))).collect();
            let noms: Vec<&str> = source
                .into_iter()
                .flat_map(|v| v.iter())
                .filter(|objet| {
                    let Some(id) = objet.get(cle) else {
                        return false;
                    };
                    let precedent = index.get(&id.to_string()).copied();
                    if na == nb {
                        precedent.is_some_and(|p| p != *objet)
                    } else {
                        precedent.is_none()
                    }
                })
                .filter_map(|objet| {
                    objet
                        .get("nom")
                        .and_then(|n| n.as_str())
                        .filter(|n| !n.trim().is_empty())
                })
                .collect();
            if let [objet] = noms.as_slice() {
                format!("{action} {nom} « {objet} »")
            } else {
                format!("{action} {nom}")
            }
        }
        _ => defaut.into(),
    }
}

impl<T: Instantane> Historique<T> {
    pub fn enregistrer(&mut self, p: &T) -> Result<(), String> {
        self.geste(false);
        self.sauvegarde = Some(empreinte(&p.instantane()?));
        Ok(())
    }
    pub fn modifie(&self, p: &T) -> bool {
        p.instantane().ok().map(|j| empreinte(&j)) != self.sauvegarde
    }
    #[cfg(test)]
    pub fn peut_annuler(&self) -> bool {
        !self.avant.is_empty()
    }
    #[cfg(test)]
    pub fn peut_retablir(&self) -> bool {
        !self.apres.is_empty()
    }
    pub fn libelles(&self, retablir: bool) -> Vec<String> {
        if retablir {
            self.apres.iter().rev().map(|e| e.libelle.clone()).collect()
        } else {
            self.avant.iter().rev().map(|e| e.libelle.clone()).collect()
        }
    }
    pub fn geste(&mut self, actif: bool) {
        if !actif && let Some(s) = self.debut_geste.take() {
            self.avant.push_back(s);
            self.borner();
        }
        self.geste = actif;
    }
    fn borner(&mut self) {
        let limite = LIMITE_MEMOIRE.load(Ordering::Relaxed);
        while self.avant.len() > NIVEAUX_MAX
            || (self.avant.len() > NIVEAUX_GARANTIS
                && self.avant.iter().chain(&self.apres).map(|e| e.json.len()).sum::<usize>() > limite)
        {
            self.avant.pop_front();
        }
    }
    /// Mémoire occupée par les instantanés (octets compressés).
    pub fn memoire(&self) -> usize {
        self.avant.iter().chain(&self.apres).map(|e| e.json.len()).sum()
    }
    pub fn pousser(&mut self, avant: &T, apres: &T) -> Result<(), String> {
        self.pousser_modifie(avant, apres).map(|_| ())
    }
    /// `pousser`, puis `modifie(apres)` sans refaire l'instantané.
    pub fn pousser_modifie(&mut self, avant: &T, apres: &T) -> Result<bool, String> {
        let a = avant.instantane()?;
        let b = apres.instantane()?;
        let modifie = Some(empreinte(&b)) != self.sauvegarde;
        if a == b {
            return Ok(modifie);
        }
        if !self.geste || self.debut_geste.is_none() {
            let entree = Entree {
                libelle: T::libelle(&a, &b),
                json: compresser(&a),
            };
            if self.geste {
                self.debut_geste = Some(entree);
            } else {
                self.avant.push_back(entree);
            }
        }
        self.apres.clear();
        self.borner();
        Ok(modifie)
    }
    /// Le contrôleur valide et reconstruit la cible avant de déplacer les piles.
    pub fn cible_n(&self, retablir: bool, nombre: usize) -> Result<T, String> {
        if nombre == 0 {
            return Err("Nombre de modifications invalide.".into());
        }
        let s = if retablir {
            self.apres.iter().rev().nth(nombre - 1)
        } else {
            self.avant.iter().rev().nth(nombre - 1)
        }
        .ok_or("Historique insuffisant.")?;
        T::restaurer(&decompresser(&s.json)?)
    }
    pub fn confirmer_n(
        &mut self,
        actuel: &T,
        retablir: bool,
        nombre: usize,
    ) -> Result<(), String> {
        if nombre == 0
            || nombre
                > if retablir {
                    self.apres.len()
                } else {
                    self.avant.len()
                }
        {
            return Err("Historique insuffisant.".into());
        }
        let mut courant = compresser(&actuel.instantane()?);
        for _ in 0..nombre {
            let entree = if retablir {
                self.apres.pop().unwrap()
            } else {
                self.avant.pop_back().unwrap()
            };
            let inverse = Entree {
                json: courant,
                libelle: entree.libelle,
            };
            courant = entree.json;
            if retablir {
                self.avant.push_back(inverse);
            } else {
                self.apres.push(inverse);
            }
        }
        self.borner();
        Ok(())
    }
    pub fn cible(&self, retablir: bool) -> Result<T, String> {
        self.cible_n(retablir, 1)
    }
    pub fn confirmer(&mut self, actuel: &T, retablir: bool) -> Result<(), String> {
        self.confirmer_n(actuel, retablir, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ancien calcul du libellé (lecture complète des deux états), pour comparaison.
    fn libelle_complet(avant: &str, apres: &str) -> String {
        let (a, b): (serde_json::Value, serde_json::Value) = (serde_json::from_str(avant).unwrap(), serde_json::from_str(apres).unwrap());
        libelle_collections(&a, &b, &CATEGORIES_PROJET, "id", "Modifier le projet")
    }

    /// Libellé allégé identique à l'ancien sur des commandes réelles (ajout, modification,
    /// suppression, plusieurs collections, rien) de l'exemple de la maison.
    #[test]
    fn libelle_allege_identique() {
        use crate::{Commande, Execution};
        let p = Projet::lire_json(include_str!("../../../../samples/maison_native.modele.json")).unwrap();
        let mur = p.murs[0].id.clone();
        let niveau = p.niveaux[0].id.clone();
        let etats = [
            p.executer(Commande::Supprimer { ids: vec![p.ouvertures[0].id.clone()] }).unwrap().0,
            p.executer(Commande::Deplacer { ids: vec![mur], delta: [100.0, 0.0, 0.0] }).unwrap().0,
            p.executer(Commande::ModifierNiveau { id: niveau.clone(), nom: "Rez".into(), altitude: 0.0 }).unwrap().0,
            p.clone(),
        ];
        let a = p.json().unwrap();
        for e in &etats {
            let b = e.json().unwrap();
            assert_eq!(libelle(&a, &b), libelle_complet(&a, &b));
            assert_eq!(libelle(&b, &a), libelle_complet(&b, &a));
        }
        assert!(libelle(&a, &etats[2].json().unwrap()).contains("Rez"));
    }

    /// Annuler et rétablir rendent exactement les états, malgré la compression ; la
    /// mémoire occupée est bien inférieure au JSON.
    #[test]
    fn compression_sans_perte() {
        use crate::{Commande, Execution};
        let mut p = Projet::lire_json(include_str!("../../../../samples/maison_native.modele.json")).unwrap();
        let mut h: Historique = Historique::default();
        let mut etats = vec![p.json().unwrap()];
        for i in 0..5 {
            let mur = p.murs[i % p.murs.len()].id.clone();
            let q = p.executer(Commande::Deplacer { ids: vec![mur], delta: [10.0, 0.0, 0.0] }).unwrap().0;
            h.pousser(&p, &q).unwrap();
            p = q;
            etats.push(p.json().unwrap());
        }
        assert!(h.memoire() * 3 < etats.iter().take(5).map(String::len).sum::<usize>());
        for k in (0..5).rev() {
            let c = h.cible(false).unwrap();
            h.confirmer(&p, false).unwrap();
            p = c;
            assert_eq!(p.json().unwrap(), etats[k]);
        }
        let c = h.cible(true).unwrap();
        h.confirmer(&p, true).unwrap();
        assert_eq!(c.json().unwrap(), etats[1]);
    }

    /// Limite mémoire minuscule : 10 niveaux restent ; sans limite atteinte : 100 au plus.
    #[test]
    fn niveaux_garantis_et_plafond() {
        let mut h: Historique<FichierFamille> = Historique::default();
        use modele::famille::modelee::{FamilleModelee, FORMAT_FAMILLE};
        let mut f = FichierFamille { format: FORMAT_FAMILLE.into(), nom: "OF".into(), largeur: 1000.0, hauteur: 1000.0, profondeur: 300.0, famille: FamilleModelee::gabarit_fenetre() };
        let limite = LIMITE_MEMOIRE.load(Ordering::Relaxed);
        LIMITE_MEMOIRE.store(1, Ordering::Relaxed);
        for i in 0..30 {
            let mut g = f.clone();
            g.largeur = 1000.0 + i as f64;
            h.pousser(&f, &g).unwrap();
            f = g;
        }
        assert_eq!(h.libelles(false).len(), NIVEAUX_GARANTIS);
        LIMITE_MEMOIRE.store(limite, Ordering::Relaxed);
        for i in 0..150 {
            let mut g = f.clone();
            g.hauteur = 1000.0 + i as f64;
            h.pousser(&f, &g).unwrap();
            f = g;
        }
        assert_eq!(h.libelles(false).len(), NIVEAUX_MAX);
    }

    #[test]
    fn famille_a_sa_propre_pile_et_ses_libelles() {
        use modele::famille::modelee::{FamilleModelee, PlanRef, AxePlan, FORMAT_FAMILLE};
        let a = FichierFamille { format: FORMAT_FAMILLE.into(), nom: "OF".into(), largeur: 1200.0, hauteur: 1350.0, profondeur: 300.0, famille: FamilleModelee::gabarit_fenetre() };
        let mut b = a.clone();
        b.famille.plans.push(PlanRef { nom: "Plan 9".into(), axe: AxePlan::X, position: 100.0, fixe: false, hote: None });
        let mut h: Historique<FichierFamille> = Historique::default();
        h.pousser(&a, &b).unwrap();
        assert_eq!(h.libelles(false), vec!["Ajouter plan de référence « Plan 9 »".to_string()]);
        let retour = h.cible(false).unwrap();
        assert_eq!(retour, a);
        h.confirmer(&b, false).unwrap();
        assert_eq!(h.cible(true).unwrap(), b);
        let mut plus_profond = b.clone();
        plus_profond.profondeur = 750.0;
        let mut profondeur: Historique<FichierFamille> = Historique::default();
        profondeur.pousser(&b, &plus_profond).unwrap();
        assert_eq!(profondeur.cible(false).unwrap().profondeur, 300.0);
        profondeur.confirmer(&plus_profond, false).unwrap();
        assert_eq!(profondeur.cible(true).unwrap().profondeur, 750.0);
    }
    #[test]
    fn sauvegarde_annulation_branche_et_documents_isoles() {
        let a = Projet::nouveau("a");
        let b = Projet::nouveau("b");
        let c = Projet::nouveau("c");
        let mut h = Historique::default();
        let mut autre = Historique::default();
        h.enregistrer(&a).unwrap();
        autre.enregistrer(&c).unwrap();
        h.pousser(&a, &b).unwrap();
        assert!(h.modifie(&b));
        let retour = h.cible(false).unwrap();
        h.confirmer(&b, false).unwrap();
        assert!(!h.modifie(&retour));
        assert!(h.peut_retablir());
        assert!(!autre.peut_annuler());
        h.pousser(&retour, &c).unwrap();
        assert!(!h.peut_retablir());
    }
    #[test]
    fn plusieurs_etapes_conservent_ordre_et_annulent_atomiquement() {
        let a = Projet::nouveau("a");
        let b = Projet::nouveau("b");
        let c = Projet::nouveau("c");
        let d = Projet::nouveau("d");
        let mut h = Historique::default();
        h.enregistrer(&a).unwrap();
        h.pousser(&a, &b).unwrap();
        h.pousser(&b, &c).unwrap();
        h.pousser(&c, &d).unwrap();
        assert!(h.cible_n(false, 4).is_err());
        assert!(h.confirmer_n(&d, false, 4).is_err());
        assert_eq!(h.libelles(false).len(), 3);
        let cible = h.cible_n(false, 2).unwrap();
        assert_eq!(cible.nom, "b");
        h.confirmer_n(&d, false, 2).unwrap();
        assert_eq!(h.cible_n(true, 1).unwrap().nom, "c");
        assert_eq!(h.cible_n(true, 2).unwrap().nom, "d");
        h.confirmer_n(&cible, true, 2).unwrap();
        assert_eq!(h.cible_n(false, 2).unwrap().nom, "b");
    }
    #[test]
    fn les_libelles_identifient_les_elements_et_suivent_le_retablissement() {
        let a = Projet::nouveau("test");
        let mut b = a.clone();
        b.niveaux.push(modele::Niveau {
            id: "n1".into(),
            nom: "Étage".into(),
            altitude: 3000.0,
            origine: None,
        });
        let mut h = Historique::default();
        h.pousser(&a, &b).unwrap();
        assert_eq!(h.libelles(false), ["Ajouter niveau « Étage »"]);
        h.confirmer_n(&b, false, 1).unwrap();
        assert_eq!(h.libelles(true), ["Ajouter niveau « Étage »"]);
    }
}
