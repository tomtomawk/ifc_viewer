//! Transactions atomiques par document, gardées par différences : chaque étape ne
//! retient que l'état avant et après des champs et des objets touchés
//! (`differences`), compressé. Mémoire proportionnelle au changement, bornée (au moins
//! `NIVEAUX_GARANTIS` niveaux, au plus `NIVEAUX_MAX`). Un geste (glissement à la
//! souris) donne une seule étape : ses poussées successives sont fusionnées. La même
//! pile sert au projet et à chaque famille ouverte dans l'éditeur.
use super::differences::{appliquer, calculer_texte, champs, identifiants, Difference};
use super::libelles::{libelle_collections, libelle_projet};
use modele::famille::modelee::FichierFamille;
use modele::Projet;
use serde_json::Value;
use std::collections::{BTreeSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Niveaux d'annulation gardés au plus.
const NIVEAUX_MAX: usize = 100;
/// Niveaux d'annulation gardés quel que soit le poids des étapes.
const NIVEAUX_GARANTIS: usize = 10;
/// Mémoire des étapes d'un document, au-delà des niveaux garantis (octets) ; réglable
/// dans les options (`definir_limite_memoire`).
static LIMITE_MEMOIRE: AtomicUsize = AtomicUsize::new(64 * 1024 * 1024);

/// Règle la mémoire de l'historique de chaque document (mégaoctets, au moins 1).
pub fn definir_limite_memoire(mo: usize) {
    LIMITE_MEMOIRE.store(mo.max(1) * 1024 * 1024, Ordering::Relaxed);
}

fn compresser(d: &Difference) -> Result<Vec<u8>, String> {
    let json = serde_json::to_vec(d).map_err(|e| e.to_string())?;
    Ok(miniz_oxide::deflate::compress_to_vec(&json, 1))
}

fn decompresser(octets: &[u8]) -> Result<Difference, String> {
    let brut = miniz_oxide::inflate::decompress_to_vec(octets).map_err(|e| format!("Historique illisible : {:?}", e.status))?;
    serde_json::from_slice(&brut).map_err(|e| e.to_string())
}

/// Empreinte d'un état : comparée pour savoir si le document diffère du fichier, sans
/// garder une copie de l'état enregistré.
fn empreinte(texte: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    texte.hash(&mut h);
    h.finish()
}

/// Document dont l'historique compare des états JSON.
pub trait Instantane: Sized + serde::Serialize + serde::de::DeserializeOwned {
    /// État en JSON compact, comparé texte à texte (`calculer_texte`).
    fn texte(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| e.to_string())
    }
    fn valeur(&self) -> Result<Value, String> {
        serde_json::to_value(self).map_err(|e| e.to_string())
    }
    /// Relit un état, sans contrôle : un état intermédiaire peut être invalide.
    fn restaurer(v: Value) -> Result<Self, String> {
        serde_json::from_value(v).map_err(|e| e.to_string())
    }
    fn libelle(avant: &str, apres: &str, d: &Difference) -> String;
}

impl Instantane for Projet {
    fn libelle(avant: &str, apres: &str, d: &Difference) -> String {
        libelle_projet(avant, apres, d)
    }
}

impl Instantane for FichierFamille {
    fn libelle(avant: &str, apres: &str, _: &Difference) -> String {
        let famille = |t: &str| serde_json::from_str::<Value>(t).ok().and_then(|v| v.get("famille").cloned());
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

/// Ce que touche une ou plusieurs étapes : identifiants des objets ajoutés, retirés ou
/// modifiés (murs, éléments, niveaux, annotations, vues…) et champs de premier niveau
/// du document (`murs`, `nom`, `soleil`…). Entrée de l'invalidation par élément.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Modifications {
    pub identifiants: BTreeSet<String>,
    pub champs: BTreeSet<String>,
}

impl Modifications {
    fn de(d: &Difference) -> Self {
        let mut identifiants_ = BTreeSet::new();
        identifiants(d, &mut identifiants_);
        Self { identifiants: identifiants_, champs: champs(d).into_iter().collect() }
    }
    fn octets(&self) -> usize {
        self.identifiants.iter().chain(&self.champs).map(String::len).sum()
    }
    fn reunir(&mut self, autre: &Self) {
        self.identifiants.extend(autre.identifiants.iter().cloned());
        self.champs.extend(autre.champs.iter().cloned());
    }
}

struct Entree {
    /// Différence compressée (`compresser`), appliquée vers l'avant pour rétablir, vers
    /// l'arrière pour annuler.
    difference: Vec<u8>,
    libelle: String,
    modifications: Modifications,
}

impl Entree {
    fn nouvelle(d: &Difference, libelle: String) -> Result<Self, String> {
        Ok(Self { difference: compresser(d)?, libelle, modifications: Modifications::de(d) })
    }
    fn memoire(&self) -> usize {
        self.difference.len() + self.libelle.len() + self.modifications.octets()
    }
}

/// Étape d'un geste en cours : état d'avant le geste (texte, gardé le temps du geste)
/// et différence jusqu'à l'état courant, recalculée à chaque poussée.
struct Geste {
    origine: String,
    difference: Difference,
    libelle: String,
    modifications: Modifications,
}

pub struct Historique<T: Instantane = Projet> {
    avant: VecDeque<Entree>,
    apres: Vec<Entree>,
    /// Empreinte de l'état enregistré.
    sauvegarde: Option<u64>,
    geste: bool,
    debut_geste: Option<Geste>,
    document: PhantomData<fn() -> T>,
}

impl<T: Instantane> Default for Historique<T> {
    fn default() -> Self {
        Self { avant: VecDeque::new(), apres: Vec::new(), sauvegarde: None, geste: false, debut_geste: None, document: PhantomData }
    }
}

impl<T: Instantane> Historique<T> {
    pub fn enregistrer(&mut self, p: &T) -> Result<(), String> {
        self.geste(false);
        self.sauvegarde = Some(empreinte(&p.texte()?));
        Ok(())
    }
    pub fn modifie(&self, p: &T) -> bool {
        p.texte().ok().map(|t| empreinte(&t)) != self.sauvegarde
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
    /// Début (`true`) ou fin d'un geste : toutes les poussées d'un geste forment une
    /// seule étape.
    pub fn geste(&mut self, actif: bool) {
        if !actif && let Some(g) = self.debut_geste.take() {
            // une différence calculée se sérialise toujours
            if let Ok(mut e) = Entree::nouvelle(&g.difference, g.libelle) {
                e.modifications = g.modifications;
                self.avant.push_back(e);
            }
            self.borner();
        }
        self.geste = actif;
    }
    fn borner(&mut self) {
        let limite = LIMITE_MEMOIRE.load(Ordering::Relaxed);
        while self.avant.len() > NIVEAUX_MAX || (self.avant.len() > NIVEAUX_GARANTIS && self.memoire() > limite) {
            self.avant.pop_front();
        }
    }
    /// Mémoire occupée par les étapes (différences compressées, libellés, identifiants).
    pub fn memoire(&self) -> usize {
        self.avant.iter().chain(&self.apres).map(Entree::memoire).sum()
    }
    pub fn pousser(&mut self, avant: &T, apres: &T) -> Result<(), String> {
        self.pousser_modifie(avant, apres).map(|_| ())
    }
    /// `pousser`, puis `modifie(apres)` sans relire l'état.
    pub fn pousser_modifie(&mut self, avant: &T, apres: &T) -> Result<bool, String> {
        let b = apres.texte()?;
        let modifie = Some(empreinte(&b)) != self.sauvegarde;
        if self.geste && let Some(g) = self.debut_geste.take() {
            // fusion : différence de l'état d'avant le geste à l'état courant
            if let Some(d) = calculer_texte(&g.origine, &b)? {
                let libelle = T::libelle(&g.origine, &b, &d);
                let mut modifications = Modifications::de(&d);
                // un objet touché puis rendu à son état reste à invalider
                modifications.reunir(&g.modifications);
                self.debut_geste = Some(Geste { origine: g.origine, difference: d, libelle, modifications });
            }
            self.apres.clear();
            return Ok(modifie);
        }
        let a = avant.texte()?;
        let Some(d) = calculer_texte(&a, &b)? else { return Ok(modifie) };
        let libelle = T::libelle(&a, &b, &d);
        if self.geste {
            let modifications = Modifications::de(&d);
            self.debut_geste = Some(Geste { origine: a, difference: d, libelle, modifications });
        } else {
            self.avant.push_back(Entree::nouvelle(&d, libelle)?);
        }
        self.apres.clear();
        self.borner();
        Ok(modifie)
    }
    fn etapes(&self, retablir: bool, nombre: usize) -> Result<Vec<&Entree>, String> {
        if nombre == 0 {
            return Err("Nombre de modifications invalide.".into());
        }
        let pile: Vec<&Entree> = if retablir { self.apres.iter().rev().take(nombre).collect() } else { self.avant.iter().rev().take(nombre).collect() };
        if pile.len() < nombre {
            return Err("Historique insuffisant.".into());
        }
        Ok(pile)
    }
    /// État obtenu en annulant (ou rétablissant) `nombre` étapes depuis `actuel`. Le
    /// contrôleur valide et reconstruit la cible avant de déplacer les piles
    /// (`confirmer_n`).
    pub fn cible_n(&self, actuel: &T, retablir: bool, nombre: usize) -> Result<T, String> {
        let mut v = actuel.valeur()?;
        for e in self.etapes(retablir, nombre)? {
            appliquer(&mut v, &decompresser(&e.difference)?, retablir)?;
        }
        T::restaurer(v)
    }
    /// Objets et champs que touchent les `nombre` prochaines étapes à annuler (ou à
    /// rétablir).
    pub fn modifications(&self, retablir: bool, nombre: usize) -> Result<Modifications, String> {
        let mut m = Modifications::default();
        for e in self.etapes(retablir, nombre)? {
            m.reunir(&e.modifications);
        }
        Ok(m)
    }
    /// Objets et champs touchés par la dernière poussée (geste en cours compris).
    pub fn derniere_modification(&self) -> Option<&Modifications> {
        self.debut_geste.as_ref().map(|g| &g.modifications).or_else(|| self.avant.back().map(|e| &e.modifications))
    }
    /// Déplace `nombre` étapes d'une pile à l'autre, une fois la cible appliquée.
    pub fn confirmer_n(&mut self, retablir: bool, nombre: usize) -> Result<(), String> {
        if nombre == 0 || nombre > if retablir { self.apres.len() } else { self.avant.len() } {
            return Err("Historique insuffisant.".into());
        }
        // Les étapes passent une à une du sommet d'une pile au sommet de l'autre.
        if retablir {
            let debut = self.apres.len() - nombre;
            self.avant.extend(self.apres.drain(debut..).rev());
        } else {
            let debut = self.avant.len() - nombre;
            self.apres.extend(self.avant.drain(debut..).rev());
        }
        self.borner();
        Ok(())
    }
    pub fn cible(&self, actuel: &T, retablir: bool) -> Result<T, String> {
        self.cible_n(actuel, retablir, 1)
    }
    pub fn confirmer(&mut self, retablir: bool) -> Result<(), String> {
        self.confirmer_n(retablir, 1)
    }
}

#[cfg(test)]
#[path = "historique_tests.rs"]
mod tests;
