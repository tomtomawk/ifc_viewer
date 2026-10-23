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
//! recomposé ici depuis les éléments lus et leurs paramètres ; les champs calculés
//! (formules, pièce de départ et d'arrivée d'une porte, pourcentages…) ne le sont pas.

use super::champs::{Obj, Record, Val};
use super::maquette::Maquette;
use super::parametres::{self, Brut, Stockage};
use std::cmp::Ordering;

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
        matches!(self, GenreChamp::Occurrence | GenreChamp::Type | GenreChamp::Nombre)
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
}

#[derive(Clone, Debug)]
pub struct Definition {
    pub categorie: i64,
    pub detail: bool,
    /// Nomenclature de clés (`m_bRefTable`).
    pub cle: bool,
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
    let Some(o) = o else { return Brut::Entier(0) };
    match o.get("m_storageType").and_then(Val::int) {
        Some(2) => Brut::Reel(o.get("m_dbl").and_then(Val::real).unwrap_or(0.0)),
        Some(3) => Brut::Texte(o.get("m_str").and_then(Val::obj).and_then(|s| s.get("m_str")).and_then(Val::str).unwrap_or_default().to_string()),
        Some(4) => Brut::Id(o.get("m_elemId").and_then(Val::element_id).unwrap_or(-1)),
        _ => Brut::Entier(o.get("m_int").and_then(Val::int).unwrap_or(0)),
    }
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
        champs.push(Champ {
            id: ids.get(k).copied().unwrap_or(k as i64),
            titre: f.get("m_columnHeading").and_then(Val::str).unwrap_or_default().to_string(),
            param: f.get("m_paramElemId").and_then(Val::element_id).unwrap_or(-1),
            masque: int(f, "m_hidden") != 0,
            total: int(f, "m_displayType") == 1,
            genre: GenreChamp::de(int(f, "m_sourceType")),
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
            Tri { champ: id_champ(t.get("m_fieldId")), decroissant: int(t, "m_sortOrder") != 0, entete: d & 2 != 0, pied: d & 4 != 0, ligne_vide: d & 16 != 0 }
        })
        .collect();
    Some(Definition {
        categorie: s.get("m_categoryId").and_then(Val::element_id).unwrap_or(-1),
        detail: int(s, "m_bInstanceSchedule") != 0,
        cle: int(s, "m_bRefTable") != 0,
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
        _ => element,
    };
    let v = m.parametres.get(&cible)?.iter().find(|(p, _)| *p == c.param)?.1.clone();
    let st = m.definitions.get(&c.param).map(|d| d.stockage).or_else(|| parametres::integres().get(&c.param).map(|i| i.stockage)).unwrap_or(Stockage::Aucun);
    Some((v.selon(st), st))
}

fn texte(m: &Maquette, c: &Champ, b: &Brut, st: Stockage) -> String {
    let (spec, enumeration) = match (m.definitions.get(&c.param), parametres::integres().get(&c.param)) {
        (Some(d), _) => (d.spec.as_str(), ""),
        (None, Some(i)) => (i.spec, i.enumeration),
        _ => ("", ""),
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

/// Contenu d'une nomenclature : éléments de sa catégorie filtrés, triés, puis une ligne
/// par élément (détail) ou par groupe des champs de tri ; en regroupement, une colonne
/// totalisée est sommée, une autre garde sa valeur si elle est commune, « <multiples> »
/// sinon (comme Revit).
pub fn calculer(m: &Maquette, d: &Definition) -> Tableau {
    let visibles: Vec<&Champ> = d.champs.iter().filter(|c| !c.masque).collect();
    // nomenclature de clés : ses lignes sont des clés, non lues
    if d.cle {
        return Tableau { titres: visibles.iter().map(|c| c.titre.clone()).collect(), lignes: Vec::new(), non_calculees: (0..visibles.len()).collect() };
    }
    let mut els: Vec<i64> = m.elements_de_categorie(d.categorie).into_iter().filter(|&e| d.filtres.iter().all(|f| passe(m, e, d, f))).collect();
    // valeurs affichées (texte) et brutes de chaque élément
    let cellule = |e: i64, c: &Champ| -> (String, Option<f64>) {
        match c.genre {
            GenreChamp::Nombre => ("1".into(), Some(1.0)),
            _ => match brut(m, e, c) {
                Some((b, st)) => (texte(m, c, &b, st), nombre(&b).filter(|_| st == Stockage::Reel || st == Stockage::Entier)),
                None => (String::new(), None),
            },
        }
    };
    let cle = |e: i64, t: &Tri| d.champs.iter().find(|c| c.id == t.champ).map(|c| cellule(e, c).0).unwrap_or_default();
    // tri sur la valeur (nombre) quand il y en a une, sinon sur le texte
    let cle_tri = |e: i64, t: &Tri| d.champs.iter().find(|c| c.id == t.champ).map(|c| cellule(e, c)).unwrap_or_default();
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
    let mut lignes = Vec::new();
    if d.detail {
        for &e in &els {
            lignes.push(visibles.iter().map(|c| cellule(e, c).0).collect());
        }
    } else if !d.tris.is_empty() {
        // groupes consécutifs de mêmes clés de tri (sans champ de tri : aucune ligne,
        // seulement le total général)
        let mut k = 0;
        while k < els.len() {
            let cles: Vec<String> = d.tris.iter().map(|t| cle(els[k], t)).collect();
            let mut fin = k + 1;
            while fin < els.len() && d.tris.iter().map(|t| cle(els[fin], t)).collect::<Vec<_>>() == cles {
                fin += 1;
            }
            let groupe = &els[k..fin];
            lignes.push(
                visibles
                    .iter()
                    .map(|c| {
                        let vals: Vec<(String, Option<f64>)> = groupe.iter().map(|&e| cellule(e, c)).collect();
                        if c.genre == GenreChamp::Nombre {
                            return groupe.len().to_string();
                        }
                        if c.total && vals.iter().all(|v| v.1.is_some()) {
                            let s: f64 = vals.iter().filter_map(|v| v.1).sum();
                            // somme mise en forme comme les valeurs de la colonne
                            if let Some(e) = groupe.first() {
                                if let Some((Brut::Reel(_), st)) = brut(m, *e, c) {
                                    return texte(m, c, &Brut::Reel(s), st);
                                }
                            }
                            return crate::step::format_real(s);
                        }
                        let premier = &vals[0].0;
                        if vals.iter().all(|v| &v.0 == premier) { premier.clone() } else { "<multiples>".into() }
                    })
                    .collect(),
            );
            k = fin;
        }
    }
    // total général : nombre d'éléments et sommes des colonnes totalisées
    if d.total_general && !els.is_empty() {
        lignes.push(
            visibles
                .iter()
                .enumerate()
                .map(|(k, c)| {
                    if c.genre == GenreChamp::Nombre {
                        return els.len().to_string();
                    }
                    if c.total {
                        let vals: Vec<Option<f64>> = els.iter().map(|&e| cellule(e, c).1).collect();
                        if vals.iter().all(Option::is_some) {
                            let s: f64 = vals.iter().flatten().sum();
                            if let Some((Brut::Reel(_), st)) = brut(m, els[0], c) {
                                return texte(m, c, &Brut::Reel(s), st);
                            }
                            return crate::step::format_real(s);
                        }
                    }
                    if k > 0 {
                        return String::new();
                    }
                    // « Total général: 112 » comme Revit
                    match (&d.titre_total, d.nombre_total) {
                        (Some(t), true) => format!("{t}: {}", els.len()),
                        (Some(t), false) => t.clone(),
                        (None, true) => els.len().to_string(),
                        (None, false) => String::new(),
                    }
                })
                .collect(),
        );
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
