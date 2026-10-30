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
    #[serde(default, skip_serializing_if = "Portee::est_type")]
    pub portee: Portee,
}

/// Portée d'un paramètre : valeur de la famille (type), propre à chaque ouverture
/// placée (occurrence, la formule donne la valeur par défaut), ou partagée avec le
/// projet (table `parametres_partages` ; la formule sert à défaut).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Portee {
    #[default]
    Type,
    Occurrence,
    Partage,
}

impl Portee {
    pub const TOUTES: [Portee; 3] = [Self::Type, Self::Occurrence, Self::Partage];
    pub fn est_type(&self) -> bool {
        *self == Self::Type
    }
    pub fn nom(self) -> &'static str {
        match self {
            Self::Type => "Type",
            Self::Occurrence => "Occurrence",
            Self::Partage => "Partagé",
        }
    }
}

/// Vue d'esquisse d'un profil ou d'un symbole : ses deux axes, l'extrusion suit le
/// troisième.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VueEsquisse {
    /// Élévation : largeur × hauteur (extrusion dans l'épaisseur).
    Elevation,
    /// Plan : largeur × épaisseur (extrusion en hauteur).
    Plan,
    /// Coupe : épaisseur × hauteur (extrusion en largeur).
    Coupe,
}

impl VueEsquisse {
    /// Axes horizontal et vertical de l'esquisse, puis axe d'extrusion.
    pub fn axes(self) -> (AxePlan, AxePlan, AxePlan) {
        match self {
            Self::Elevation => (AxePlan::X, AxePlan::Z, AxePlan::Y),
            Self::Plan => (AxePlan::X, AxePlan::Y, AxePlan::Z),
            Self::Coupe => (AxePlan::Y, AxePlan::Z, AxePlan::X),
        }
    }
    /// Point de la famille depuis (h, v, profondeur).
    pub fn point(self, h: f64, v: f64, d: f64) -> DVec3 {
        let (ah, av, ad) = self.axes();
        let mut p = [0.0; 3];
        p[ah as usize] = h;
        p[av as usize] = v;
        p[ad as usize] = d;
        DVec3::from_array(p)
    }
}

/// Sommet d'esquisse verrouillé : une face par axe de l'esquisse.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PointEsquisse {
    pub h: Face,
    pub v: Face,
}

/// Extrusion d'un profil quelconque (polygone fermé) entre deux faces.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Extrusion {
    pub nom: String,
    #[serde(default)]
    pub vide: bool,
    #[serde(default = "menuiserie")]
    pub materiau: Materiau,
    pub vue: VueEsquisse,
    pub profil: Vec<PointEsquisse>,
    pub profondeur: [Face; 2],
}

/// Symbole 2D affiché dans les vues du projet : plan (sens d'ouverture, débattement)
/// ou élévation (triangles d'ouverture). Arc : trois points (départ, passage, fin).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Symbole {
    pub vue: VueEsquisse,
    pub points: Vec<PointEsquisse>,
    #[serde(default)]
    pub arc: bool,
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

/// Sommet d'esquisse : plan et décalage sur chacun des deux axes.
fn point(h: &str, dh: f64, v: &str, dv: f64) -> PointEsquisse {
    PointEsquisse { h: Face { plan: h.into(), decalage: dh }, v: Face { plan: v.into(), decalage: dv } }
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extrusions: Vec<Extrusion>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symboles: Vec<Symbole>,
    #[serde(default = "couleur_defaut")]
    pub couleur: [u8; 3],
    /// Catégorie : range le type parmi les portes ou les fenêtres.
    #[serde(default = "fenetre")]
    pub genre: super::GenreOuverture,
}

fn fenetre() -> super::GenreOuverture {
    super::GenreOuverture::Fenetre
}

/// Solide résolu dans le repère de la famille : triangles, vide, matériau.
#[derive(Debug, Clone)]
pub struct SolideFamille {
    pub tris: Vec<[DVec3; 3]>,
    pub vide: bool,
    pub materiau: Materiau,
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
    /// Valeurs avec `surcharges` : valeurs d'occurrence (ouverture placée) et
    /// paramètres partagés (projet), qui remplacent la formule des paramètres de même
    /// nom et de cette portée.
    pub fn valeurs_avec(&self, largeur: f64, hauteur: f64, epaisseur: f64, surcharges: &BTreeMap<String, f64>) -> (BTreeMap<String, f64>, Vec<String>) {
        let mut v = BTreeMap::from([(LARGEUR.to_string(), largeur), (HAUTEUR.to_string(), hauteur), (EPAISSEUR.to_string(), epaisseur)]);
        for p in self.parametres.iter().filter(|p| p.portee != Portee::Type) {
            if let Some(x) = surcharges.get(&p.nom).filter(|x| x.is_finite()) {
                v.insert(p.nom.clone(), *x);
            }
        }
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
        self.resoudre_avec(largeur, hauteur, epaisseur, &BTreeMap::new())
    }

    /// Résolution avec valeurs d'occurrence et paramètres partagés (`valeurs_avec`).
    pub fn resoudre_avec(&self, largeur: f64, hauteur: f64, epaisseur: f64, surcharges: &BTreeMap<String, f64>) -> Resolution {
        let (valeurs, mut erreurs) = self.valeurs_avec(largeur, hauteur, epaisseur, surcharges);
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

    /// Point d'esquisse résolu (h, v).
    pub fn point_esquisse(&self, r: &Resolution, p: &PointEsquisse) -> (f64, f64) {
        let pos = |f: &Face| r.positions.get(&f.plan).copied().unwrap_or(0.0) + f.decalage;
        (pos(&p.h), pos(&p.v))
    }

    /// Volumes et extrusions résolus, en triangles dans le repère de la famille.
    pub fn solides(&self, r: &Resolution) -> Vec<SolideFamille> {
        let pos = |f: &Face| r.positions.get(&f.plan).copied().unwrap_or(0.0) + f.decalage;
        let mut out: Vec<SolideFamille> = self
            .boites(r)
            .into_iter()
            .map(|b| {
                let contour = [[b.min.x, b.min.y], [b.max.x, b.min.y], [b.max.x, b.max.y], [b.min.x, b.max.y]];
                SolideFamille { tris: super::solides::prisme(&contour, b.min.z, b.max.z), vide: b.vide, materiau: b.materiau }
            })
            .collect();
        for e in &self.extrusions {
            let contour: Vec<[f64; 2]> = e.profil.iter().map(|p| { let (h, v) = self.point_esquisse(r, p); [h, v] }).collect();
            let (d0, d1) = (pos(&e.profondeur[0]), pos(&e.profondeur[1]));
            // prisme local (h, v, d) ramené aux axes de la famille ; une permutation
            // impaire (élévation) retourne les faces
            let impaire = e.vue == VueEsquisse::Elevation;
            let tris = super::solides::prisme(&contour, d0.min(d1), d0.max(d1))
                .into_iter()
                .map(|t| {
                    let m = t.map(|p| e.vue.point(p.x, p.y, p.z));
                    if impaire { [m[0], m[2], m[1]] } else { m }
                })
                .collect();
            out.push(SolideFamille { tris, vide: e.vide, materiau: e.materiau });
        }
        out
    }

    /// Symboles de la vue `vue` résolus en polylignes (h, v) ; un arc est discrétisé.
    pub fn symboles_resolus(&self, r: &Resolution, vue: VueEsquisse) -> Vec<Vec<(f64, f64)>> {
        self.symboles
            .iter()
            .filter(|s| s.vue == vue)
            .map(|s| {
                let pts: Vec<(f64, f64)> = s.points.iter().map(|p| self.point_esquisse(r, p)).collect();
                if s.arc && pts.len() == 3 {
                    let v = |p: (f64, f64)| glam::DVec2::new(p.0, p.1);
                    if let Some((o, rayon, depart, balayage)) = crate::dessin2d::arc_3_points(v(pts[0]), v(pts[1]), v(pts[2])) {
                        return crate::dessin2d::points_arc(o, rayon, depart, balayage).into_iter().map(|p| (p.x, p.y)).collect();
                    }
                }
                pts
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
        for e in &self.extrusions {
            let (ah, av, ad) = e.vue.axes();
            let bon = |f: &Face, axe| self.plan(&f.plan).is_some_and(|p| p.axe == axe);
            if e.profil.len() < 3 || !e.profil.iter().all(|p| bon(&p.h, ah) && bon(&p.v, av)) || !e.profondeur.iter().all(|f| bon(f, ad)) {
                return Err(format!("{} : profil d'au moins trois sommets sur des plans de la vue d'esquisse attendu.", e.nom));
            }
        }
        for (i, s) in self.symboles.iter().enumerate() {
            let (ah, av, _) = s.vue.axes();
            if s.points.len() < 2 || (s.arc && s.points.len() != 3) || !s.points.iter().all(|p| self.plan(&p.h.plan).is_some_and(|x| x.axe == ah) && self.plan(&p.v.plan).is_some_and(|x| x.axe == av)) {
                return Err(format!("Symbole {} : points sur des plans absents.", i + 1));
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
                Parametre { nom: "Dormant".into(), formule: "60".into(), portee: Portee::Type },
                Parametre { nom: "Profondeur".into(), formule: "70".into(), portee: Portee::Type },
                Parametre { nom: "Retrait".into(), formule: "0".into(), portee: Portee::Type },
                Parametre { nom: "Vitrage".into(), formule: "28".into(), portee: Portee::Type },
                Parametre { nom: "Appui".into(), formule: "40".into(), portee: Portee::Type },
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
                plan("Mi-hauteur", Z, 675.0, false),
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
                Contrainte::Egalite { a: "Dormant bas".into(), m: "Mi-hauteur".into(), b: "Dormant haut".into(), priorite: 5 },
            ],
            volumes: vec![
                volume("Baie", true, Materiau::Menuiserie, [("Gauche", 0.0), ("Droite", 0.0)], [("Intérieur", -5.0), ("Extérieur", 5.0)], [("Bas", 0.0), ("Haut", 0.0)]),
                volume("Montant gauche", false, Materiau::Menuiserie, [("Gauche", 0.0), ("Dormant gauche", 0.0)], [("Cadre int", 0.0), ("Cadre ext", 0.0)], [("Bas", 0.0), ("Haut", 0.0)]),
                volume("Montant droit", false, Materiau::Menuiserie, [("Dormant droit", 0.0), ("Droite", 0.0)], [("Cadre int", 0.0), ("Cadre ext", 0.0)], [("Bas", 0.0), ("Haut", 0.0)]),
                volume("Traverse basse", false, Materiau::Menuiserie, [("Dormant gauche", 0.0), ("Dormant droit", 0.0)], [("Cadre int", 0.0), ("Cadre ext", 0.0)], [("Bas", 0.0), ("Dormant bas", 0.0)]),
                volume("Traverse haute", false, Materiau::Menuiserie, [("Dormant gauche", 0.0), ("Dormant droit", 0.0)], [("Cadre int", 0.0), ("Cadre ext", 0.0)], [("Dormant haut", 0.0), ("Haut", 0.0)]),
                volume("Vitrage", false, Materiau::Vitrage, [("Dormant gauche", 0.0), ("Dormant droit", 0.0)], [("Cadre int", 21.0), ("Cadre ext", -21.0)], [("Dormant bas", 0.0), ("Dormant haut", 0.0)]),
            ],
            // appui en pente : profil esquissé en coupe, extrudé sur la largeur
            extrusions: vec![Extrusion {
                nom: "Appui".into(),
                vide: false,
                materiau: Materiau::Appui,
                vue: VueEsquisse::Coupe,
                profil: vec![point("Cadre ext", -10.0, "Bas", 0.0), point("Nu appui", 0.0, "Bas", -20.0), point("Nu appui", 0.0, "Bas", -45.0), point("Cadre ext", -10.0, "Bas", -45.0)],
                profondeur: [face("Gauche", -20.0), face("Droite", 20.0)],
            }],
            // ouvrant à la française, paumelles à gauche (pointe du triangle)
            symboles: vec![Symbole {
                vue: VueEsquisse::Elevation,
                points: vec![point("Dormant droit", 0.0, "Dormant bas", 0.0), point("Dormant gauche", 0.0, "Mi-hauteur", 0.0), point("Dormant droit", 0.0, "Dormant haut", 0.0)],
                arc: false,
            }],
            couleur: couleur_defaut(),
            genre: super::GenreOuverture::Fenetre,
        }
    }

    /// Gabarit de famille selon la catégorie.
    pub fn gabarit(genre: super::GenreOuverture) -> FamilleModelee {
        match genre {
            super::GenreOuverture::Porte => Self::gabarit_porte(),
            _ => Self::gabarit_fenetre(),
        }
    }

    /// Gabarit de porte : baie (vide) depuis le sol, dormant sur trois côtés, vantail
    /// plein, débattement en plan (vantail ouvert à 90° vers l'intérieur, arc) et
    /// triangle d'ouverture en élévation.
    pub fn gabarit_porte() -> FamilleModelee {
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
        let param = |nom: &str, formule: &str| Parametre { nom: nom.into(), formule: formule.into(), portee: Portee::Type };
        FamilleModelee {
            parametres: vec![
                param("Dormant", "60"),
                param("Profondeur", "70"),
                param("Retrait", "0"),
                param("Vantail", "40"),
                param("Passage", "Largeur - 2 * Dormant"),
                param("Diagonale", "Passage * 0,7071"),
            ],
            plans: vec![
                plan("Centre", X, 0.0, true),
                plan("Gauche", X, -450.0, false),
                plan("Droite", X, 450.0, false),
                plan("Dormant gauche", X, -390.0, false),
                plan("Dormant droit", X, 390.0, false),
                plan("Arc x", X, 0.0, false),
                hote("Extérieur", Hote::Exterieur),
                hote("Intérieur", Hote::Interieur),
                plan("Cadre ext", Y, 100.0, false),
                plan("Cadre int", Y, 30.0, false),
                plan("Ouvert", Y, -800.0, false),
                plan("Arc y", Y, -500.0, false),
                plan("Bas", Z, 0.0, true),
                plan("Haut", Z, 2150.0, false),
                plan("Dormant haut", Z, 2090.0, false),
                plan("Mi-hauteur", Z, 1045.0, false),
            ],
            contraintes: vec![
                Contrainte::Egalite { a: "Gauche".into(), m: "Centre".into(), b: "Droite".into(), priorite: 9 },
                dist("Gauche", "Droite", LARGEUR, 9),
                dist("Bas", "Haut", HAUTEUR, 9),
                dist("Gauche", "Dormant gauche", "Dormant", 5),
                dist("Dormant droit", "Droite", "Dormant", 5),
                dist("Dormant haut", "Haut", "Dormant", 5),
                dist("Cadre ext", "Extérieur", "Retrait", 5),
                dist("Cadre int", "Cadre ext", "Profondeur", 5),
                dist("Ouvert", "Cadre int", "Passage", 5),
                dist("Dormant gauche", "Arc x", "Diagonale", 5),
                dist("Arc y", "Cadre int", "Diagonale", 5),
                Contrainte::Egalite { a: "Bas".into(), m: "Mi-hauteur".into(), b: "Dormant haut".into(), priorite: 5 },
            ],
            volumes: vec![
                volume("Baie", true, Materiau::Menuiserie, [("Gauche", 0.0), ("Droite", 0.0)], [("Intérieur", -5.0), ("Extérieur", 5.0)], [("Bas", 0.0), ("Haut", 0.0)]),
                volume("Montant gauche", false, Materiau::Menuiserie, [("Gauche", 0.0), ("Dormant gauche", 0.0)], [("Cadre int", 0.0), ("Cadre ext", 0.0)], [("Bas", 0.0), ("Haut", 0.0)]),
                volume("Montant droit", false, Materiau::Menuiserie, [("Dormant droit", 0.0), ("Droite", 0.0)], [("Cadre int", 0.0), ("Cadre ext", 0.0)], [("Bas", 0.0), ("Haut", 0.0)]),
                volume("Traverse haute", false, Materiau::Menuiserie, [("Dormant gauche", 0.0), ("Dormant droit", 0.0)], [("Cadre int", 0.0), ("Cadre ext", 0.0)], [("Dormant haut", 0.0), ("Haut", 0.0)]),
                volume("Vantail", false, Materiau::Panneau, [("Dormant gauche", 2.0), ("Dormant droit", -2.0)], [("Cadre int", 0.0), ("Cadre int", 40.0)], [("Bas", 10.0), ("Dormant haut", -2.0)]),
            ],
            extrusions: Vec::new(),
            symboles: vec![
                // vantail ouvert à 90° vers l'intérieur et arc de débattement
                Symbole { vue: VueEsquisse::Plan, points: vec![point("Dormant gauche", 0.0, "Cadre int", 0.0), point("Dormant gauche", 0.0, "Ouvert", 0.0)], arc: false },
                Symbole { vue: VueEsquisse::Plan, points: vec![point("Dormant droit", 0.0, "Cadre int", 0.0), point("Arc x", 0.0, "Arc y", 0.0), point("Dormant gauche", 0.0, "Ouvert", 0.0)], arc: true },
                Symbole { vue: VueEsquisse::Elevation, points: vec![point("Dormant droit", 0.0, "Bas", 0.0), point("Dormant gauche", 0.0, "Mi-hauteur", 0.0), point("Dormant droit", 0.0, "Dormant haut", 0.0)], arc: false },
            ],
            couleur: [190, 160, 120],
            genre: super::GenreOuverture::Porte,
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
    fn extrusions_symboles_et_surcharges() {
        let f = FamilleModelee::gabarit_fenetre();
        let r = f.resoudre(1400.0, 1600.0, 300.0);
        let solides = f.solides(&r);
        // appui extrudé : volume fermé, en débord vers l'extérieur, sous la baie
        let appui = solides.iter().find(|s| s.materiau == Materiau::Appui).unwrap();
        let v = crate::modele::geometrie::volume(&appui.tris);
        assert!(v > 0.0, "{v}");
        assert!(appui.tris.iter().flatten().all(|p| p.z <= 1e-9) && appui.tris.iter().flatten().any(|p| p.y > 150.0));
        let s = f.symboles_resolus(&r, VueEsquisse::Elevation);
        assert_eq!(s.len(), 1);
        assert!((s[0][1].0 + 640.0).abs() < 1e-9 && (s[0][1].1 - 800.0).abs() < 1e-9);
        // porte : arc de débattement d'un quart de tour, du rayon du passage
        let p = FamilleModelee::gabarit_porte();
        p.valider(900.0, 2150.0, 200.0).unwrap();
        let r = p.resoudre(900.0, 2150.0, 200.0);
        assert!(r.conflits.is_empty(), "{:?}", r.conflits);
        let arc = &p.symboles_resolus(&r, VueEsquisse::Plan)[1];
        let pivot = (r.positions["Dormant gauche"], r.positions["Cadre int"]);
        for q in arc {
            assert!(((q.0 - pivot.0).hypot(q.1 - pivot.1) - 780.0).abs() < 1.0, "{q:?}");
        }
        // paramètre d'occurrence : surchargé par l'ouverture placée
        let mut g = FamilleModelee::gabarit_fenetre();
        g.parametres.iter_mut().find(|p| p.nom == "Retrait").unwrap().portee = Portee::Occurrence;
        let r = g.resoudre_avec(1400.0, 1600.0, 300.0, &BTreeMap::from([("Retrait".to_string(), 50.0)]));
        assert!((r.positions["Cadre ext"] - 100.0).abs() < 1e-9);
        // un paramètre de type ne l'est pas
        let r = FamilleModelee::gabarit_fenetre().resoudre_avec(1400.0, 1600.0, 300.0, &BTreeMap::from([("Retrait".to_string(), 50.0)]));
        assert!((r.positions["Cadre ext"] - 150.0).abs() < 1e-9);
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
