//! Modèle natif v0 : le format d'édition, source de vérité d'un projet (voir
//! docs/MODELE.md). Un fichier Revit ou IFC est converti vers ce modèle : ce qui est
//! reconnu devient paramétrique (niveaux, murs, ouvertures, dalles), le reste un élément
//! importé (géométrie figée et propriétés). Chaque élément garde son origine (fichier,
//! identifiant source) pour réimporter une version suivante.
//!
//! Unités : millimètres ; coordonnées du fichier source (Revit : origine interne ; IFC :
//! repère du projet).

pub mod assemblage;
pub mod depuis_ifc;
pub mod depuis_rvt;
pub mod geometrie;
pub mod jonctions;
pub mod edition;
pub mod enregistrement;
pub mod rapport;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Nom du format, en tête de chaque fichier.
pub const FORMAT: &str = "ifc_viewer-modele";
/// Version du schéma ; toute évolution incompatible l'incrémente (migrations).
pub const VERSION: u32 = 0;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Projet {
    pub format: String,
    pub version: u32,
    pub nom: String,
    pub sources: Vec<Source>,
    #[serde(default)]
    pub phases: Vec<String>,
    #[serde(default)]
    pub sous_projets: Vec<String>,
    #[serde(default)]
    pub variantes: Vec<Variante>,
    pub niveaux: Vec<Niveau>,
    pub types_murs: Vec<TypeCompose>,
    pub types_dalles: Vec<TypeCompose>,
    pub types_ouvertures: Vec<TypeOuverture>,
    pub murs: Vec<Mur>,
    /// Murs composés : chaque couche y est un mur à part entière (voir [`Assemblage`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assemblages: Vec<Assemblage>,
    /// Matériaux nommés par les couches : couleur et motif de coupe.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub materiaux: Vec<Materiau>,
    pub ouvertures: Vec<Ouverture>,
    pub dalles: Vec<Dalle>,
    pub importes: Vec<ElementImporte>,
    /// Éléments de la source qui n'ont rien donné (sans géométrie, hors du périmètre v0),
    /// comptés par catégorie.
    pub ignores: BTreeMap<String, usize>,
}

/// Option de conception ; les identifiants distinguent les options homonymes.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Variante {
    pub id: String,
    pub nom: String,
    pub jeu: String,
}

impl Variante {
    pub fn libelle(&self) -> String {
        if self.jeu.is_empty() { self.nom.clone() } else { format!("{} — {}", self.jeu, self.nom) }
    }
}

/// Fichier d'où vient une partie du projet.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Source {
    /// Clé courte (« s1 ») reprise par les origines.
    pub id: String,
    pub fichier: String,
    /// « RVT » ou « IFC ».
    pub format: String,
    pub octets: u64,
    /// Date de modification du fichier (secondes depuis 1970), 0 si inconnue.
    pub modifie: u64,
}

/// Élément source dont vient un élément du modèle.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Origine {
    pub source: String,
    /// ElementId Revit, ou numéro d'entité STEP (« #123 »).
    pub id: String,
    /// IfcGUID (IFC ; Revit 2024 et suivants).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub guid: Option<String>,
    /// Classe source (`SWall`, `IFCWALL`…).
    pub classe: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Niveau {
    pub id: String,
    pub nom: String,
    /// Altitude, mm.
    pub altitude: f64,
    pub origine: Option<Origine>,
}

/// Couche d'un type composé, de l'extérieur vers l'intérieur (murs) ou du haut vers le
/// bas (dalles).
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct Couche {
    pub materiau: Option<String>,
    /// Épaisseur, mm.
    pub epaisseur: f64,
    #[serde(default, skip_serializing_if = "FonctionCouche::est_autre")]
    pub fonction: FonctionCouche,
}

/// Rôle d'une couche ; la structure sert de référence d'implantation (axe du porteur).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FonctionCouche {
    Structure,
    Isolation,
    Finition,
    #[default]
    Autre,
}

impl FonctionCouche {
    pub const TOUTES: [FonctionCouche; 4] = [Self::Structure, Self::Isolation, Self::Finition, Self::Autre];

    fn est_autre(&self) -> bool {
        *self == Self::Autre
    }

    pub fn libelle(self) -> &'static str {
        match self {
            Self::Structure => "Structure",
            Self::Isolation => "Isolation",
            Self::Finition => "Finition",
            Self::Autre => "Autre",
        }
    }
}

/// Motif de coupe d'un matériau (poché des plans et des coupes).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MotifCoupe {
    /// Hachures à 45°.
    #[default]
    Diagonales,
    /// Hachures croisées (isolants).
    Croisees,
    /// Aplat de la couleur du matériau.
    Uni,
    /// Contour seul.
    Vide,
}

impl MotifCoupe {
    pub const TOUS: [MotifCoupe; 4] = [Self::Diagonales, Self::Croisees, Self::Uni, Self::Vide];

    pub fn libelle(self) -> &'static str {
        match self {
            Self::Diagonales => "Diagonales",
            Self::Croisees => "Croisées",
            Self::Uni => "Uni",
            Self::Vide => "Vide",
        }
    }
}

/// Matériau nommé par les couches (`Couche::materiau`).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Materiau {
    pub nom: String,
    /// Couleur d'affichage (sRGB).
    pub couleur: [u8; 3],
    #[serde(default)]
    pub motif: MotifCoupe,
}

/// Mur composé dont chaque couche est un mur indépendant (son type monocouche, ses
/// contraintes de hauteur, sa phase, son sous-projet), regroupé pour être sélectionné,
/// tracé et hébergé ensemble. Une ouverture dont `mur` vaut l'identifiant de
/// l'assemblage traverse toutes ses couches.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Assemblage {
    pub id: String,
    pub nom: String,
    /// Type composé d'origine (couches de l'extérieur vers l'intérieur).
    pub type_mur: String,
    /// Murs des couches, de l'extérieur vers l'intérieur.
    pub couches: Vec<String>,
    pub origine: Option<Origine>,
}

/// Type de mur ou de dalle : ses couches.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TypeCompose {
    pub id: String,
    pub nom: String,
    pub couches: Vec<Couche>,
    /// Couches lues dans la source (false : une couche unique de l'épaisseur totale,
    /// détail non disponible).
    pub couches_detaillees: bool,
    pub origine: Option<Origine>,
}

impl TypeCompose {
    pub fn epaisseur(&self) -> f64 {
        self.couches.iter().map(|c| c.epaisseur).sum()
    }
}

/// Axe d'un mur dans le plan (x, y en mm) : ligne médiane de son épaisseur.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(tag = "forme", rename_all = "snake_case")]
pub enum Axe {
    Droit { debut: [f64; 2], fin: [f64; 2] },
    /// Arc de `debut` à `fin` autour de `centre`, dans le sens trigonométrique si
    /// `direct`.
    Arc { debut: [f64; 2], fin: [f64; 2], centre: [f64; 2], direct: bool },
}

impl Axe {
    /// Rayon et angles (début, balayage signé) d'un arc.
    fn arc(&self) -> Option<(f64, f64, f64)> {
        let Axe::Arc { debut, fin, centre, direct } = *self else { return None };
        let r = ((debut[0] - centre[0]).powi(2) + (debut[1] - centre[1]).powi(2)).sqrt();
        let a0 = (debut[1] - centre[1]).atan2(debut[0] - centre[0]);
        let a1 = (fin[1] - centre[1]).atan2(fin[0] - centre[0]);
        let mut balayage = a1 - a0;
        let tour = std::f64::consts::TAU;
        if direct {
            balayage = balayage.rem_euclid(tour);
        } else {
            balayage = -(-balayage).rem_euclid(tour);
        }
        Some((r, a0, balayage))
    }

    pub fn longueur(&self) -> f64 {
        match *self {
            Axe::Droit { debut, fin } => ((fin[0] - debut[0]).powi(2) + (fin[1] - debut[1]).powi(2)).sqrt(),
            Axe::Arc { .. } => self.arc().map_or(0.0, |(r, _, b)| r * b.abs()),
        }
    }

    /// Abscisse curviligne de la projection de `p` sur l'axe (bornée à l'axe).
    pub fn abscisse(&self, p: [f64; 2]) -> f64 {
        match *self {
            Axe::Droit { debut, fin } => {
                let (dx, dy) = (fin[0] - debut[0], fin[1] - debut[1]);
                let l = (dx * dx + dy * dy).sqrt().max(1e-12);
                (((p[0] - debut[0]) * dx + (p[1] - debut[1]) * dy) / l).clamp(0.0, l)
            }
            Axe::Arc { centre, .. } => {
                let (r, a0, b) = self.arc().unwrap_or((0.0, 0.0, 0.0));
                let a = (p[1] - centre[1]).atan2(p[0] - centre[0]);
                let d = (a - a0) * b.signum();
                (d.rem_euclid(std::f64::consts::TAU) * r).min(self.longueur())
            }
        }
    }
}

/// Haut d'un mur : hauteur libre, ou attaché à un niveau (avec décalage).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "contrainte", rename_all = "snake_case")]
pub enum Haut {
    Libre { hauteur: f64 },
    Niveau { niveau: String, decalage: f64 },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Mur {
    /// None : modèle principal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variante: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sous_projet: Option<String>,
    pub id: String,
    pub nom: String,
    pub type_mur: String,
    /// Assemblage dont ce mur est une couche ; None : mur indépendant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assemblage: Option<String>,
    /// Niveau de base et décalage de la base (mm).
    pub niveau: String,
    pub decalage_bas: f64,
    pub haut: Haut,
    pub axe: Axe,
    pub origine: Option<Origine>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GenreOuverture {
    Porte,
    Fenetre,
    /// Trémie sans menuiserie.
    Baie,
}

/// Type de porte ou de fenêtre : dimensions de la baie qu'il ouvre.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TypeOuverture {
    pub id: String,
    pub nom: String,
    pub genre: GenreOuverture,
    pub largeur: f64,
    pub hauteur: f64,
    pub origine: Option<Origine>,
}

/// Ouverture hébergée par un mur : baie rectangulaire à travers toute son épaisseur.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Ouverture {
    pub id: String,
    pub nom: String,
    pub genre: GenreOuverture,
    pub mur: String,
    pub type_ouverture: Option<String>,
    /// Abscisse du milieu de la baie le long de l'axe du mur, depuis son début (mm).
    pub abscisse: f64,
    pub largeur: f64,
    pub hauteur: f64,
    /// Hauteur du bas de la baie au-dessus du niveau du mur (mm).
    pub allege: f64,
    /// Menuiserie telle que lue (affichage, en attendant des familles paramétriques).
    pub representation: Option<Maillage>,
    pub origine: Option<Origine>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Dalle {
    /// None : modèle principal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variante: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sous_projet: Option<String>,
    pub id: String,
    pub nom: String,
    pub type_dalle: String,
    pub niveau: String,
    /// Altitude du dessus par rapport au niveau (mm).
    pub decalage: f64,
    /// Contours dans le plan (mm) : extérieurs et trous, règle pair-impair.
    pub contours: Vec<Vec<[f64; 2]>>,
    pub origine: Option<Origine>,
}

/// Élément non reconnu : géométrie figée, déplaçable mais pas modifiable dans sa forme.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ElementImporte {
    pub id: String,
    pub nom: String,
    pub categorie: String,
    pub niveau: Option<String>,
    pub maillage: Maillage,
    /// Pourquoi l'élément n'est pas paramétrique.
    pub raison: String,
    pub origine: Option<Origine>,
}

/// Maillage indexé (mm).
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Maillage {
    pub sommets: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

impl Maillage {
    /// Maillage indexé d'une soupe de triangles (sommets fusionnés à 10⁻³ mm).
    pub fn depuis_triangles(tris: &[[glam::DVec3; 3]]) -> Maillage {
        let mut m = Maillage::default();
        let mut index: std::collections::HashMap<[i64; 3], u32> = std::collections::HashMap::new();
        for t in tris {
            let ids = t.map(|p| {
                let cle = [p.x, p.y, p.z].map(|c| (c * 1000.0).round() as i64);
                *index.entry(cle).or_insert_with(|| {
                    m.sommets.push([p.x, p.y, p.z]);
                    (m.sommets.len() - 1) as u32
                })
            });
            if ids[0] != ids[1] && ids[1] != ids[2] && ids[0] != ids[2] {
                m.triangles.push(ids);
            }
        }
        m
    }
}

impl Projet {
    pub fn nouveau(nom: &str) -> Projet {
        Projet { format: FORMAT.into(), version: VERSION, nom: nom.into(), ..Default::default() }
    }

    pub fn niveau(&self, id: &str) -> Option<&Niveau> {
        self.niveaux.iter().find(|n| n.id == id)
    }

    pub fn type_mur(&self, id: &str) -> Option<&TypeCompose> {
        self.types_murs.iter().find(|t| t.id == id)
    }

    pub fn assemblage(&self, id: &str) -> Option<&Assemblage> {
        self.assemblages.iter().find(|a| a.id == id)
    }

    pub fn materiau(&self, nom: &str) -> Option<&Materiau> {
        self.materiaux.iter().find(|m| m.nom == nom)
    }

    pub fn type_dalle(&self, id: &str) -> Option<&TypeCompose> {
        self.types_dalles.iter().find(|t| t.id == id)
    }

    /// Altitudes du bas et du haut d'un mur (mm).
    pub fn bas_haut(&self, m: &Mur) -> Option<(f64, f64)> {
        let bas = self.niveau(&m.niveau)?.altitude + m.decalage_bas;
        let haut = match &m.haut {
            Haut::Libre { hauteur } => bas + hauteur,
            Haut::Niveau { niveau, decalage } => self.niveau(niveau)?.altitude + decalage,
        };
        Some((bas, haut))
    }

    /// Volume d'un mur déduit de ses paramètres (mm³) : longueur de l'axe × épaisseur du
    /// type × hauteur, moins ses baies (bornées à la hauteur du mur). Les jonctions entre
    /// murs ne sont pas modélisées en v0.
    pub fn volume_mur(&self, m: &Mur) -> Option<f64> {
        let e = self.type_mur(&m.type_mur)?.epaisseur();
        let (bas, haut) = self.bas_haut(m)?;
        let h = haut - bas;
        let base = self.niveau(&m.niveau)?.altitude;
        let vides: f64 = self
            .ouvertures
            .iter()
            // baies du mur, ou de son assemblage (elles traversent toutes les couches)
            .filter(|o| o.mur == m.id || m.assemblage.as_ref() == Some(&o.mur))
            .map(|o| {
                let (z0, z1) = ((base + o.allege).max(bas), (base + o.allege + o.hauteur).min(haut));
                o.largeur * (z1 - z0).max(0.0)
            })
            .sum();
        Some((m.axe.longueur() * h - vides) * e)
    }

    /// Volume d'une dalle déduit de ses paramètres (mm³) : aire des contours × épaisseur
    /// du type.
    pub fn volume_dalle(&self, d: &Dalle) -> Option<f64> {
        Some(geometrie::aire_pair_impair(&d.contours) * self.type_dalle(&d.type_dalle)?.epaisseur())
    }

    /// Longueurs arrondies au micromètre (bruit des calculs en virgule flottante).
    pub fn arrondir(&mut self) {
        let r = |x: &mut f64| *x = (*x * 1000.0).round() / 1000.0 + 0.0;
        let r2 = |p: &mut [f64; 2]| p.iter_mut().for_each(r);
        let maillage = |m: &mut Maillage| m.sommets.iter_mut().flatten().for_each(r);
        self.niveaux.iter_mut().for_each(|n| r(&mut n.altitude));
        for t in self.types_murs.iter_mut().chain(self.types_dalles.iter_mut()) {
            t.couches.iter_mut().for_each(|c| r(&mut c.epaisseur));
        }
        for t in &mut self.types_ouvertures {
            r(&mut t.largeur);
            r(&mut t.hauteur);
        }
        for m in &mut self.murs {
            r(&mut m.decalage_bas);
            match &mut m.haut {
                Haut::Libre { hauteur } => r(hauteur),
                Haut::Niveau { decalage, .. } => r(decalage),
            }
            match &mut m.axe {
                Axe::Droit { debut, fin } => {
                    r2(debut);
                    r2(fin);
                }
                Axe::Arc { debut, fin, centre, .. } => {
                    r2(debut);
                    r2(fin);
                    r2(centre);
                }
            }
        }
        for o in &mut self.ouvertures {
            for x in [&mut o.abscisse, &mut o.largeur, &mut o.hauteur, &mut o.allege] {
                r(x);
            }
            if let Some(m) = &mut o.representation {
                maillage(m);
            }
        }
        for d in &mut self.dalles {
            r(&mut d.decalage);
            d.contours.iter_mut().flatten().for_each(r2);
        }
        self.importes.iter_mut().for_each(|i| maillage(&mut i.maillage));
    }

    pub fn json(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| e.to_string())
    }

    pub fn lire_json(texte: &str) -> Result<Projet, String> {
        let p: Projet = serde_json::from_str(texte).map_err(|e| e.to_string())?;
        if p.format != FORMAT {
            return Err(format!("format inconnu : {}", p.format));
        }
        if p.version > VERSION {
            return Err(format!("version {} plus récente que celle prise en charge ({VERSION})", p.version));
        }
        Ok(p)
    }
}

/// Écart de volume relatif au-delà duquel un mur ou une dalle reconnu reste un élément
/// importé : ses paramètres ne reproduisent pas assez fidèlement la géométrie lue.
pub const ECART_MAX: f64 = 0.05;

/// Comparaison, pour un élément reconnu, du volume lu dans la source et de celui déduit
/// de ses paramètres.
#[derive(Debug, Clone)]
pub struct Mesure {
    pub categorie: &'static str,
    pub nom: String,
    /// Volumes en mm³.
    pub source: f64,
    pub parametres: f64,
    /// Gardé paramétrique (écart dans la tolérance).
    pub retenu: bool,
}

impl Mesure {
    pub fn ecart(&self) -> f64 {
        if self.source.abs() < 1e-9 { f64::INFINITY } else { (self.parametres - self.source) / self.source }
    }
}

/// Conversion en cours : le projet, et pour chaque élément reconnu sa géométrie lue
/// (volume et maillage), qui sert à le valider et à le garder en élément importé s'il
/// est écarté.
#[derive(Default)]
pub struct Conversion {
    pub projet: Projet,
    pub lu: std::collections::HashMap<String, (f64, Maillage, String)>,
    pub mesures: Vec<Mesure>,
    /// Murs à plusieurs couches dont le côté extérieur est vérifié : vrai si l'axe doit
    /// être retourné pour avoir l'extérieur à sa gauche (première couche).
    pub sens_couches: std::collections::HashMap<String, bool>,
    /// Couleurs des matériaux lues dans la source (nom → sRGB).
    pub couleurs: std::collections::HashMap<String, [u8; 3]>,
}

impl Conversion {
    /// Retient la géométrie lue d'un élément reconnu (catégorie pour un éventuel repli).
    pub fn lire(&mut self, id: &str, tris: &[[glam::DVec3; 3]], categorie: &str) {
        self.lu.insert(id.to_string(), (geometrie::volume(tris), Maillage::depuis_triangles(tris), categorie.to_string()));
    }

    /// Ajoute un élément importé ; identifiant stable tiré de son origine (IfcGUID, sinon
    /// fichier et identifiant source).
    pub fn importer(&mut self, nom: String, categorie: String, niveau: Option<String>, tris: &[[glam::DVec3; 3]], raison: &str, origine: Option<Origine>) {
        if tris.is_empty() {
            *self.projet.ignores.entry(format!("{categorie} (sans géométrie)")).or_default() += 1;
            return;
        }
        let fichier = self.projet.sources.first().map(|s| s.fichier.as_str()).unwrap_or_default();
        let cle = origine.as_ref().map(|o| o.guid.clone().unwrap_or_else(|| format!("{fichier}:{}", o.id))).unwrap_or_else(|| format!("{fichier}:{nom}"));
        let id = identifiant("importe", &cle);
        self.projet.importes.push(ElementImporte { id, nom, categorie, niveau, maillage: Maillage::depuis_triangles(tris), raison: raison.into(), origine });
    }

    /// Compare chaque mur et chaque dalle à sa géométrie lue ; au-delà de `ECART_MAX`,
    /// l'élément redevient un élément importé (un mur avec ses menuiseries), et les types
    /// qui ne servent plus sont retirés.
    pub fn valider(&mut self) {
        let p = &mut self.projet;
        let mut ecartes: Vec<(String, String)> = Vec::new();
        for m in &p.murs {
            let (Some(v), Some((src, _, _))) = (p.volume_mur(m), self.lu.get(&m.id)) else { continue };
            let mut x = Mesure { categorie: "Murs", nom: m.nom.clone(), source: *src, parametres: v, retenu: true };
            x.retenu = x.ecart().abs() <= ECART_MAX;
            if !x.retenu {
                ecartes.push((m.id.clone(), format!("écart de volume {:+.1} % entre les paramètres et la géométrie lue", 100.0 * x.ecart())));
            }
            self.mesures.push(x);
        }
        for d in &p.dalles {
            let (Some(v), Some((src, _, _))) = (p.volume_dalle(d), self.lu.get(&d.id)) else { continue };
            let mut x = Mesure { categorie: "Dalles", nom: d.nom.clone(), source: *src, parametres: v, retenu: true };
            x.retenu = x.ecart().abs() <= ECART_MAX;
            if !x.retenu {
                ecartes.push((d.id.clone(), format!("écart de volume {:+.1} % entre les paramètres et la géométrie lue", 100.0 * x.ecart())));
            }
            self.mesures.push(x);
        }
        for (id, raison) in ecartes {
            let Some((_, maillage, categorie)) = self.lu.remove(&id) else { continue };
            let (nom, niveau, origine) = if let Some(i) = p.murs.iter().position(|m| m.id == id) {
                let m = p.murs.remove(i);
                // menuiseries du mur : importées avec lui
                let (avec, sans): (Vec<Ouverture>, Vec<Ouverture>) = std::mem::take(&mut p.ouvertures).into_iter().partition(|o| o.mur == id);
                p.ouvertures = sans;
                for o in avec {
                    if let Some(r) = o.representation {
                        let cat = match o.genre {
                            GenreOuverture::Porte => "Portes",
                            GenreOuverture::Fenetre => "Fenêtres",
                            GenreOuverture::Baie => "Baies",
                        };
                        p.importes.push(ElementImporte { id: o.id, nom: o.nom, categorie: cat.into(), niveau: Some(m.niveau.clone()), maillage: r, raison: "hébergée par un mur importé".into(), origine: o.origine });
                    }
                }
                (m.nom, Some(m.niveau), m.origine)
            } else if let Some(i) = p.dalles.iter().position(|d| d.id == id) {
                let d = p.dalles.remove(i);
                (d.nom, Some(d.niveau), d.origine)
            } else {
                continue;
            };
            p.importes.push(ElementImporte { id, nom, categorie, niveau, maillage, raison, origine });
        }
        let murs: std::collections::HashSet<&str> = p.murs.iter().map(|m| m.type_mur.as_str()).collect();
        p.types_murs.retain(|t| murs.contains(t.id.as_str()));
        let dalles: std::collections::HashSet<&str> = p.dalles.iter().map(|d| d.type_dalle.as_str()).collect();
        p.types_dalles.retain(|t| dalles.contains(t.id.as_str()));
        let ouv: std::collections::HashSet<&str> = p.ouvertures.iter().filter_map(|o| o.type_ouverture.as_deref()).collect();
        p.types_ouvertures.retain(|t| ouv.contains(t.id.as_str()));
        self.assembler();
        self.projet.arrondir();
    }

    /// Murs retenus à plusieurs couches dont le côté extérieur est vérifié : un mur par
    /// couche, en assemblage (même identifiant que le mur ; ses ouvertures traversent
    /// toutes les couches). Les autres gardent leur type composé.
    fn assembler(&mut self) {
        let p = &mut self.projet;
        let mut ids: Vec<(String, bool)> = self.sens_couches.iter().map(|(k, v)| (k.clone(), *v)).collect();
        ids.sort();
        for (id, inverser) in ids {
            let Some(m) = p.murs.iter_mut().find(|m| m.id == id) else { continue };
            let Axe::Droit { debut, fin } = m.axe else { continue };
            if inverser {
                // extérieur à gauche : axe retourné, abscisses des baies reprises depuis l'autre bout
                m.axe = Axe::Droit { debut: fin, fin: debut };
                let l = m.axe.longueur();
                for o in p.ouvertures.iter_mut().filter(|o| o.mur == id) {
                    o.abscisse = l - o.abscisse;
                }
            }
            let _ = p.eclater_mur(&id);
        }
        for m in &mut p.materiaux {
            if let Some(c) = self.couleurs.get(&m.nom) {
                m.couleur = *c;
            }
        }
    }
}

/// Identifiant stable d'un élément converti : même source, même élément → même
/// identifiant d'une conversion à l'autre (forme d'un UUID, deux hachages FNV-1a).
pub fn identifiant(espace: &str, cle: &str) -> String {
    let h = |graine: u64| {
        let mut x = graine;
        for b in espace.bytes().chain([0]).chain(cle.bytes()) {
            x ^= b as u64;
            x = x.wrapping_mul(0x100_0000_01b3);
        }
        x
    };
    let (a, b) = (h(0xcbf2_9ce4_8422_2325), h(0x6c62_272e_07bb_0142));
    format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", a >> 32, (a >> 16) & 0xffff, a & 0xffff, b >> 48, b & 0xffff_ffff_ffff)
}

/// Source décrite d'après le fichier.
pub fn source(id: &str, chemin: &std::path::Path, format: &str) -> Source {
    let meta = std::fs::metadata(chemin).ok();
    let modifie = meta.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
    Source {
        id: id.into(),
        fichier: chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        format: format.into(),
        octets: meta.map_or(0, |m| m.len()),
        modifie,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axes() {
        let d = Axe::Droit { debut: [0.0, 0.0], fin: [3000.0, 4000.0] };
        assert!((d.longueur() - 5000.0).abs() < 1e-9);
        assert!((d.abscisse([1500.0, 2000.0]) - 2500.0).abs() < 1e-9);
        // quart de cercle de rayon 1000, sens direct puis indirect
        let a = Axe::Arc { debut: [1000.0, 0.0], fin: [0.0, 1000.0], centre: [0.0, 0.0], direct: true };
        assert!((a.longueur() - std::f64::consts::FRAC_PI_2 * 1000.0).abs() < 1e-6);
        let p = [1000.0 * std::f64::consts::FRAC_1_SQRT_2; 2];
        assert!((a.abscisse(p) - a.longueur() / 2.0).abs() < 1e-6);
        let b = Axe::Arc { debut: [1000.0, 0.0], fin: [0.0, 1000.0], centre: [0.0, 0.0], direct: false };
        assert!((b.longueur() - 3.0 * std::f64::consts::FRAC_PI_2 * 1000.0).abs() < 1e-6);
    }

    #[test]
    fn identifiants_stables() {
        assert_eq!(identifiant("s1", "mur 12"), identifiant("s1", "mur 12"));
        assert_ne!(identifiant("s1", "mur 12"), identifiant("s1", "mur 13"));
        assert_eq!(identifiant("a", "b").len(), 36);
    }

    /// Mur de 5 m × 3 m × 200 mm percé d'une baie de 1 m × 1,2 m : 2,76 m³ ; aller-retour
    /// JSON.
    #[test]
    fn volume_et_json() {
        let mut p = Projet::nouveau("essai");
        p.niveaux.push(Niveau { id: "n".into(), nom: "RDC".into(), altitude: 0.0, origine: None });
        p.types_murs.push(TypeCompose { id: "t".into(), nom: "200".into(), couches: vec![Couche { materiau: None, epaisseur: 200.0, ..Default::default() }], couches_detaillees: false, origine: None });
        p.murs.push(Mur { variante: None, phase: None, sous_projet: None, id: "m".into(), nom: "Mur".into(), type_mur: "t".into(), assemblage: None, niveau: "n".into(), decalage_bas: 0.0, haut: Haut::Libre { hauteur: 3000.0 }, axe: Axe::Droit { debut: [0.0, 0.0], fin: [5000.0, 0.0] }, origine: None });
        p.ouvertures.push(Ouverture { id: "o".into(), nom: String::new(), genre: GenreOuverture::Baie, mur: "m".into(), type_ouverture: None, abscisse: 1500.0, largeur: 1000.0, hauteur: 1200.0, allege: 900.0, representation: None, origine: None });
        assert!((p.volume_mur(&p.murs[0]).unwrap() - 2.76e9).abs() < 1.0);
        let q = Projet::lire_json(&p.json().unwrap()).unwrap();
        assert_eq!(q.murs[0].axe, p.murs[0].axe);
        assert_eq!(q.murs[0].haut, Haut::Libre { hauteur: 3000.0 });
    }
}
