//! Famille modélisée, à la manière de l'éditeur de familles de Revit : des plans de
//! référence pilotés par des contraintes (cotes libellées par des paramètres, égalités)
//! résolues par priorité, et des volumes (pleins ou vides) dont chaque face est
//! verrouillée sur un plan, avec un décalage. Une fois chargée dans la maquette, la
//! famille est placée dans la baie de son mur hôte : les volumes pleins forment la
//! menuiserie, les vides percent le mur.
//!
//! Repère de la famille : X le long du mur (milieu de la baie en 0, vers la droite vu de
//! l'intérieur), Y à travers le mur (axe du mur en 0, extérieur positif), Z depuis le bas
//! de la baie (allège). Les plans hôtes « Extérieur » et « Intérieur » suivent les faces
//! du mur ; l'épaisseur vient du mur hôte (paramètre `Epaisseur`).
use super::famille::Role;
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Axe d'un plan de référence.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AxePlan {
    /// Largeur (plans verticaux perpendiculaires au mur).
    X,
    /// Épaisseur du mur (plans parallèles au mur).
    Y,
    /// Hauteur (plans horizontaux).
    Z,
}

impl AxePlan {
    pub const TOUS: [AxePlan; 3] = [Self::X, Self::Y, Self::Z];
    pub fn nom(self) -> &'static str {
        match self {
            Self::X => "largeur",
            Self::Y => "épaisseur",
            Self::Z => "hauteur",
        }
    }
}

/// Plan fixé par l'hôte.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Hote {
    /// Face extérieure du mur : +Epaisseur/2.
    Exterieur,
    /// Face intérieure du mur : -Epaisseur/2.
    Interieur,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PlanRef {
    pub nom: String,
    pub axe: AxePlan,
    /// Position dessinée (mm) : celle d'un plan que rien ne contraint.
    pub position: f64,
    /// Épinglé à sa position (plans d'origine).
    #[serde(default)]
    pub fixe: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hote: Option<Hote>,
}

/// Paramètre de famille : formule (nombre, autres paramètres, + - * / et parenthèses).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Parametre {
    pub nom: String,
    pub formule: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "genre", rename_all = "snake_case")]
pub enum Contrainte {
    /// Cote : position(b) - position(a) = valeur (expression).
    Distance { a: String, b: String, valeur: String, priorite: u8 },
    /// Égalité (EQ) : `m` au milieu de `a` et `b`.
    Egalite { a: String, m: String, b: String, priorite: u8 },
}

impl Contrainte {
    pub fn priorite(&self) -> u8 {
        match self {
            Self::Distance { priorite, .. } | Self::Egalite { priorite, .. } => *priorite,
        }
    }
    pub fn priorite_mut(&mut self) -> &mut u8 {
        match self {
            Self::Distance { priorite, .. } | Self::Egalite { priorite, .. } => priorite,
        }
    }
    pub fn plans(&self) -> Vec<&str> {
        match self {
            Self::Distance { a, b, .. } => vec![a, b],
            Self::Egalite { a, m, b, .. } => vec![a, m, b],
        }
    }
}

/// Face d'un volume verrouillée sur un plan de référence, avec un décalage (mm).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Face {
    pub plan: String,
    #[serde(default)]
    pub decalage: f64,
}

/// Rôle d'un volume plein (couleur au rendu).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Materiau {
    Menuiserie,
    Vitrage,
    Panneau,
    Appui,
}

impl Materiau {
    pub const TOUS: [Materiau; 4] = [Self::Menuiserie, Self::Vitrage, Self::Panneau, Self::Appui];
    pub fn nom(self) -> &'static str {
        match self {
            Self::Menuiserie => "Menuiserie",
            Self::Vitrage => "Vitrage",
            Self::Panneau => "Panneau",
            Self::Appui => "Appui",
        }
    }
    pub fn role(self) -> Role {
        match self {
            Self::Menuiserie => Role::Dormant,
            Self::Vitrage => Role::Vitrage,
            Self::Panneau => Role::Panneau,
            Self::Appui => Role::Appui,
        }
    }
}

/// Extrusion rectangulaire : deux faces par axe (X, Y, Z).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Volume {
    pub nom: String,
    /// Vide : perce le mur hôte, n'est pas dessiné dans la maquette.
    #[serde(default)]
    pub vide: bool,
    #[serde(default = "menuiserie")]
    pub materiau: Materiau,
    pub min: [Face; 3],
    pub max: [Face; 3],
}

fn menuiserie() -> Materiau {
    Materiau::Menuiserie
}

/// Famille modélisée d'ouverture.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FamilleModelee {
    pub parametres: Vec<Parametre>,
    pub plans: Vec<PlanRef>,
    pub contraintes: Vec<Contrainte>,
    pub volumes: Vec<Volume>,
    #[serde(default = "couleur_defaut")]
    pub couleur: [u8; 3],
}

fn couleur_defaut() -> [u8; 3] {
    [235, 235, 230]
}

/// Résultat de la résolution : positions des plans, contraintes écartées (en conflit
/// avec une contrainte de priorité supérieure) et plans restés libres.
#[derive(Debug, Clone, Default)]
pub struct Resolution {
    pub positions: BTreeMap<String, f64>,
    pub valeurs: BTreeMap<String, f64>,
    pub conflits: Vec<usize>,
    pub libres: Vec<String>,
    pub erreurs: Vec<String>,
}

/// Boîte d'un volume résolu, dans le repère de la famille.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoiteVolume {
    pub indice: usize,
    pub min: DVec3,
    pub max: DVec3,
    pub vide: bool,
    pub materiau: Materiau,
}

/// Paramètres fournis par le type et l'hôte.
pub const LARGEUR: &str = "Largeur";
pub const HAUTEUR: &str = "Hauteur";
pub const EPAISSEUR: &str = "Epaisseur";

/// Évalue une expression : nombres (point ou virgule décimale), noms de paramètres,
/// + - * / et parenthèses.
pub fn evaluer(expr: &str, vars: &BTreeMap<String, f64>) -> Result<f64, String> {
    struct L<'a> {
        c: Vec<char>,
        i: usize,
        vars: &'a BTreeMap<String, f64>,
    }
    impl L<'_> {
        fn blancs(&mut self) {
            while self.c.get(self.i).is_some_and(|c| c.is_whitespace()) {
                self.i += 1;
            }
        }
        fn somme(&mut self) -> Result<f64, String> {
            let mut v = self.produit()?;
            loop {
                self.blancs();
                match self.c.get(self.i) {
                    Some('+') => { self.i += 1; v += self.produit()?; }
                    Some('-') => { self.i += 1; v -= self.produit()?; }
                    _ => return Ok(v),
                }
            }
        }
        fn produit(&mut self) -> Result<f64, String> {
            let mut v = self.facteur()?;
            loop {
                self.blancs();
                match self.c.get(self.i) {
                    Some('*') => { self.i += 1; v *= self.facteur()?; }
                    Some('/') => {
                        self.i += 1;
                        let d = self.facteur()?;
                        if d.abs() < 1e-12 { return Err("Division par zéro.".into()); }
                        v /= d;
                    }
                    _ => return Ok(v),
                }
            }
        }
        fn facteur(&mut self) -> Result<f64, String> {
            self.blancs();
            match self.c.get(self.i).copied() {
                Some('-') => { self.i += 1; Ok(-self.facteur()?) }
                Some('(') => {
                    self.i += 1;
                    let v = self.somme()?;
                    self.blancs();
                    if self.c.get(self.i) != Some(&')') { return Err("Parenthèse fermante attendue.".into()); }
                    self.i += 1;
                    Ok(v)
                }
                Some(c) if c.is_ascii_digit() || c == '.' || c == ',' => {
                    let d = self.i;
                    while self.c.get(self.i).is_some_and(|c| c.is_ascii_digit() || *c == '.' || *c == ',') { self.i += 1; }
                    let t: String = self.c[d..self.i].iter().map(|c| if *c == ',' { '.' } else { *c }).collect();
                    t.parse().map_err(|_| format!("Nombre illisible : {t}"))
                }
                Some(c) if c.is_alphabetic() || c == '_' => {
                    let d = self.i;
                    while self.c.get(self.i).is_some_and(|c| c.is_alphanumeric() || *c == '_') { self.i += 1; }
                    let nom: String = self.c[d..self.i].iter().collect();
                    self.vars.get(&nom).copied().ok_or_else(|| format!("Paramètre inconnu : {nom}"))
                }
                _ => Err("Expression incomplète.".into()),
            }
        }
    }
    let mut l = L { c: expr.chars().collect(), i: 0, vars };
    let v = l.somme()?;
    l.blancs();
    if l.i < l.c.len() {
        return Err(format!("Caractère inattendu : {}", l.c[l.i]));
    }
    if !v.is_finite() {
        return Err("Valeur non finie.".into());
    }
    Ok(v)
}

/// Équation linéaire sur les positions d'un axe : Σ coef·x = rhs.
#[derive(Clone)]
struct Equation {
    coefs: Vec<f64>,
    rhs: f64,
}

/// Système réduit tenu à jour équation par équation (élimination de Gauss-Jordan) :
/// une équation incompatible avec les précédentes est refusée, une équation
/// redondante ignorée.
struct Systeme {
    lignes: Vec<(usize, Equation)>,
}

enum Ajout {
    Ajoutee,
    Redondante,
    Conflit,
}

impl Systeme {
    fn ajouter(&mut self, mut e: Equation) -> Ajout {
        for (pivot, l) in &self.lignes {
            let c = e.coefs[*pivot];
            if c != 0.0 {
                for (k, v) in l.coefs.iter().enumerate() {
                    e.coefs[k] -= c * v;
                }
                e.rhs -= c * l.rhs;
            }
        }
        let Some((pivot, &c)) = e.coefs.iter().enumerate().max_by(|a, b| a.1.abs().total_cmp(&b.1.abs())) else { return Ajout::Redondante };
        if c.abs() < 1e-9 {
            return if e.rhs.abs() > 0.01 { Ajout::Conflit } else { Ajout::Redondante };
        }
        for v in &mut e.coefs {
            *v /= c;
        }
        e.rhs /= c;
        for (_, l) in &mut self.lignes {
            let f = l.coefs[pivot];
            if f != 0.0 {
                for (k, v) in e.coefs.iter().enumerate() {
                    l.coefs[k] -= f * v;
                }
                l.rhs -= f * e.rhs;
            }
        }
        self.lignes.push((pivot, e));
        Ajout::Ajoutee
    }
}

impl FamilleModelee {
    pub fn plan(&self, nom: &str) -> Option<&PlanRef> {
        self.plans.iter().find(|p| p.nom == nom)
    }

    /// Valeurs des paramètres : type (Largeur, Hauteur), hôte (Epaisseur), puis les
    /// formules dans l'ordre où elles deviennent calculables.
    pub fn valeurs(&self, largeur: f64, hauteur: f64, epaisseur: f64) -> (BTreeMap<String, f64>, Vec<String>) {
        let mut v = BTreeMap::from([(LARGEUR.to_string(), largeur), (HAUTEUR.to_string(), hauteur), (EPAISSEUR.to_string(), epaisseur)]);
        let mut restants: Vec<&Parametre> = self.parametres.iter().filter(|p| !v.contains_key(&p.nom)).collect();
        while !restants.is_empty() {
            let avant = restants.len();
            restants.retain(|p| match evaluer(&p.formule, &v) {
                Ok(x) => {
                    v.insert(p.nom.clone(), x);
                    false
                }
                Err(_) => true,
            });
            if restants.len() == avant {
                break;
            }
        }
        let erreurs = restants.iter().map(|p| format!("{} : {}", p.nom, evaluer(&p.formule, &v).err().unwrap_or_else(|| "référence circulaire".into()))).collect();
        (v, erreurs)
    }

    /// Résout les positions des plans, axe par axe. Ordre d'ajout : plans fixés et
    /// plans hôtes, contraintes par priorité décroissante (à égalité, dans l'ordre de
    /// création), puis les positions dessinées des plans encore libres.
    pub fn resoudre(&self, largeur: f64, hauteur: f64, epaisseur: f64) -> Resolution {
        let (valeurs, mut erreurs) = self.valeurs(largeur, hauteur, epaisseur);
        let mut r = Resolution { valeurs, ..Default::default() };
        let mut ordre: Vec<usize> = (0..self.contraintes.len()).collect();
        ordre.sort_by_key(|&i| std::cmp::Reverse(self.contraintes[i].priorite()));
        for axe in AxePlan::TOUS {
            let plans: Vec<&PlanRef> = self.plans.iter().filter(|p| p.axe == axe).collect();
            let indice = |nom: &str| plans.iter().position(|p| p.nom == nom);
            let n = plans.len();
            let mut s = Systeme { lignes: Vec::new() };
            let unite = |k: usize, v: f64| {
                let mut coefs = vec![0.0; n];
                coefs[k] = 1.0;
                Equation { coefs, rhs: v }
            };
            for (k, p) in plans.iter().enumerate() {
                let v = match p.hote {
                    Some(Hote::Exterieur) => epaisseur / 2.0,
                    Some(Hote::Interieur) => -epaisseur / 2.0,
                    None if p.fixe => p.position,
                    None => continue,
                };
                s.ajouter(unite(k, v));
            }
            for &i in &ordre {
                let c = &self.contraintes[i];
                let idx: Option<Vec<usize>> = c.plans().iter().map(|nom| indice(nom)).collect();
                let Some(idx) = idx else {
                    // contrainte d'un autre axe (ou plans absents : signalés une fois)
                    if axe == AxePlan::X && c.plans().iter().any(|nom| self.plan(nom).is_none()) {
                        erreurs.push(format!("Contrainte {} : plan absent.", i + 1));
                    }
                    continue;
                };
                let mut coefs = vec![0.0; n];
                let rhs = match c {
                    Contrainte::Distance { valeur, .. } => {
                        coefs[idx[1]] += 1.0;
                        coefs[idx[0]] -= 1.0;
                        match evaluer(valeur, &r.valeurs) {
                            Ok(v) => v,
                            Err(e) => {
                                erreurs.push(format!("Contrainte {} : {e}", i + 1));
                                continue;
                            }
                        }
                    }
                    Contrainte::Egalite { .. } => {
                        coefs[idx[0]] += 1.0;
                        coefs[idx[1]] -= 2.0;
                        coefs[idx[2]] += 1.0;
                        0.0
                    }
                };
                if let Ajout::Conflit = s.ajouter(Equation { coefs, rhs }) {
                    r.conflits.push(i);
                }
            }
            // plans libres : à leur position dessinée
            for (k, p) in plans.iter().enumerate() {
                if let Ajout::Ajoutee = s.ajouter(unite(k, p.position)) {
                    if !p.fixe && p.hote.is_none() && self.contraintes.iter().all(|c| !c.plans().contains(&p.nom.as_str())) {
                        r.libres.push(p.nom.clone());
                    }
                }
            }
            for (pivot, l) in &s.lignes {
                r.positions.insert(plans[*pivot].nom.clone(), l.rhs);
            }
        }
        r.conflits.sort_unstable();
        r.erreurs = erreurs;
        r
    }

    /// Boîtes des volumes pour une résolution donnée (faces dont le plan manque : 0).
    pub fn boites(&self, r: &Resolution) -> Vec<BoiteVolume> {
        let pos = |f: &Face| r.positions.get(&f.plan).copied().unwrap_or(0.0) + f.decalage;
        self.volumes
            .iter()
            .enumerate()
            .map(|(indice, v)| {
                let a = DVec3::new(pos(&v.min[0]), pos(&v.min[1]), pos(&v.min[2]));
                let b = DVec3::new(pos(&v.max[0]), pos(&v.max[1]), pos(&v.max[2]));
                BoiteVolume { indice, min: a.min(b), max: a.max(b), vide: v.vide, materiau: v.materiau }
            })
            .collect()
    }

    /// Contrôle avant enregistrement : noms uniques, références existantes, formules
    /// calculables, volumes non plats.
    pub fn valider(&self, largeur: f64, hauteur: f64, epaisseur: f64) -> Result<(), String> {
        let mut noms = std::collections::HashSet::new();
        for p in &self.plans {
            if p.nom.trim().is_empty() || !noms.insert(p.nom.as_str()) {
                return Err(format!("Plan de référence sans nom ou en double : « {} ».", p.nom));
            }
        }
        let mut params = std::collections::HashSet::from([LARGEUR, HAUTEUR, EPAISSEUR]);
        for p in &self.parametres {
            if p.nom.is_empty() || !p.nom.chars().all(|c| c.is_alphanumeric() || c == '_') || p.nom.starts_with(|c: char| c.is_ascii_digit()) {
                return Err(format!("Nom de paramètre non valide : « {} » (lettres, chiffres, _).", p.nom));
            }
            if !params.insert(p.nom.as_str()) {
                return Err(format!("Paramètre en double : {}.", p.nom));
            }
        }
        for v in &self.volumes {
            for f in v.min.iter().chain(&v.max) {
                if self.plan(&f.plan).is_none() {
                    return Err(format!("{} : plan « {} » absent.", v.nom, f.plan));
                }
            }
        }
        for (i, c) in self.contraintes.iter().enumerate() {
            let axes: Vec<Option<AxePlan>> = c.plans().iter().map(|n| self.plan(n).map(|p| p.axe)).collect();
            if axes.iter().any(Option::is_none) || axes.windows(2).any(|w| w[0] != w[1]) {
                return Err(format!("Contrainte {} : plans absents ou d'axes différents.", i + 1));
            }
        }
        let r = self.resoudre(largeur, hauteur, epaisseur);
        if let Some(e) = r.erreurs.first() {
            return Err(e.clone());
        }
        for b in self.boites(&r) {
            if (b.max - b.min).min_element() < 0.1 {
                return Err(format!("{} : volume plat.", self.volumes[b.indice].nom));
            }
        }
        Ok(())
    }

    /// Gabarit de fenêtre : plans de la baie, du dormant et du cadre, vide de baie,
    /// dormant, vitrage et appui ; Largeur et Hauteur viennent du type.
    pub fn gabarit_fenetre() -> FamilleModelee {
        use AxePlan::*;
        let plan = |nom: &str, axe, position, fixe| PlanRef { nom: nom.into(), axe, position, fixe, hote: None };
        let hote = |nom: &str, h| PlanRef { nom: nom.into(), axe: Y, position: 0.0, fixe: false, hote: Some(h) };
        let dist = |a: &str, b: &str, v: &str, priorite| Contrainte::Distance { a: a.into(), b: b.into(), valeur: v.into(), priorite };
        let face = |plan: &str, decalage| Face { plan: plan.into(), decalage };
        let volume = |nom: &str, vide, materiau, x: [(&str, f64); 2], y: [(&str, f64); 2], z: [(&str, f64); 2]| Volume {
            nom: nom.into(),
            vide,
            materiau,
            min: [face(x[0].0, x[0].1), face(y[0].0, y[0].1), face(z[0].0, z[0].1)],
            max: [face(x[1].0, x[1].1), face(y[1].0, y[1].1), face(z[1].0, z[1].1)],
        };
        FamilleModelee {
            parametres: vec![
                Parametre { nom: "Dormant".into(), formule: "60".into() },
                Parametre { nom: "Profondeur".into(), formule: "70".into() },
                Parametre { nom: "Retrait".into(), formule: "0".into() },
                Parametre { nom: "Vitrage".into(), formule: "28".into() },
                Parametre { nom: "Appui".into(), formule: "40".into() },
            ],
            plans: vec![
                plan("Centre", X, 0.0, true),
                plan("Gauche", X, -600.0, false),
                plan("Droite", X, 600.0, false),
                plan("Dormant gauche", X, -540.0, false),
                plan("Dormant droit", X, 540.0, false),
                hote("Extérieur", Hote::Exterieur),
                hote("Intérieur", Hote::Interieur),
                plan("Cadre ext", Y, 100.0, false),
                plan("Cadre int", Y, 30.0, false),
                plan("Nu appui", Y, 190.0, false),
                plan("Bas", Z, 0.0, true),
                plan("Haut", Z, 1350.0, false),
                plan("Dormant bas", Z, 60.0, false),
                plan("Dormant haut", Z, 1290.0, false),
            ],
            contraintes: vec![
                Contrainte::Egalite { a: "Gauche".into(), m: "Centre".into(), b: "Droite".into(), priorite: 9 },
                dist("Gauche", "Droite", LARGEUR, 9),
                dist("Bas", "Haut", HAUTEUR, 9),
                dist("Gauche", "Dormant gauche", "Dormant", 5),
                dist("Dormant droit", "Droite", "Dormant", 5),
                dist("Bas", "Dormant bas", "Dormant", 5),
                dist("Dormant haut", "Haut", "Dormant", 5),
                dist("Cadre ext", "Extérieur", "Retrait", 5),
                dist("Cadre int", "Cadre ext", "Profondeur", 5),
                dist("Extérieur", "Nu appui", "Appui", 5),
            ],
            volumes: vec![
                volume("Baie", true, Materiau::Menuiserie, [("Gauche", 0.0), ("Droite", 0.0)], [("Intérieur", -5.0), ("Extérieur", 5.0)], [("Bas", 0.0), ("Haut", 0.0)]),
                volume("Montant gauche", false, Materiau::Menuiserie, [("Gauche", 0.0), ("Dormant gauche", 0.0)], [("Cadre int", 0.0), ("Cadre ext", 0.0)], [("Bas", 0.0), ("Haut", 0.0)]),
                volume("Montant droit", false, Materiau::Menuiserie, [("Dormant droit", 0.0), ("Droite", 0.0)], [("Cadre int", 0.0), ("Cadre ext", 0.0)], [("Bas", 0.0), ("Haut", 0.0)]),
                volume("Traverse basse", false, Materiau::Menuiserie, [("Dormant gauche", 0.0), ("Dormant droit", 0.0)], [("Cadre int", 0.0), ("Cadre ext", 0.0)], [("Bas", 0.0), ("Dormant bas", 0.0)]),
                volume("Traverse haute", false, Materiau::Menuiserie, [("Dormant gauche", 0.0), ("Dormant droit", 0.0)], [("Cadre int", 0.0), ("Cadre ext", 0.0)], [("Dormant haut", 0.0), ("Haut", 0.0)]),
                volume("Vitrage", false, Materiau::Vitrage, [("Dormant gauche", 0.0), ("Dormant droit", 0.0)], [("Cadre int", 21.0), ("Cadre ext", -21.0)], [("Dormant bas", 0.0), ("Dormant haut", 0.0)]),
                volume("Appui", false, Materiau::Appui, [("Gauche", -20.0), ("Droite", 20.0)], [("Cadre ext", -10.0), ("Nu appui", 0.0)], [("Bas", -40.0), ("Bas", 0.0)]),
            ],
            couleur: couleur_defaut(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressions() {
        let v = BTreeMap::from([("Largeur".to_string(), 1200.0), ("Dormant".to_string(), 60.0)]);
        assert_eq!(evaluer("Largeur / 2 - Dormant", &v).unwrap(), 540.0);
        assert_eq!(evaluer("-(Largeur - 2 * Dormant) / 2", &v).unwrap(), -540.0);
        assert_eq!(evaluer("2,5 * 2", &v).unwrap(), 5.0);
        assert!(evaluer("Inconnu + 1", &v).is_err());
        assert!(evaluer("1 / 0", &v).is_err());
        assert!(evaluer("(1 + 2", &v).is_err());
    }

    #[test]
    fn gabarit_suit_les_cotes_du_type() {
        let f = FamilleModelee::gabarit_fenetre();
        f.valider(1400.0, 1600.0, 300.0).unwrap();
        let r = f.resoudre(1400.0, 1600.0, 300.0);
        assert!(r.conflits.is_empty() && r.erreurs.is_empty(), "{r:?}");
        let p = |n: &str| r.positions[n];
        assert!((p("Gauche") + 700.0).abs() < 1e-9 && (p("Droite") - 700.0).abs() < 1e-9);
        assert!((p("Dormant droit") - 640.0).abs() < 1e-9);
        assert!((p("Haut") - 1600.0).abs() < 1e-9);
        // cadre au nu extérieur, appui en débord
        assert!((p("Cadre ext") - 150.0).abs() < 1e-9 && (p("Cadre int") - 80.0).abs() < 1e-9);
        assert!((p("Nu appui") - 190.0).abs() < 1e-9);
        let boites = f.boites(&r);
        let baie = boites.iter().find(|b| b.vide).unwrap();
        assert!((baie.max.x - baie.min.x - 1400.0).abs() < 1e-9 && baie.max.y > 150.0 && baie.min.y < -150.0);
    }

    #[test]
    fn priorite_et_conflits() {
        let mut f = FamilleModelee::gabarit_fenetre();
        // cote de priorité basse qui contredit la largeur : écartée, la largeur gagne
        f.contraintes.push(Contrainte::Distance { a: "Gauche".into(), b: "Droite".into(), valeur: "1000".into(), priorite: 1 });
        let r = f.resoudre(1400.0, 1600.0, 300.0);
        assert_eq!(r.conflits, vec![f.contraintes.len() - 1]);
        assert!((r.positions["Droite"] - 700.0).abs() < 1e-9);
        // priorité la plus haute : c'est elle qui gagne, la largeur du type est écartée
        *f.contraintes.last_mut().unwrap().priorite_mut() = 10;
        let r = f.resoudre(1400.0, 1600.0, 300.0);
        assert!((r.positions["Droite"] - r.positions["Gauche"] - 1000.0).abs() < 1e-9);
        assert_eq!(r.conflits, vec![1]);
        // plan libre : reste à sa position dessinée
        f.plans.push(PlanRef { nom: "Repère".into(), axe: AxePlan::X, position: 123.0, fixe: false, hote: None });
        let r = f.resoudre(1400.0, 1600.0, 300.0);
        assert_eq!(r.positions["Repère"], 123.0);
        assert!(r.libres.contains(&"Repère".to_string()));
    }
}
