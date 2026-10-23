//! Paramètres Revit : définitions, valeurs et mise en forme (LISEZMOI-RVT.md du dossier Decodage RVT,
//! § 7 « Paramètres »).
//!
//! Un paramètre utilisateur est un élément dérivé de `ParamElem` (de famille, partagé,
//! de projet, global, de table de clés), nommé par `m_pParamDef` → `ParamDef.m_caption`,
//! dont la classe de définition donne le stockage. Un paramètre intégré a un id négatif
//! (BuiltInParameter de l'API) : son libellé, son stockage, son unité et son groupe ne
//! sont pas dans le fichier mais dans une table relevée dans Revit
//! (`donnees/parametres_integres.tsv`).
//!
//! Les valeurs sont dans les jeux typés `Element.m_pParamValueSet{Double,Int,AString,
//! ElementId}`, dans des listes `NamedParam` sans étiquette de type (occurrence
//! `FamilyInstance.m_pInstParams`, type `FamilySymbol.m_pParams`, type courant d'un
//! document de famille `Family.m_familyParams`), ou, pour beaucoup de paramètres
//! intégrés, dans un champ propre à la classe : correspondances apprises en comparant
//! les fichiers aux valeurs de l'API (`donnees/parametres_champs.tsv`).

use super::champs::{Obj, Record, Val};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Stockage d'un paramètre (`StorageType` de l'API).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stockage {
    Reel,
    Entier,
    Texte,
    Id,
    Aucun,
}

impl Stockage {
    fn de(s: &str) -> Stockage {
        match s {
            "Double" => Stockage::Reel,
            "Integer" => Stockage::Entier,
            "String" => Stockage::Texte,
            "ElementId" => Stockage::Id,
            _ => Stockage::Aucun,
        }
    }
}

/// Paramètre intégré (BuiltInParameter).
pub struct Integre {
    pub enumeration: &'static str,
    pub libelle: &'static str,
    pub stockage: Stockage,
    /// Unité (`autodesk.spec…`), vide si elle n'a pas été observée.
    pub spec: &'static str,
    /// Groupe (`autodesk.parameter.group…`), vide s'il n'a pas été observé.
    pub groupe: &'static str,
}

/// Table des paramètres intégrés, par id.
pub fn integres() -> &'static HashMap<i64, Integre> {
    static T: OnceLock<HashMap<i64, Integre>> = OnceLock::new();
    T.get_or_init(|| {
        include_str!("donnees/parametres_integres.tsv")
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let c: Vec<&'static str> = l.split('\t').collect();
                let id = c.first()?.parse().ok()?;
                let champ = |i: usize| c.get(i).copied().unwrap_or("");
                Some((id, Integre { enumeration: champ(1), libelle: champ(2), stockage: Stockage::de(champ(3)), spec: champ(4), groupe: champ(5) }))
            })
            .collect()
    })
}

/// Identifiant Forge sans sa version (« autodesk.spec.aec:length-2.0.0 » → « …:length »).
fn sans_version(s: &str) -> &str {
    match s.rfind('-') {
        Some(i) if s[i + 1..].chars().all(|c| c.is_ascii_digit() || c == '.') => &s[..i],
        _ => s,
    }
}

/// Libellé français d'un groupe de paramètres ; « Autre » pour un groupe vide ou inconnu.
pub fn libelle_groupe(groupe: &str) -> &'static str {
    static T: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    let t = T.get_or_init(|| include_str!("donnees/groupes_parametres.tsv").lines().filter(|l| !l.starts_with('#')).filter_map(|l| l.split_once('\t')).collect());
    t.get(sans_version(groupe)).copied().unwrap_or("Autre")
}

/// Rang d'un groupe dans la palette des propriétés de Revit (ordre de l'interface pour
/// les groupes courants, les autres ensuite).
pub fn rang_groupe(libelle: &str) -> usize {
    const ORDRE: &[&str] = &[
        "Contraintes",
        "Construction",
        "Graphismes",
        "Texte",
        "Matériaux et finitions",
        "Cotes",
        "Structure",
        "Données d'identification",
        "Phase de construction",
        "Données",
        "Général",
        "Visibilité",
        "Paramètres IFC",
        "Propriétés analytiques",
        "Analyse d'énergie",
        "Autre",
    ];
    ORDRE.iter().position(|&g| g == libelle).unwrap_or(ORDRE.len())
}

/// Pas d'un chemin de champ : nom, indices de liste, puis corps du pointeur atteint.
struct Pas {
    champ: &'static str,
    indices: Vec<usize>,
    pointeur: bool,
}

/// Chemin « m_refs[1]>.m_offset » (« . » champ, « > »
/// corps d'un pointeur, [i] élément de liste).
fn chemin(s: &'static str) -> Option<Vec<Pas>> {
    s.split('.')
        .map(|t| {
            let (t, pointeur) = match t.strip_suffix('>') {
                Some(t) => (t, true),
                None => (t, false),
            };
            let (champ, reste) = t.split_once('[').map(|(c, r)| (c, Some(r))).unwrap_or((t, None));
            let indices = match reste {
                Some(r) => r.trim_end_matches(']').split("][").map(|i| i.parse().ok()).collect::<Option<Vec<usize>>>()?,
                None => Vec::new(),
            };
            Some(Pas { champ, indices, pointeur })
        })
        .collect()
}

/// Paramètres intégrés portés par un champ, par classe : (id du paramètre, chemin).
fn champs_appris() -> &'static HashMap<&'static str, Vec<(i64, Vec<Pas>)>> {
    static T: OnceLock<HashMap<&'static str, Vec<(i64, Vec<Pas>)>>> = OnceLock::new();
    T.get_or_init(|| {
        let mut t: HashMap<&'static str, Vec<(i64, Vec<Pas>)>> = HashMap::new();
        for l in include_str!("donnees/parametres_champs.tsv").lines().filter(|l| !l.starts_with('#')) {
            let c: Vec<&'static str> = l.split('\t').collect();
            if let (Some(&classe), Some(Ok(id)), Some(ch)) = (c.first(), c.get(1).map(|s| s.parse()), c.get(2).and_then(|s| chemin(s))) {
                t.entry(classe).or_default().push((id, ch));
            }
        }
        t
    })
}

/// Valeur au bout d'un chemin.
fn suivre<'r, 's>(r: &'r Record<'s>, o: &'r Obj<'s>, pas: &[Pas]) -> Option<&'r Val<'s>> {
    let mut o = o;
    let mut v = None;
    for (k, p) in pas.iter().enumerate() {
        let mut x = o.get(p.champ)?;
        for &i in &p.indices {
            x = x.list().get(i)?;
        }
        if k + 1 == pas.len() {
            v = Some(x);
            break;
        }
        o = if p.pointeur { &r.deref(x)?.obj } else { x.obj()? };
    }
    v
}

/// Valeur brute d'un paramètre.
#[derive(Clone, Debug, PartialEq)]
pub enum Brut {
    Reel(f64),
    Entier(i64),
    Texte(String),
    Id(i64),
    /// `NamedParam` : toutes les cases, à lire selon le stockage du paramètre.
    Nomme { reel: f64, entier: i64, texte: String, id: i64 },
}

impl Brut {
    fn de(v: &Val, st: Stockage) -> Option<Brut> {
        Some(match (v, st) {
            (Val::Obj(_), _) => Brut::Id(v.element_id()?),
            (Val::Int(i), Stockage::Reel) => Brut::Reel(*i as f64),
            (Val::Int(i), _) => Brut::Entier(*i),
            (Val::Real(x), _) => Brut::Reel(*x),
            (Val::Str(s), _) => Brut::Texte(s.clone()),
            _ => return None,
        })
    }

    /// Valeur selon le stockage (une formule a sa valeur calculée dans `m_value`…).
    pub fn selon(&self, st: Stockage) -> Brut {
        match (self, st) {
            (Brut::Nomme { reel, .. }, Stockage::Reel) => Brut::Reel(*reel),
            (Brut::Nomme { entier, .. }, Stockage::Entier) => Brut::Entier(*entier),
            (Brut::Nomme { texte, .. }, Stockage::Texte) => Brut::Texte(texte.clone()),
            (Brut::Nomme { id, .. }, Stockage::Id) => Brut::Id(*id),
            // stockage inconnu : la case remplie
            (Brut::Nomme { reel, entier, texte, id }, Stockage::Aucun) => {
                if !texte.is_empty() {
                    Brut::Texte(texte.clone())
                } else if *entier != 0 {
                    Brut::Entier(*entier)
                } else if *reel != 0.0 {
                    Brut::Reel(*reel)
                } else {
                    Brut::Id(*id)
                }
            }
            // un entier rangé dans le jeu des réels (styles de texte…), et l'inverse
            (Brut::Reel(x), Stockage::Entier) => Brut::Entier(x.round() as i64),
            (Brut::Entier(i), Stockage::Reel) => Brut::Reel(*i as f64),
            (b, _) => b.clone(),
        }
    }
}

/// Définition d'un paramètre utilisateur.
#[derive(Clone, Debug)]
pub struct Definition {
    pub nom: String,
    /// « famille », « partagé », « projet », « global », « table de clés »…
    pub genre: String,
    pub stockage: Stockage,
    /// Unité (`autodesk.spec…`) ; « autodesk.spec:spec.bool » pour un Oui/Non.
    pub spec: String,
    pub groupe: String,
    pub visible: bool,
    /// Paramètre de famille : d'occurrence (true) ou de type.
    pub occurrence: Option<bool>,
    /// Paramètre partagé : GUID.
    pub guid: Option<String>,
}

/// Classe de définition → stockage (les classes dérivées en héritent).
const STOCKAGE_DEF: &[(&str, Stockage)] = &[
    ("ParamDefValue", Stockage::Reel),
    ("ParamDefLength", Stockage::Reel),
    ("ParamDefAngle", Stockage::Reel),
    ("ParamDefArea", Stockage::Reel),
    ("ParamDefVolume", Stockage::Reel),
    ("ParamDefDouble", Stockage::Reel),
    ("ParamDefYesNo", Stockage::Entier),
    ("ParamDefInt", Stockage::Entier),
    ("ParamDefMaterialBrowse", Stockage::Id),
    ("ParamDefMaterial", Stockage::Id),
    ("ParamDefFamType", Stockage::Id),
    ("ParamDefElementId", Stockage::Id),
    ("ParamDefRefTableEntry", Stockage::Id),
    ("ParamDefString", Stockage::Texte),
];


/// Octets d'un GUID Windows (trois premiers champs en petit-boutiste) → ordre canonique.
pub fn le_vers_be(b: &[u8; 16]) -> [u8; 16] {
    let mut o = *b;
    o[..4].reverse();
    o[4..6].reverse();
    o[6..8].reverse();
    o
}

pub fn uuid_texte(b: &[u8; 16]) -> String {
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

/// Définition d'un élément dérivé de `ParamElem` ; `herite(classe, base)` dit si une
/// classe du schéma dérive d'une autre.
pub fn definition(r: &Record, herite: &dyn Fn(&str, &str) -> bool) -> Option<Definition> {
    let o = &r.root;
    let d = r.field_body(o, "m_pParamDef")?;
    let classe = d.class_name();
    let stockage = STOCKAGE_DEF.iter().find(|(b, _)| herite(classe, b)).map(|&(_, s)| s).unwrap_or(Stockage::Texte);
    let forge = |o: &Obj, k: &str| o.get(k).and_then(Val::obj).and_then(|x| x.get("m_typeId")).and_then(Val::str).unwrap_or_default().to_string();
    let spec = if herite(classe, "ParamDefYesNo") { "autodesk.spec:spec.bool-1.0.0".to_string() } else { forge(d, "m_specTypeId") };
    let genre = match o.class_name() {
        "ParamElemFamily" => "famille",
        "ParamElemExternal" => "partagé",
        "ParamElemProject" => "projet",
        "ParamElemGlobal" => "global",
        "ParamElemRefTableEntry" => "table de clés",
        c => c,
    };
    Some(Definition {
        nom: d.get("m_caption")?.str()?.to_string(),
        genre: genre.to_string(),
        stockage,
        spec,
        groupe: forge(d, "m_groupTypeId"),
        visible: d.get("m_userVisible").and_then(Val::int) != Some(0),
        occurrence: o.get("m_instanceParam").and_then(Val::int).map(|v| v != 0),
        guid: match o.get("m_externalParamKey").and_then(Val::obj).and_then(|k| k.get("m_guidValue")).and_then(Val::obj).and_then(|g| g.get("m_guid")) {
            Some(Val::Guid(b)) => Some(uuid_texte(&le_vers_be(b))),
            _ => None,
        },
    })
}

/// Valeurs de paramètres d'un objet : champs appris, jeux typés, listes `NamedParam`.
/// Un paramètre n'est pris qu'une fois (premier rencontré).
pub fn valeurs(r: &Record) -> Vec<(i64, Brut)> {
    let o = &r.root;
    let mut out: Vec<(i64, Brut)> = Vec::new();
    let mut vus = std::collections::HashSet::new();
    let mut ajouter = |out: &mut Vec<(i64, Brut)>, id: i64, b: Brut| {
        if vus.insert(id) {
            out.push((id, b));
        }
    };
    // niveau de l'élément (Element.m_assocLevelId) : paramètres « Niveau » des
    // nomenclatures (SCHEDULE_LEVEL_PARAM, ROOM_LEVEL_ID)
    if let Some(n) = o.get("m_assocLevelId").and_then(Val::element_id).filter(|&n| n != -1) {
        for id in [-1_002_062, -1_006_916] {
            ajouter(&mut out, id, Brut::Id(n));
        }
    }
    // niveau : nom (DATUM_TEXT) et élévation (LEVEL_ELEV, Z de son plan)
    if o.class_name() == "Level" {
        if let Some(nom) = o.get("m_text").and_then(Val::str) {
            ajouter(&mut out, -1_008_000, Brut::Texte(nom.to_string()));
        }
        if let Some(z) = r.field_body(o, "m_pSurface").and_then(|s| s.get("m_origin")).and_then(Val::reals).and_then(|v| v.get(2).copied()) {
            ajouter(&mut out, -1_007_102, Brut::Reel(z));
        }
    }
    // phases de l'élément (champs de Element) : PHASE_CREATED, PHASE_DEMOLISHED
    for (k, id) in [("m_createdPhaseId", -1_012_100), ("m_demolishedPhaseId", -1_012_101)] {
        if let Some(p) = o.get(k).and_then(Val::element_id) {
            ajouter(&mut out, id, Brut::Id(p));
        }
    }
    // champ appris d'abord : Revit lit le champ quand un jeu garde aussi une valeur
    // (ancienne) pour le même paramètre
    if let Some(liste) = champs_appris().get(o.class_name()) {
        let t = integres();
        for (id, pas) in liste {
            let st = t.get(id).map(|i| i.stockage).unwrap_or(Stockage::Aucun);
            if let Some(b) = suivre(r, o, pas).and_then(|v| Brut::de(v, st)) {
                ajouter(&mut out, *id, b);
            }
        }
    }
    for (k, st) in [("m_pParamValueSetDouble", Stockage::Reel), ("m_pParamValueSetInt", Stockage::Entier), ("m_pParamValueSetAString", Stockage::Texte), ("m_pParamValueSetElementId", Stockage::Id)] {
        let Some(ps) = r.field_body(o, k) else { continue };
        for x in ps.get("m_paramSet").map(Val::list).unwrap_or_default() {
            if let Some((id, b)) = x.obj().and_then(|x| Some((x.get("m_paramId").and_then(Val::element_id)?, Brut::de(x.get("m_value")?, st)?))) {
                ajouter(&mut out, id, b);
            }
        }
    }
    for k in ["m_pInstParams", "m_pParams", "m_familyParams"] {
        let Some(ps) = r.field_body(o, k) else { continue };
        for x in ps.get("m_params").map(Val::list).unwrap_or_default() {
            let Some(x) = x.obj() else { continue };
            let Some(id) = x.get("m_paramId").and_then(Val::element_id) else { continue };
            let b = Brut::Nomme {
                reel: x.get("m_value").and_then(Val::real).unwrap_or(0.0),
                entier: x.get("m_int").and_then(Val::int).unwrap_or(0),
                texte: x.get("m_str").and_then(Val::str).unwrap_or_default().to_string(),
                id: x.get("m_elemId").and_then(Val::element_id).unwrap_or(-1),
            };
            ajouter(&mut out, id, b);
        }
    }
    out
}

/// Paramètre décrit pour l'affichage : libellé, groupe, texte de la valeur.
pub struct Affiche {
    pub nom: String,
    pub groupe: &'static str,
    pub valeur: String,
}

/// Genre d'unité d'un réel : depuis le spec, sinon d'après le nom d'énumération.
fn unite(spec: &str, enumeration: &str) -> &'static str {
    let s = sans_version(spec);
    let s = s.rsplit(':').next().unwrap_or("");
    match s {
        "length" | "sheetLength" | "decimalSheetLength" | "distance" | "barDiameter" | "crackWidth" | "displacement" | "reinforcementCover" | "reinforcementLength"
        | "reinforcementSpacing" | "sectionDimension" | "pipeDimension" | "pipeSize" | "ductSize" | "conduitSize" | "cableTraySize" | "wireDiameter" => "longueur",
        "area" | "sectionArea" | "reinforcementArea" => "surface",
        "volume" | "reinforcementVolume" => "volume",
        "angle" | "rotationAngle" | "siteAngle" | "rotation" => "angle",
        "" => {
            let e = enumeration;
            if e.contains("AREA") {
                "surface"
            } else if e.contains("VOLUME") {
                "volume"
            } else if e.contains("ANGLE") || e.contains("ROTATION") {
                "angle"
            } else if ["HEIGHT", "WIDTH", "LENGTH", "OFFSET", "ELEVATION", "THICKNESS", "DEPTH", "RADIUS", "DIST"].iter().any(|k| e.contains(k)) {
                "longueur"
            } else {
                "nombre"
            }
        }
        _ => "nombre",
    }
}

/// Nombre sans zéros inutiles, arrondi à `decimales`.
fn nombre(v: f64, decimales: i32) -> String {
    let p = 10f64.powi(decimales);
    crate::step::format_real((v * p).round() / p)
}

/// Format d'affichage d'une unité (`FormatOptions` de l'API).
#[derive(Clone, Debug)]
pub struct Format {
    /// Unité (`autodesk.unit.unit:meters`…, sans version).
    pub unite: String,
    /// Précision (0,01…).
    pub precision: f64,
    /// Symbole (`autodesk.unit.symbol:mSup2`…, vide : aucun).
    pub symbole: String,
    pub sans_zeros: bool,
}

/// Unités du projet (`UnitsElem.m_units` : `m_formatOptionsMap` spec → format,
/// `m_decimalSymbol`). Vide : mise en forme du viewer (mm, m², m³, degrés).
#[derive(Clone, Debug, Default)]
pub struct Unites {
    pub formats: HashMap<String, Format>,
    pub virgule: bool,
}

impl Unites {
    /// Lit `UnitsElem`.
    pub fn lire(r: &Record) -> Option<Unites> {
        let u = r.root.get("m_units")?.obj()?;
        let mut formats = HashMap::new();
        for p in u.get("m_formatOptionsMap").map(Val::list).unwrap_or_default() {
            let Some(p) = p.obj() else { continue };
            let forge = |o: Option<&Obj>, k: &str| o.and_then(|o| o.get(k)).and_then(Val::obj).and_then(|x| x.get("m_typeId")).and_then(Val::str).map(|s| sans_version(s).to_string()).unwrap_or_default();
            let spec = p.get("first").and_then(Val::obj).and_then(|x| x.get("m_typeId")).and_then(Val::str).map(|s| sans_version(s).to_string()).unwrap_or_default();
            let f = p.get("second").and_then(|v| r.deref(v).map(|b| &b.obj).or(v.obj()));
            let Some(f) = f else { continue };
            formats.insert(
                spec,
                Format {
                    unite: forge(Some(f), "m_unitTypeId"),
                    precision: f.get("m_accuracy").and_then(Val::real).unwrap_or(0.01),
                    symbole: forge(Some(f), "m_symbolTypeId"),
                    sans_zeros: f.get("m_bSuppressTrailingZeros").and_then(Val::int) == Some(1),
                },
            );
        }
        Some(Unites { formats, virgule: u.get("m_decimalSymbol").and_then(Val::int) == Some(1) })
    }

    /// Valeur interne (pieds, pieds², radians…) mise en forme selon le format du spec ;
    /// None si le projet n'en a pas, ou l'unité n'est pas connue.
    pub fn formater(&self, v: f64, spec: &str) -> Option<String> {
        let f = self.formats.get(sans_version(spec))?;
        formater(v, f, self.virgule)
    }
}

/// Mise en forme d'une valeur interne selon un format : conversion d'unité, arrondi à la
/// précision, symbole.
pub fn formater(v: f64, f: &Format, virgule: bool) -> Option<String> {
    const FT: f64 = 0.3048;
    let u = f.unite.rsplit(':').next().unwrap_or("");
    let x = match u {
        "meters" | "metersCentimeters" => v * FT,
        "centimeters" => v * FT * 100.0,
        "millimeters" => v * FT * 1000.0,
        "decimeters" => v * FT * 10.0,
        "feet" => v,
        "inches" => v * 12.0,
        "squareMeters" => v * FT * FT,
        "squareCentimeters" => v * FT * FT * 1e4,
        "squareMillimeters" => v * FT * FT * 1e6,
        "squareFeet" => v,
        "cubicMeters" => v * FT * FT * FT,
        "liters" => v * FT * FT * FT * 1000.0,
        "cubicFeet" => v,
        "degrees" | "slopeDegrees" => v.to_degrees(),
        "radians" => v,
        "general" | "fixed" | "currency" | "percentage" => v,
        _ => return None,
    };
    let p = if f.precision > 0.0 { f.precision } else { 1.0 };
    let decimales = (-p.log10()).round().max(0.0) as usize;
    let arrondi = (x / p).round() * p;
    let mut t = format!("{:.*}", decimales, arrondi);
    if t == "-0" || t.starts_with("-0.") && t.trim_start_matches(['-', '0', '.']).is_empty() {
        t = t.trim_start_matches('-').to_string();
    }
    if f.sans_zeros && t.contains('.') {
        t = t.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if virgule {
        t = t.replace('.', ",");
    }
    let symbole = match f.symbole.rsplit(':').next().unwrap_or("") {
        "" => "",
        "mSup2" => " m²",
        "mSup3" => " m³",
        "m" => " m",
        "cm" => " cm",
        "mm" => " mm",
        "degree" | "slopeDegree" => "°",
        "percent" => "%",
        "liter" => " L",
        "ftSup2" => " ft²",
        s => return Some(format!("{t} {s}")),
    };
    Some(format!("{t}{symbole}"))
}

/// Texte d'une valeur ; None pour une valeur vide (chaîne vide, ElementId −1) ;
/// `nom_element` nomme l'élément désigné par un ElementId.
/// Libellé affiché par Revit pour la valeur entière d'un paramètre intégré à valeurs
/// énumérées (relevé dans l'interface ; seules les valeurs observées sont connues).
pub fn libelle_valeur(param: i64, v: i64) -> Option<&'static str> {
    static T: OnceLock<HashMap<(i64, i64), &'static str>> = OnceLock::new();
    let t = T.get_or_init(|| {
        include_str!("donnees/valeurs_enumerees.tsv")
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let mut c = l.splitn(3, '\t');
                Some(((c.next()?.parse().ok()?, c.next()?.parse().ok()?), c.next()?))
            })
            .collect()
    });
    t.get(&(param, v)).copied()
}

pub fn texte(b: &Brut, param: i64, st: Stockage, spec: &str, enumeration: &str, unites: &Unites, nom_element: &dyn Fn(i64) -> Option<String>) -> Option<String> {
    Some(match b.selon(st) {
        // unités du projet quand elles sont connues pour ce spec
        Brut::Reel(v) if unites.formater(v, spec).is_some() => unites.formater(v, spec).unwrap_or_default(),
        Brut::Reel(v) => match unite(spec, enumeration) {
            "longueur" => format!("{} mm", nombre(v * super::geometrie::FT, 1)),
            "surface" => format!("{} m²", nombre(v * 0.092_903_04, 3)),
            "volume" => format!("{} m³", nombre(v * 0.028_316_846_592, 3)),
            "angle" => format!("{}°", nombre(v.to_degrees(), 2)),
            _ => nombre(v, 6),
        },
        Brut::Entier(i) if sans_version(spec).ends_with("spec.bool") => (if i != 0 { "Oui" } else { "Non" }).to_string(),
        Brut::Entier(i) => libelle_valeur(param, i).map(str::to_string).unwrap_or_else(|| i.to_string()),
        Brut::Texte(s) if s.is_empty() => return None,
        Brut::Texte(s) => s,
        Brut::Id(-1) => return None,
        Brut::Id(id) => nom_element(id).unwrap_or_else(|| format!("#{id}")),
        Brut::Nomme { .. } => return None,
    })
}

/// Paramètres affichables d'un élément, rangés par groupe dans l'ordre de Revit.
pub fn affiches(vals: &[(i64, Brut)], defs: &HashMap<i64, Definition>, unites: &Unites, nom_element: &dyn Fn(i64, i64) -> Option<String>) -> Vec<Affiche> {
    let t = integres();
    let mut out = Vec::new();
    for (id, b) in vals {
        let (nom, st, spec, enumeration, groupe) = match (defs.get(id), t.get(id)) {
            (Some(d), _) if d.visible => (d.nom.clone(), d.stockage, d.spec.as_str(), "", d.groupe.as_str()),
            (None, Some(i)) if *id < 0 && !i.libelle.is_empty() && i.libelle != "?" => (i.libelle.to_string(), i.stockage, i.spec, i.enumeration, i.groupe),
            _ => continue,
        };
        if let Some(valeur) = texte(b, *id, st, spec, enumeration, unites, &|e| nom_element(*id, e)) {
            out.push(Affiche { nom, groupe: libelle_groupe(groupe), valeur });
        }
    }
    out.sort_by(|a, b| (rang_groupe(a.groupe), a.groupe, &a.nom).cmp(&(rang_groupe(b.groupe), b.groupe, &b.nom)));
    out
}

/// IfcGUID d'un élément (`ExportUtils.GetExportId` compressé) : GUID de l'épisode de
/// création (octets Windows de `Global/History`) dont les 4 derniers octets sont
/// combinés par OU exclusif avec l'ElementId, écrit en 22 caractères base 64 (alphabet
/// IFC : 2 bits puis 21 groupes de 6 bits).
pub fn ifc_guid(episode: &[u8; 16], id: i64) -> String {
    const ALPHABET: &[u8; 64] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";
    let mut g = le_vers_be(episode);
    let fin = u32::from_be_bytes(g[12..16].try_into().unwrap()) ^ (id as u32);
    g[12..16].copy_from_slice(&fin.to_be_bytes());
    let n = u128::from_be_bytes(g);
    (0..22).map(|i| ALPHABET[((n >> (6 * (21 - i))) & 63) as usize] as char).collect()
}

/// UniqueId de Revit : GUID de l'épisode, puis l'ElementId sur 8 chiffres hexadécimaux.
pub fn unique_id(episode: &[u8; 16], id: i64) -> String {
    format!("{}-{:08x}", uuid_texte(&le_vers_be(episode)), id as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiants_fen_2pf() {
        // épisode tel qu'enregistré dans Global/History de FEN_2PF.rfa ; UniqueId et
        // IfcGUID relevés dans Revit 2026 (script RiveTT rvt-reference-groupes-uniqueid)
        let ep = [0xd9, 0x52, 0xdc, 0x79, 0x75, 0xc4, 0x41, 0x48, 0xa4, 0x7e, 0x5c, 0x94, 0xd4, 0xe2, 0x38, 0xe2];
        assert_eq!(unique_id(&ep, 9_899_388), "79dc52d9-c475-4841-a47e-5c94d4e238e2-00970d7c");
        assert_eq!(ifc_guid(&ep, 9_899_388), "1vt5BPn7L8GQH_N9JKTJMU");
        assert_eq!(ifc_guid(&ep, 10), "1vt5BPn7L8GQH_N9JKuZZe");
    }

    #[test]
    fn tables() {
        let t = integres();
        assert!(t.len() > 3600);
        let l = &t[&-1_001_301];
        assert_eq!((l.libelle, l.stockage), ("Largeur", Stockage::Reel));
        assert_eq!(libelle_groupe("autodesk.parameter.group:dimensions-1.0.0"), "Cotes");
        assert_eq!(libelle_groupe(""), "Autre");
        let c = champs_appris();
        assert!(c["FamilyInstance"].iter().any(|(id, p)| *id == -1_002_066 && p.len() == 2 && p[0].champ == "m_refs" && p[0].indices == [1] && p[0].pointeur));
    }

    #[test]
    fn mise_en_forme() {
        let aucun = |_: i64| None;
        assert_eq!(texte(&Brut::Reel(3.937_007_874_015_747_3), -1_001_301, Stockage::Reel, "autodesk.spec.aec:length-2.0.1", "", &Unites::default(), &aucun).as_deref(), Some("1200 mm"));
        assert_eq!(texte(&Brut::Entier(1), 9_900_253, Stockage::Entier, "autodesk.spec:spec.bool-1.0.0", "", &Unites::default(), &aucun).as_deref(), Some("Oui"));
        let n = Brut::Nomme { reel: 0.360_892_388_451_443_56, entier: 0, texte: String::new(), id: -1 };
        assert_eq!(texte(&n, 9_900_475, Stockage::Reel, "autodesk.spec.aec:length-2.0.1", "", &Unites::default(), &aucun).as_deref(), Some("110 mm"));
        assert_eq!(texte(&Brut::Id(-1), -1_002_050, Stockage::Id, "", "", &Unites::default(), &aucun), None);
        // énumération : « Définir les propriétés thermiques par » = Type schématique
        assert_eq!(texte(&Brut::Entier(1), -1_005_439, Stockage::Entier, "", "", &Unites::default(), &aucun).as_deref(), Some("Type schématique"));
    }
}
