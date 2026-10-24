//! Nomenclatures Revit : définition et contenu (LISEZMOI-RVT.md du dossier Decodage
//! RVT, § 7 « Nomenclatures »).
//!
//! La définition est `DBViewSchedule.m_pSchema` → `ScheduleSchema` : catégorie
//! `m_categoryId`, détail de chaque occurrence `m_bInstanceSchedule`, total général
//! `m_showGrandTotal`, champs `m_pFields` (`ScheduleField` : titre `m_columnHeading`,
//! paramètre `m_paramElemId`, `m_hidden`, total `m_displayType` 1, genre `m_sourceType`)
//! dans l'ordre de `m_fieldIds`, filtres `m_filterArr` (`ScheduleFilter` : champ,
//! `m_filterType` = `ScheduleFilterType` de l'API + 2, valeur `ParamStorage`), tris et
//! regroupements `m_sortFieldArr` (`ScheduleSortField` : champ, `m_sortOrder` 0
//! croissant, `m_groupingFlags` : 2 en-tête, 4 pied, 16 ligne vide).
//!
//! Revit ne garde pas le contenu : il le recalcule à l'affichage. Le contenu est donc
//! recomposé ici depuis les éléments lus et leurs paramètres, formules des champs
//! calculés comprises (`formules`), pourcentages, pièces de départ et d'arrivée des
//! portes et fenêtres (`Maquette::pieces_de_porte`).

use super::champs::{Obj, Record, Val};
use super::formules::{self, Expr, Valeur};
use super::maquette::Maquette;
use super::parametres::{self, Brut, Format, Stockage};
use std::cmp::Ordering;
use std::collections::HashMap;

/// Genre d'un champ (`ScheduleFieldType` de l'API).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenreChamp {
    Occurrence,
    Type,
    Nombre,
    Vue,
    Formule,
    Pourcentage,
    PieceDepart,
    PieceArrivee,
    InfoProjet,
    Combine,
    Autre(i64),
}

impl GenreChamp {
    fn de(v: i64) -> GenreChamp {
        match v {
            0 => GenreChamp::Occurrence,
            1 => GenreChamp::Type,
            2 => GenreChamp::Nombre,
            4 => GenreChamp::Vue,
            5 => GenreChamp::Formule,
            6 => GenreChamp::Pourcentage,
            8 => GenreChamp::PieceDepart,
            9 => GenreChamp::PieceArrivee,
            10 => GenreChamp::InfoProjet,
            20 => GenreChamp::Combine,
            v => GenreChamp::Autre(v),
        }
    }

    /// Champs dont la valeur est recomposée ici.
    pub fn calcule(self) -> bool {
        matches!(self, GenreChamp::Occurrence | GenreChamp::Type | GenreChamp::Nombre | GenreChamp::Formule | GenreChamp::Pourcentage | GenreChamp::PieceDepart | GenreChamp::PieceArrivee | GenreChamp::Vue | GenreChamp::InfoProjet)
    }
}

#[derive(Clone, Debug)]
pub struct Champ {
    pub id: i64,
    pub titre: String,
    pub param: i64,
    pub masque: bool,
    /// Totalisé dans les regroupements (`m_displayType` 1).
    pub total: bool,
    pub genre: GenreChamp,
    /// Formule d'un champ calculé (`m_pExpression`).
    pub formule: Option<Expr>,
    /// Unité d'un champ calculé (`m_pParamDef` → `m_specTypeId`).
    pub spec: String,
    /// Format propre au champ (`m_pFormatOptions` quand `m_bUseDefault` vaut 0), sinon
    /// celui du projet.
    pub format: Option<Format>,
    /// Champ pourcentage : champ dont il est la part (`m_percentageOfId`) et champ de
    /// regroupement du total (`m_percentageById`, −1 : total général).
    pub pourcentage: Option<(i64, i64)>,
}

#[derive(Clone, Debug)]
pub struct Filtre {
    pub champ: i64,
    /// `ScheduleFilterType` de l'API (0 égal, 1 différent, 2 supérieur, 3 supérieur ou
    /// égal, 4 inférieur, 5 inférieur ou égal, 6 contient, 7 ne contient pas, 8 commence
    /// par, 9 ne commence pas par, 10 finit par, 11 ne finit pas par, 12 a un paramètre,
    /// 13 a une valeur, 14 n'a pas de valeur).
    pub genre: i64,
    pub valeur: Brut,
}

#[derive(Clone, Debug)]
pub struct Tri {
    pub champ: i64,
    pub decroissant: bool,
    pub entete: bool,
    pub pied: bool,
    pub ligne_vide: bool,
    /// Pied de groupe : titre (valeur du groupe, drapeau 32) et nombre d'éléments
    /// (drapeau 64), écrits « RDC: 3 » dans la première colonne.
    pub titre_pied: bool,
    pub nombre_pied: bool,
}

#[derive(Clone, Debug)]
pub struct Definition {
    pub categorie: i64,
    pub detail: bool,
    /// Nomenclature de clés (`m_bRefTable`).
    pub cle: bool,
    /// Schéma de surface d'une nomenclature de zones (`m_areaSchemeId`, −1 : aucun).
    pub schema: i64,
    pub total_general: bool,
    /// Titre du total général (`m_grandTotalTitle`) s'il est affiché, avec ou sans le
    /// nombre d'éléments (`m_showGrandTotalTitle`, `m_showGrandTotalCount`).
    pub titre_total: Option<String>,
    pub nombre_total: bool,
    pub champs: Vec<Champ>,
    pub filtres: Vec<Filtre>,
    pub tris: Vec<Tri>,
}

fn id_champ(v: Option<&Val>) -> i64 {
    v.and_then(Val::obj).and_then(|o| o.get("m_id")).and_then(Val::int).unwrap_or(-1)
}

fn valeur_filtre(o: Option<&Obj>) -> Brut {
    o.map(parametres::valeur_stockee).unwrap_or(Brut::Entier(0))
}

/// Objet désigné par un pointeur possédé, ou incorporé.
fn corps<'r, 's>(r: &'r Record<'s>, v: &'r Val<'s>) -> Option<&'r Obj<'s>> {
    r.deref(v).map(|b| &b.obj).or(v.obj())
}

/// Définition d'une nomenclature (`DBViewSchedule`).
pub fn definition(r: &Record) -> Option<Definition> {
    let s = r.field_body(&r.root, "m_pSchema")?;
    let int = |o: &Obj, k: &str| o.get(k).and_then(Val::int).unwrap_or(0);
    let ids: Vec<i64> = s.get("m_fieldIds").map(Val::list).unwrap_or_default().iter().map(|v| id_champ(Some(v))).collect();
    let mut champs = Vec::new();
    for (k, f) in s.get("m_pFields").map(Val::list).unwrap_or_default().iter().enumerate() {
        let Some(f) = r.deref(f).map(|b| &b.obj) else { continue };
        let pd = f.get("m_pParamDef").and_then(|v| corps(r, v));
        let spec = match pd {
            Some(d) if d.class_name() == "ParamDefYesNo" => "autodesk.spec:spec.bool-1.0.0".to_string(),
            Some(d) => d.get("m_specTypeId").and_then(Val::obj).and_then(|x| x.get("m_typeId")).and_then(Val::str).unwrap_or_default().to_string(),
            None => String::new(),
        };
        let format = f.get("m_pFormatOptions").and_then(|v| corps(r, v)).filter(|o| int(o, "m_bUseDefault") == 0).map(Format::de);
        champs.push(Champ {
            id: ids.get(k).copied().unwrap_or(k as i64),
            titre: f.get("m_columnHeading").and_then(Val::str).unwrap_or_default().to_string(),
            param: f.get("m_paramElemId").and_then(Val::element_id).unwrap_or(-1),
            masque: int(f, "m_hidden") != 0,
            total: int(f, "m_displayType") == 1,
            genre: GenreChamp::de(int(f, "m_sourceType")),
            formule: f.get("m_pExpression").and_then(|v| corps(r, v)).and_then(|x| formules::lire(r, x)),
            spec,
            format,
            pourcentage: (int(f, "m_sourceType") == 6).then(|| (id_champ(f.get("m_percentageOfId")), id_champ(f.get("m_percentageById")))),
        });
    }
    let filtres = s
        .get("m_filterArr")
        .map(Val::list)
        .unwrap_or_default()
        .iter()
        .filter_map(|v| corps(r, v))
        .map(|f| Filtre { champ: id_champ(f.get("m_fieldId")), genre: int(f, "m_filterType") - 2, valeur: valeur_filtre(f.get("m_value").and_then(Val::obj)) })
        .collect();
    let tris = s
        .get("m_sortFieldArr")
        .map(Val::list)
        .unwrap_or_default()
        .iter()
        .filter_map(|v| corps(r, v))
        .map(|t| {
            let d = int(t, "m_groupingFlags");
            Tri { champ: id_champ(t.get("m_fieldId")), decroissant: int(t, "m_sortOrder") != 0, entete: d & 2 != 0, pied: d & 4 != 0, ligne_vide: d & 16 != 0, titre_pied: d & 32 != 0, nombre_pied: d & 64 != 0 }
        })
        .collect();
    Some(Definition {
        categorie: s.get("m_categoryId").and_then(Val::element_id).unwrap_or(-1),
        detail: int(s, "m_bInstanceSchedule") != 0,
        cle: int(s, "m_bRefTable") != 0,
        schema: s.get("m_areaSchemeId").and_then(Val::element_id).unwrap_or(-1),
        total_general: int(s, "m_showGrandTotal") != 0,
        titre_total: (int(s, "m_showGrandTotalTitle") != 0).then(|| s.get("m_grandTotalTitle").and_then(Val::str).unwrap_or("Total général").to_string()),
        nombre_total: int(s, "m_showGrandTotalCount") != 0,
        champs,
        filtres,
        tris,
    })
}

/// Contenu calculé : titres des colonnes visibles, lignes de données, colonnes non
/// calculées (indices).
pub struct Tableau {
    pub titres: Vec<String>,
    pub lignes: Vec<Vec<String>>,
    pub non_calculees: Vec<usize>,
}

/// Valeur brute d'un champ pour un élément (paramètre de l'occurrence ou de son type).
fn brut(m: &Maquette, element: i64, c: &Champ) -> Option<(Brut, Stockage)> {
    let cible = match c.genre {
        GenreChamp::Type => m.type_de(element)?,
        GenreChamp::PieceDepart => Some(m.pieces_de_porte(element).0).filter(|&p| p != -1)?,
        GenreChamp::PieceArrivee => Some(m.pieces_de_porte(element).1).filter(|&p| p != -1)?,
        GenreChamp::InfoProjet => m.info_projet,
        _ => element,
    };
    let v = m.parametres.get(&cible)?.iter().find(|(p, _)| *p == c.param)?.1.clone();
    let st = m.definitions.get(&c.param).map(|d| d.stockage).or_else(|| parametres::integres().get(&c.param).map(|i| i.stockage)).unwrap_or(Stockage::Aucun);
    Some((v.selon(st), st))
}

/// Valeur d'un paramètre de l'élément, sinon de son type.
fn parametre(m: &Maquette, element: i64, p: i64) -> Option<Valeur> {
    let st = m.definitions.get(&p).map(|d| d.stockage).or_else(|| parametres::integres().get(&p).map(|i| i.stockage)).unwrap_or(Stockage::Aucun);
    [Some(element), m.type_de(element)].into_iter().flatten().find_map(|e| m.parametres.get(&e)?.iter().find(|(q, _)| *q == p).map(|(_, b)| formules::de_brut(b, st)))
}

/// Totaux des champs pourcentage : champ → groupe (valeur du champ de regroupement,
/// vide pour le total général) → somme du champ dont il est la part.
type Totaux = HashMap<i64, HashMap<String, f64>>;

/// Groupe d'un élément pour un total de pourcentage.
fn groupe_pourcentage(m: &Maquette, element: i64, d: &Definition, par: i64) -> String {
    match d.champs.iter().find(|c| c.id == par).and_then(|c| valeur(m, element, d, c, &Totaux::new(), 1)) {
        Some(Valeur::Texte(s)) => s,
        Some(Valeur::Nombre(x)) => x.to_string(),
        None => String::new(),
    }
}

/// Valeur d'un champ pour un élément ; un champ calculé évalue sa formule, dont les
/// variables sont d'autres champs de la nomenclature ou des paramètres ; un pourcentage
/// est la part de l'élément dans le total de son groupe.
fn valeur(m: &Maquette, element: i64, d: &Definition, c: &Champ, totaux: &Totaux, profondeur: usize) -> Option<Valeur> {
    if profondeur > 16 {
        return None;
    }
    let champ = |id: i64| d.champs.iter().find(|c| c.id == id);
    match c.genre {
        GenreChamp::Formule => {
            let var = |x: &Expr| match x {
                Expr::Champ(k) => champ(*k).and_then(|c| valeur(m, element, d, c, totaux, profondeur + 1)),
                Expr::Param(p) => parametre(m, element, *p),
                _ => None,
            };
            formules::evaluer(c.formule.as_ref()?, &var)
        }
        GenreChamp::Pourcentage => {
            let (de, par) = c.pourcentage?;
            let x = valeur(m, element, d, champ(de)?, totaux, profondeur + 1)?.nombre()?;
            let t = *totaux.get(&c.id)?.get(&groupe_pourcentage(m, element, d, par))?;
            (t != 0.0).then(|| Valeur::Nombre(x / t))
        }
        GenreChamp::Nombre => Some(Valeur::Nombre(1.0)),
        _ => brut(m, element, c).map(|(b, st)| formules::de_brut(&b, st)),
    }
}

/// Texte du résultat d'une formule ou d'un pourcentage, dans l'unité du champ.
fn texte_formule(m: &Maquette, c: &Champ, v: &Valeur) -> String {
    match v {
        Valeur::Texte(s) => s.clone(),
        Valeur::Nombre(x) if c.spec.contains("spec.bool") => texte(m, c, &Brut::Entier(*x as i64), Stockage::Entier),
        Valeur::Nombre(x) => texte(m, c, &Brut::Reel(*x), Stockage::Reel),
    }
}

fn texte(m: &Maquette, c: &Champ, b: &Brut, st: Stockage) -> String {
    if let (Some(f), Brut::Reel(x)) = (&c.format, b) {
        if let Some(t) = parametres::formater(*x, f, m.unites.virgule) {
            return t;
        }
    }
    let (spec, enumeration) = match (m.definitions.get(&c.param), parametres::integres().get(&c.param)) {
        (Some(d), _) => (d.spec.as_str(), ""),
        (None, Some(i)) => (i.spec, i.enumeration),
        _ => (c.spec.as_str(), ""),
    };
    parametres::texte(b, c.param, st, spec, enumeration, &m.unites, &|e| m.nom_element(c.param, e)).unwrap_or_default()
}

fn nombre(b: &Brut) -> Option<f64> {
    match b {
        Brut::Reel(x) => Some(*x),
        Brut::Entier(i) | Brut::Id(i) => Some(*i as f64),
        _ => None,
    }
}

fn texte_brut(b: &Brut) -> String {
    match b {
        Brut::Texte(s) => s.clone(),
        Brut::Reel(x) => x.to_string(),
        Brut::Entier(i) | Brut::Id(i) => i.to_string(),
        Brut::Nomme { texte, .. } => texte.clone(),
    }
}

/// Un élément passe-t-il le filtre ? Valeur absente : la valeur par défaut du type de
/// la valeur du filtre.
fn passe(m: &Maquette, element: i64, d: &Definition, f: &Filtre) -> bool {
    let Some(c) = d.champs.iter().find(|c| c.id == f.champ) else { return true };
    let v = brut(m, element, c);
    if f.genre == 12 {
        return v.is_some();
    }
    // élément sans valeur : comme dans Revit, seules les négations passent
    let Some((v, _)) = v else { return matches!(f.genre, 1 | 7 | 9 | 11 | 14) };
    // texte affiché pour les ElementId (nom de l'élément), valeur sinon
    let a = match &v {
        Brut::Id(i) => m.nom_element(c.param, *i).unwrap_or_else(|| i.to_string()),
        b => texte_brut(b),
    };
    let ordre = match (nombre(&v), nombre(&f.valeur)) {
        (Some(x), Some(y)) if !matches!(f.valeur, Brut::Texte(_)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        _ => a.to_lowercase().cmp(&texte_brut(&f.valeur).to_lowercase()),
    };
    let (a, b) = (a.to_lowercase(), texte_brut(&f.valeur).to_lowercase());
    match f.genre {
        0 => ordre == Ordering::Equal,
        1 => ordre != Ordering::Equal,
        2 => ordre == Ordering::Greater,
        3 => ordre != Ordering::Less,
        4 => ordre == Ordering::Less,
        5 => ordre != Ordering::Greater,
        6 => a.contains(&b),
        7 => !a.contains(&b),
        8 => a.starts_with(&b),
        9 => !a.starts_with(&b),
        10 => a.ends_with(&b),
        11 => !a.ends_with(&b),
        13 => !a.is_empty(),
        14 => a.is_empty(),
        _ => true,
    }
}

/// Comparaison « naturelle » : nombres par valeur, textes sans casse, chiffres
/// compris (« R+2 » avant « R+10 »).
fn naturel(a: &str, b: &str) -> Ordering {
    if let (Ok(x), Ok(y)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
        return x.partial_cmp(&y).unwrap_or(Ordering::Equal);
    }
    let (mut i, mut j) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (i.peek().copied(), j.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let lire = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut s = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        s.push(c);
                        it.next();
                    }
                    s.parse::<u64>().unwrap_or(0)
                };
                let (n, o) = (lire(&mut i), lire(&mut j));
                if n != o {
                    return n.cmp(&o);
                }
            }
            (Some(x), Some(y)) => {
                let o = x.to_lowercase().cmp(y.to_lowercase());
                if o != Ordering::Equal {
                    return o;
                }
                i.next();
                j.next();
            }
        }
    }
}

/// Lignes d'un niveau de regroupement : groupes consécutifs de même valeur du champ de
/// tri, chacun avec son en-tête (valeur dans la première colonne), ses lignes, son pied
/// (totaux ; titre et nombre selon les drapeaux) ; une ligne vide sépare deux groupes
/// voisins, pas après le dernier (comme Revit).
#[allow(clippy::too_many_arguments)]
fn composer(
    niveau: usize,
    els: &[i64],
    d: &Definition,
    largeur: usize,
    cle: &dyn Fn(i64, &Tri) -> String,
    feuille: &dyn Fn(&[i64]) -> Vec<Vec<String>>,
    total: &dyn Fn(&[i64], Option<String>) -> Vec<String>,
    lignes: &mut Vec<Vec<String>>,
) {
    let Some(t) = d.tris.get(niveau) else {
        lignes.extend(feuille(els));
        return;
    };
    let mut k = 0;
    while k < els.len() {
        let valeur = cle(els[k], t);
        let mut fin = k + 1;
        while fin < els.len() && cle(els[fin], t) == valeur {
            fin += 1;
        }
        let groupe = &els[k..fin];
        if k > 0 && t.ligne_vide {
            lignes.push(vec![String::new(); largeur]);
        }
        if t.entete {
            let mut l = vec![String::new(); largeur];
            if let Some(x) = l.first_mut() {
                *x = valeur.clone();
            }
            lignes.push(l);
        }
        composer(niveau + 1, groupe, d, largeur, cle, feuille, total, lignes);
        if t.pied {
            let tete = match (t.titre_pied, t.nombre_pied) {
                (true, true) => Some(format!("{valeur}: {}", groupe.len())),
                (true, false) => Some(valeur),
                (false, true) => Some(groupe.len().to_string()),
                (false, false) => None,
            };
            lignes.push(total(groupe, tete));
        }
        k = fin;
    }
}

/// Contenu d'une nomenclature : éléments de sa catégorie filtrés, triés, puis regroupés
/// par les champs de tri (`composer`) : une ligne par élément (détail) ou par groupe des
/// champs de tri ; en regroupement, une colonne totalisée est sommée, une autre garde sa
/// valeur si elle est commune, « <multiples> » sinon (comme Revit).
pub fn calculer(m: &Maquette, d: &Definition) -> Tableau {
    let visibles: Vec<&Champ> = d.champs.iter().filter(|c| !c.masque).collect();
    // nomenclature de clés : ses lignes sont des clés, non lues
    if d.cle {
        return Tableau { titres: visibles.iter().map(|c| c.titre.clone()).collect(), lignes: Vec::new(), non_calculees: (0..visibles.len()).collect() };
    }
    // zones : celles du schéma de la nomenclature
    let schema_ok = |e: i64| d.schema == -1 || m.pieces.iter().find(|p| p.id == e).is_none_or(|p| p.schema == d.schema);
    let mut els: Vec<i64> = m.elements_de_categorie(d.categorie).into_iter().filter(|&e| schema_ok(e) && d.filtres.iter().all(|f| passe(m, e, d, f))).collect();
    // totaux des pourcentages, sur les éléments retenus
    let mut totaux = Totaux::new();
    for c in d.champs.iter().filter(|c| c.genre == GenreChamp::Pourcentage) {
        let Some((de, par)) = c.pourcentage else { continue };
        let Some(cd) = d.champs.iter().find(|x| x.id == de) else { continue };
        let mut t: HashMap<String, f64> = HashMap::new();
        for &e in &els {
            if let Some(x) = valeur(m, e, d, cd, &Totaux::new(), 1).and_then(|v| v.nombre()) {
                *t.entry(groupe_pourcentage(m, e, d, par)).or_default() += x;
            }
        }
        totaux.insert(c.id, t);
    }
    // valeurs affichées (texte) et brutes de chaque élément
    let cellule = |e: i64, c: &Champ| -> (String, Option<f64>) {
        match c.genre {
            GenreChamp::Nombre => ("1".into(), Some(1.0)),
            GenreChamp::Formule | GenreChamp::Pourcentage => match valeur(m, e, d, c, &totaux, 0) {
                Some(v) => (texte_formule(m, c, &v), v.nombre().filter(|_| !c.spec.contains("spec.bool"))),
                None => (String::new(), None),
            },
            _ => match brut(m, e, c) {
                Some((b, st)) => (texte(m, c, &b, st), nombre(&b).filter(|_| st == Stockage::Reel || st == Stockage::Entier)),
                None => (String::new(), None),
            },
        }
    };
    let cle = |e: i64, t: &Tri| d.champs.iter().find(|c| c.id == t.champ).map(|c| cellule(e, c).0).unwrap_or_default();
    // tri sur la valeur (nombre) quand il y en a une, sinon sur le texte ; un niveau se
    // range par altitude (RDC, R+1, R+2), comme dans Revit
    let cle_tri = |e: i64, t: &Tri| {
        let Some(c) = d.champs.iter().find(|c| c.id == t.champ) else { return Default::default() };
        let (texte, n) = cellule(e, c);
        let altitude = match brut(m, e, c) {
            Some((Brut::Id(i), _)) => m.niveaux.iter().find(|n| n.id == i).map(|n| n.altitude),
            _ => None,
        };
        (texte, altitude.or(n))
    };
    els.sort_by(|&a, &b| {
        for t in &d.tris {
            let (x, y) = (cle_tri(a, t), cle_tri(b, t));
            let o = match (x.1, y.1) {
                (Some(u), Some(v)) => u.partial_cmp(&v).unwrap_or(Ordering::Equal),
                _ => naturel(&x.0, &y.0),
            };
            let o = if t.decroissant { o.reverse() } else { o };
            if o != Ordering::Equal {
                return o;
            }
        }
        a.cmp(&b)
    });
    let titres = visibles.iter().map(|c| c.titre.clone()).collect();
    let non_calculees = visibles.iter().enumerate().filter(|(_, c)| !c.genre.calcule()).map(|(k, _)| k).collect();
    // somme d'une colonne totalisée sur des éléments (ceux qui ont une valeur), mise en
    // forme comme ses valeurs
    let somme = |groupe: &[i64], c: &Champ| -> Option<String> {
        let vals: Vec<f64> = groupe.iter().filter_map(|&e| cellule(e, c).1).collect();
        if !c.total || vals.is_empty() {
            return None;
        }
        let s: f64 = vals.iter().sum();
        if matches!(c.genre, GenreChamp::Formule | GenreChamp::Pourcentage) {
            return Some(texte_formule(m, c, &Valeur::Nombre(s)));
        }
        if let Some((Brut::Reel(_), st)) = groupe.first().and_then(|&e| brut(m, e, c)) {
            return Some(texte(m, c, &Brut::Reel(s), st));
        }
        Some(crate::step::format_real(s))
    };
    // lignes du bas d'un regroupement : une par élément (détail), sinon une pour le
    // groupe (colonne totalisée sommée, valeur commune ou « <multiples> »)
    let feuille = |groupe: &[i64]| -> Vec<Vec<String>> {
        if d.detail {
            return groupe.iter().map(|&e| visibles.iter().map(|c| cellule(e, c).0).collect()).collect();
        }
        vec![visibles
            .iter()
            .map(|c| {
                if c.genre == GenreChamp::Nombre {
                    return groupe.len().to_string();
                }
                if let Some(s) = somme(groupe, c) {
                    return s;
                }
                let vals: Vec<String> = groupe.iter().map(|&e| cellule(e, c).0).collect();
                if vals.iter().all(|v| v == &vals[0]) { vals[0].clone() } else { "<multiples>".into() }
            })
            .collect()]
    };
    // ligne de total (pied de groupe, total général) : titre ou nombre dans la première
    // colonne, nombre d'éléments dans un champ Nombre, sommes des colonnes totalisées
    let total = |groupe: &[i64], tete: Option<String>| -> Vec<String> {
        visibles
            .iter()
            .enumerate()
            .map(|(k, c)| match (&tete, c.genre) {
                (Some(t), _) if k == 0 => t.clone(),
                (_, GenreChamp::Nombre) => groupe.len().to_string(),
                _ => somme(groupe, c).unwrap_or_default(),
            })
            .collect()
    };
    let mut lignes = Vec::new();
    // sans détail ni champ de tri : une seule ligne pour tous les éléments
    if !els.is_empty() {
        composer(0, &els, d, visibles.len(), &cle, &feuille, &total, &mut lignes);
    }
    // total général : « Total général: 112 » comme Revit
    if d.total_general && !els.is_empty() {
        let tete = match (&d.titre_total, d.nombre_total) {
            (Some(t), true) => Some(format!("{t}: {}", els.len())),
            (Some(t), false) => Some(t.clone()),
            (None, true) => Some(els.len().to_string()),
            (None, false) => None,
        };
        lignes.push(total(&els, tete));
    }
    Tableau { titres, lignes, non_calculees }
}

/// Tris et regroupements en clair : « Niveau (croissant, en-tête, pied, ligne vide) ».
pub fn texte_tris(d: &Definition) -> String {
    d.tris
        .iter()
        .map(|t| {
            let nom = d.champs.iter().find(|c| c.id == t.champ).map(|c| c.titre.as_str()).unwrap_or("?");
            let mut o = vec![if t.decroissant { "décroissant" } else { "croissant" }];
            o.extend([(t.entete, "en-tête"), (t.pied, "pied"), (t.ligne_vide, "ligne vide")].iter().filter(|x| x.0).map(|x| x.1));
            format!("{nom} ({})", o.join(", "))
        })
        .collect::<Vec<_>>()
        .join(" ; ")
}
