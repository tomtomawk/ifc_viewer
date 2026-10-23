//! Feuilles (LISEZMOI-RVT.md du dossier Decodage RVT § 7 « Feuilles ») : cartouches, symboles et autres
//! instances propres à une vue, titres des fenêtres de vue, étiquettes remplies par les
//! paramètres, et passage du modèle à la feuille pour chaque fenêtre de vue.
//!
//! - Fenêtre de vue (`Viewport`) : `m_offset` est la position sur la feuille de l'origine
//!   de la vue ; un point p du modèle se place en `m_offset` + `m_scale` · ((p − origine)
//!   · droite, (p − origine) · haut). Boîte de l'API (`GetBoxOutline`) : contour de la
//!   vue (son cadrage s'il est actif) élargi de 0,01 pied de chaque côté.
//! - Instance propre à une vue (`FamilyInstance` avec `m_ownerDBViewId`) : dessin du type
//!   (`GElement` du `FamilySymbol` : traits et régions) placé par `m_pInstanceInfo.m_Trf`,
//!   textes et étiquettes (`TagNote`) du document de la famille, placés de même.
//! - Étiquette : paramètres `m_paramDataFormattingArr` ; valeur lue sur la feuille, puis
//!   l'instance, puis les informations du projet ; échelle et révision actuelle calculées.

use super::maquette::{InstanceVue, Maquette};
use super::parametres::{self, Brut};
use super::vues::{self, Alignement, Contenu, Element2D, ParamEtiquette, Vertical, Vue};
use crate::geom::{Part, Rgba};
use crate::scene::NavFenetre;
use crate::texte;
use glam::{DMat3, DMat4, DVec3, DVec4};
use std::collections::HashMap;

/// Taille de texte (mm) d'un type dont la police n'est pas connue.
const TAILLE_DEFAUT: f64 = 2.5;
/// Épaisseur des traits sur le papier (mm).
const TRAIT: f64 = 0.18;
const NOIR: Rgba = [0.08, 0.08, 0.08, 1.0];
const OST_DETAIL_COMPONENTS: i64 = -2_002_000;
const OST_TITLE_BLOCKS: i64 = -2_000_280;

const SHEET_SCALE: i64 = -1_007_402;
const SHEET_CURRENT_REVISION: i64 = -1_007_413;
const SHEET_CURRENT_REVISION_DESCRIPTION: i64 = -1_007_414;
const SHEET_CURRENT_REVISION_DATE: i64 = -1_007_415;
const SHEET_CURRENT_REVISION_ISSUED_TO: i64 = -1_007_416;
const SHEET_CURRENT_REVISION_ISSUED_BY: i64 = -1_007_417;
const VIEW_NAME: i64 = -1_005_112;
const VIEW_DESCRIPTION: i64 = -1_005_114;
const VIEW_SCALE: i64 = -1_005_150;
const VIEW_SCALE_PULLDOWN_METRIC: i64 = -1_005_151;
const VIEWPORT_DETAIL_NUMBER: i64 = -1_005_201;
const VIEWPORT_VIEW_NAME: i64 = -1_005_203;
const VIEWPORT_SCALE: i64 = -1_005_204;

/// Texte de la valeur d'un paramètre d'un élément lu, mis en forme comme dans les
/// propriétés ; None s'il est absent ou vide.
fn texte_param(m: &Maquette, element: i64, param: i64) -> Option<String> {
    let (_, b) = m.parametres.get(&element)?.iter().find(|(p, _)| *p == param)?;
    let (st, spec, en) = match (m.definitions.get(&param), parametres::integres().get(&param)) {
        (Some(d), _) => (d.stockage, d.spec.as_str(), ""),
        (None, Some(i)) => (i.stockage, i.spec, i.enumeration),
        _ => return match b {
            Brut::Texte(s) if !s.is_empty() => Some(s.clone()),
            _ => None,
        },
    };
    parametres::texte(b, param, st, spec, en, &|e| m.nom_element(param, e))
}

/// Échelle comme Revit l'écrit sur une feuille (« 1 : 50 »).
fn echelle_texte(e: f64) -> String {
    format!("1 : {}", e.round())
}

/// Révision actuelle d'une feuille : celle de plus grand numéro d'ordre parmi les
/// révisions présentes (`m_explicitlyPresentRevisionIds`).
fn revision_actuelle<'m>(m: &'m Maquette, f: &Vue) -> Option<&'m super::maquette::Revision> {
    f.revisions.iter().filter_map(|r| m.revisions.get(r)).max_by_key(|r| r.sequence)
}

/// Fenêtres de vue d'une feuille : (élément, vue placée).
fn fenetres<'m>(m: &'m Maquette, f: &Vue) -> Vec<(&'m Element2D, &'m Vue)> {
    m.elements_2d
        .iter()
        .filter(|e| e.vue == f.id)
        .filter_map(|e| match &e.contenu {
            Contenu::FenetreDeVue { vue, .. } => Some((e, m.vues.iter().find(|v| v.id == *vue)?)),
            _ => None,
        })
        .collect()
}

/// Valeur d'un paramètre affiché par le cartouche `instance` de la feuille `f`.
pub fn valeur_cartouche(m: &Maquette, f: &Vue, instance: i64, param: i64) -> Option<String> {
    let rev = || revision_actuelle(m, f);
    match param {
        SHEET_SCALE => {
            let mut e: Vec<f64> = fenetres(m, f).iter().filter(|(_, v)| v.genre != "Nomenclature" && v.genre != "Légende").map(|(_, v)| v.echelle).collect();
            e.sort_by(f64::total_cmp);
            e.dedup();
            return match e.as_slice() {
                [] => None,
                [x] => Some(echelle_texte(*x)),
                _ => Some("Comme indiqué".into()),
            };
        }
        SHEET_CURRENT_REVISION => return rev().map(|r| r.numero.clone()).filter(|s| !s.is_empty()),
        SHEET_CURRENT_REVISION_DESCRIPTION => return rev().map(|r| r.description.clone()).filter(|s| !s.is_empty()),
        SHEET_CURRENT_REVISION_DATE => return rev().map(|r| r.date.clone()).filter(|s| !s.is_empty()),
        SHEET_CURRENT_REVISION_ISSUED_TO => return rev().map(|r| r.remise_a.clone()).filter(|s| !s.is_empty()),
        SHEET_CURRENT_REVISION_ISSUED_BY => return rev().map(|r| r.remise_par.clone()).filter(|s| !s.is_empty()),
        _ => {}
    }
    texte_param(m, f.id, param).or_else(|| texte_param(m, instance, param)).or_else(|| texte_param(m, m.info_projet, param))
}

/// Valeur d'un paramètre affiché par le titre d'une fenêtre de vue.
pub fn valeur_titre(m: &Maquette, fenetre: &Element2D, v: &Vue, param: i64) -> Option<String> {
    match param {
        // le titre sur la feuille remplace le nom de la vue (export de Revit)
        VIEW_NAME | VIEWPORT_VIEW_NAME => Some(if v.titre_feuille.is_empty() { v.nom.clone() } else { v.titre_feuille.clone() }),
        VIEW_SCALE | VIEW_SCALE_PULLDOWN_METRIC | VIEWPORT_SCALE => Some(echelle_texte(v.echelle)),
        VIEWPORT_DETAIL_NUMBER => texte_param(m, fenetre.id, param),
        VIEW_DESCRIPTION => texte_param(m, v.id, param),
        _ => texte_param(m, fenetre.id, param).or_else(|| texte_param(m, v.id, param)).or_else(|| texte_param(m, m.info_projet, param)),
    }
}

/// Texte d'une étiquette : préfixe, valeur et suffixe de chaque paramètre, séparés par
/// une espace ou un saut de ligne ; paramètre sans valeur omis.
fn texte_etiquette(params: &[ParamEtiquette], valeur: &dyn Fn(i64) -> Option<String>) -> String {
    let mut out = String::new();
    for p in params {
        let Some(v) = valeur(p.param) else { continue };
        if !out.is_empty() {
            out.push(if p.a_la_ligne { '\n' } else { ' ' });
        }
        out.push_str(&format!("{}{}{}", p.prefixe, v, p.suffixe));
    }
    out
}

/// Nom et taille (mm, capitales) de la police d'un type de texte ou d'étiquette.
pub fn police(m: &Maquette, type_id: i64) -> (&str, f64) {
    match m.polices.get(&type_id) {
        Some(p) if p.taille > 0.0 => (p.nom.as_str(), p.taille),
        _ => ("Arial", TAILLE_DEFAUT),
    }
}

/// Triangles (mm) d'un texte mis en page dans son cadre : lignes coupées à la largeur
/// du cadre, alignées sur le point d'ancrage (à gauche, au milieu ou à droite ; bloc de
/// lignes en dessous, centré ou au-dessus). Mesuré sur l'export d'une feuille par
/// Revit : première ligne de base à l'ascendante d'Arial sous le haut du bloc, interligne
/// de 1,7 fois la taille, bloc de l'ascendante de la première ligne à la descendante de
/// la dernière. `base` et `haut` portent l'échelle (longueur 1 sur une feuille, celle de
/// la vue sinon).
#[allow(clippy::too_many_arguments)]
pub fn triangles_texte(texte: &str, point: DVec3, base: DVec3, haut: DVec3, largeur: f64, alignement: Alignement, vertical: Vertical, police: (&str, f64), gras: bool) -> Vec<[DVec3; 3]> {
    let (nom, taille) = police;
    let (asc, desc) = texte::metriques(nom, taille, gras);
    let interligne = 1.7 * taille;
    // coupure tolérante : nos largeurs de glyphes ne sont pas celles de Revit au
    // dixième de millimètre près (« Ech : » d'E13 : 7,81 mm dans Revit, 7,86 ici)
    let lignes = texte::couper(texte, nom, taille, gras, largeur * 1.02);
    let bloc = asc + (lignes.len().max(1) - 1) as f64 * interligne + desc;
    let dy = match vertical {
        Vertical::Haut => 0.0,
        Vertical::Milieu => bloc / 2.0,
        Vertical::Bas => bloc,
    };
    let mut out = Vec::new();
    for (k, l) in lignes.iter().enumerate() {
        let ligne = texte::ligne(l, nom, taille, gras);
        let x0 = match alignement {
            Alignement::Gauche => 0.0,
            Alignement::Centre => -ligne.largeur / 2.0,
            Alignement::Droite => -ligne.largeur,
        };
        let y0 = dy - asc - k as f64 * interligne;
        out.extend(ligne.triangles.iter().map(|t| t.map(|q| point + (x0 + q[0]) * base + (y0 + q[1]) * haut)));
    }
    out
}

/// Couleur d'un type de texte : celle de sa sous-catégorie (`GStyleElem`), noire sinon.
pub fn couleur_texte(m: &Maquette, type_id: i64) -> Rgba {
    let cat = m.types.get(&type_id).map(|t| t.categorie_texte).or_else(|| m.categories_textes.get(&type_id).copied());
    match cat.and_then(|c| m.couleurs_categories.get(&c)) {
        Some(c) => [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0],
        None => NOIR,
    }
}

/// Couleur et glyphes d'une note de texte d'une vue (mm du modèle : taille × échelle de
/// la vue).
pub fn note(m: &Maquette, e: &Element2D, echelle: f64) -> Option<(Rgba, Vec<[DVec3; 3]>)> {
    let Contenu::Texte { texte, point, base, haut, largeur, alignement, vertical, gras, type_id, .. } = &e.contenu else { return None };
    let t = triangles_texte(texte, *point, *base * echelle, *haut * echelle, *largeur, *alignement, *vertical, police(m, *type_id), *gras);
    (!t.is_empty()).then(|| (couleur_texte(m, *type_id), t))
}

/// Boîte (mm) de chaque élément du modèle maillé, par id.
pub fn boites(m: &Maquette) -> HashMap<i64, (DVec3, DVec3)> {
    let geoms = m.murs.iter().map(|x| (x.id, &x.geometrie)).chain(m.dalles.iter().map(|x| (x.id, &x.geometrie))).chain(m.toits.iter().map(|x| (x.id, &x.geometrie)));
    let geoms = geoms.chain(m.toposolides.iter().map(|x| (x.id, &x.geometrie))).chain(m.escaliers.iter().map(|x| (x.id, &x.geometrie)));
    let geoms = geoms.chain(m.garde_corps.iter().map(|x| (x.id, &x.geometrie))).chain(m.instances.iter().map(|x| (x.id, &x.geometrie)));
    geoms
        .filter_map(|(id, g)| {
            let g = g.as_ref().ok()?;
            let b = g.sommets.iter().fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(a, b), p| (a.min(*p), b.max(*p)));
            (b.0.x <= b.1.x).then_some((id, b))
        })
        .collect()
}

/// Catégorie (BuiltInCategory) d'un élément du modèle : celle de la famille d'une
/// instance, sinon d'après sa classe (murs, sols, plafonds, toits, escaliers,
/// garde-corps, pièces, niveaux, quadrillages).
pub fn categorie(m: &Maquette, id: i64) -> Option<i64> {
    if let Some(i) = m.instances.iter().find(|i| i.id == id) {
        return m.types.get(&i.type_id).and_then(|t| m.familles.get(&t.famille)).map(|f| f.categorie);
    }
    if m.murs.iter().any(|x| x.id == id) {
        return Some(-2_000_011);
    }
    if let Some(d) = m.dalles.iter().find(|x| x.id == id) {
        return Some(if d.classe == "Ceiling" { -2_000_038 } else { -2_000_032 });
    }
    let c = if m.toits.iter().any(|x| x.id == id) {
        -2_000_035
    } else if m.escaliers.iter().any(|x| x.id == id) {
        -2_000_120
    } else if m.garde_corps.iter().any(|x| x.id == id) {
        -2_000_126
    } else if m.pieces.iter().any(|x| x.id == id) {
        -2_000_160
    } else if m.niveaux.iter().any(|x| x.id == id) {
        -2_000_240
    } else if m.grilles.iter().any(|x| x.id == id) {
        -2_000_220
    } else {
        return None;
    };
    Some(c)
}

/// Éléments du modèle dont la catégorie est masquée dans la vue `v`.
pub fn masques_par_categorie(m: &Maquette, v: &Vue) -> Vec<i64> {
    if v.categories_masquees.is_empty() {
        return Vec::new();
    }
    let ids = m.murs.iter().map(|x| x.id).chain(m.dalles.iter().map(|x| x.id)).chain(m.toits.iter().map(|x| x.id)).chain(m.escaliers.iter().map(|x| x.id));
    let ids = ids.chain(m.garde_corps.iter().map(|x| x.id)).chain(m.instances.iter().map(|x| x.id)).chain(m.pieces.iter().map(|x| x.id));
    ids.filter(|&id| categorie(m, id).is_some_and(|c| v.categories_masquees.contains(&c))).collect()
}

/// Élément visible dans la vue `v` comme dans Revit : pas masqué dans la vue ; une cote
/// seulement si chacun des éléments qu'elle cote y est visible (pas une ligne
/// d'esquisse, pas masqué, catégorie affichée, boîte qui touche le cadrage de la vue) :
/// vérifié contre le collecteur de la vue dans l'API (Kerreut, « RDC 50e » et « PC Est »).
pub fn visible_dans_vue(m: &Maquette, e: &Element2D, v: &Vue, boites: &HashMap<i64, (DVec3, DVec3)>) -> bool {
    if v.masques.contains(&e.id) {
        return false;
    }
    let Contenu::Cote { references, .. } = &e.contenu else { return true };
    let cadre = v.cadre.filter(|c| c.actif[0] && c.actif[1]);
    references.iter().all(|r| {
        if m.lignes_esquisse.contains(r) || v.masques.contains(r) || categorie(m, *r).is_some_and(|c| v.categories_masquees.contains(&c)) {
            return false;
        }
        match (cadre, boites.get(r)) {
            (Some(c), Some((a, b))) => (0..2).all(|i| {
                let ax = c.axes[i];
                let (lo, hi) = [a.x, b.x].iter().flat_map(|&x| [a.y, b.y].map(move |y| (x, y))).flat_map(|(x, y)| [a.z, b.z].map(move |z| DVec3::new(x, y, z))).map(|p| (p - c.origine).dot(ax)).fold((f64::MAX, f64::MIN), |(l, h), t| (l.min(t), h.max(t)));
                hi >= c.min[i] && lo <= c.max[i]
            }),
            _ => true,
        }
    })
}

/// Tracé d'une cote comme dans Revit (couleur de la sous-catégorie de son type, noire
/// sinon ; triangles en m) : ligne de cote entre les points extrêmes, prolongée de
/// `DIM_LINE_EXTENSION` ; ligne d'attache perpendiculaire à chaque point de référence,
/// de longueur fixe ou jusqu'à l'élément moins l'écart, prolongée au-delà de la ligne ;
/// marque oblique à 45° ; valeur de chaque segment au-dessus de la ligne (à gauche pour
/// une cote verticale), centrée sur le milieu du segment, au format du type ou du projet.
pub fn cote(m: &Maquette, e: &Element2D, normale: DVec3, droite: DVec3, echelle: f64) -> Option<Vec<Part>> {
    let Contenu::Cote { origine, direction: Some(d), segments, points, type_id, textes, .. } = &e.contenu else { return None };
    let t = m.types_cotes.get(type_id);
    let k = |x: f64| x * echelle;
    let (ext_ligne, ext_attache, ecart, longueur, fixe, decalage) = match t {
        Some(t) => (t.extension_ligne, t.extension_attache, t.ecart_element, t.longueur_attache, t.attache_fixe, t.decalage_texte),
        None => (0.0, 2.0, 1.5, 0.0, false, 1.0),
    };
    let n = normale.cross(*d).normalize_or_zero();
    let sur_ligne = |p: DVec3| *origine + (p - *origine).dot(*d) * *d;
    let ts: Vec<f64> = points.iter().chain(segments.iter().map(|s| &s.1)).map(|p| (*p - *origine).dot(*d)).collect();
    let (t0, t1) = ts.iter().fold((f64::MAX, f64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
    if t0 > t1 {
        return None;
    }
    let mut traits = Vec::new();
    let l = TRAIT * echelle;
    vues::ruban(&[*origine + (t0 - k(ext_ligne)) * *d, *origine + (t1 + k(ext_ligne)) * *d], normale, l, &mut traits);
    let mut marques = Vec::new();
    let oblique = (*d + n).normalize() * k(1.25);
    for p in points {
        let q = sur_ligne(*p);
        let dist = (*p - q).dot(n);
        let cote_elt = if dist.abs() > 1e-9 { dist.signum() } else { 1.0 };
        // de l'élément (ou de la longueur fixe) jusqu'au-delà de la ligne de cote
        let debut = if fixe { cote_elt * k(longueur) } else if dist.abs() > k(ecart) { dist - cote_elt * k(ecart) } else { 0.0 };
        let fin = -cote_elt * k(ext_attache);
        if (debut - fin).abs() > 1e-9 {
            vues::ruban(&[q + debut * n, q + fin * n], normale, l, &mut traits);
        }
        vues::ruban(&[q - oblique, q + oblique], normale, 2.5 * l, &mut marques);
    }
    let couleur = t.map(|t| t.categorie).and_then(|c| m.couleurs_categories.get(&c)).map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0]).unwrap_or(NOIR);
    // valeurs : sens de lecture vers la droite de la vue, ou vers son haut pour une cote
    // verticale ; texte au-dessus de la ligne dans ce sens
    let mut r = *d;
    if r.dot(droite) < -1e-6 || (r.dot(droite).abs() <= 1e-6 && r.dot(normale.cross(droite)) < 0.0) {
        r = -r;
    }
    let dessus = normale.cross(r);
    let format = t.and_then(|t| t.format.as_ref()).or(m.format_longueurs.as_ref());
    let (nom, taille) = police(m, *type_id);
    let gras = t.is_some_and(|t| t.gras);
    let mut glyphes = Vec::new();
    for (k_seg, (v, milieu)) in segments.iter().enumerate() {
        // valeur remplacée par un texte, ou encadrée d'un préfixe et d'un suffixe
        let texte = match textes.get(k_seg) {
            Some(t) if !t.remplacement.is_empty() => t.remplacement.clone(),
            Some(t) => format!("{}{}{}", t.prefixe, vues::texte_longueur(*v, format), t.suffixe),
            None => vues::texte_longueur(*v, format),
        };
        let base = sur_ligne(*milieu) + dessus * k(decalage);
        glyphes.extend(triangles_texte(&texte, base, r * echelle, dessus * echelle, 0.0, Alignement::Centre, Vertical::Bas, (nom, taille), gras));
    }
    // rubans déjà en mètres ; glyphes en mm
    let parts = [traits, marques, en_m(glyphes)].into_iter().map(|tris| Part { color: couleur, tris });
    Some(parts.filter(|p| !p.tris.is_empty()).collect())
}

/// Triangles en mètres.
fn en_m(t: Vec<[DVec3; 3]>) -> Vec<[DVec3; 3]> {
    t.into_iter().map(|t| t.map(|p| p / 1000.0)).collect()
}

/// Visibilité d'un texte de famille commandée par un paramètre (Oui/Non) : valeur du
/// paramètre de même nom dans l'instance ou le type ; visible si elle est inconnue.
fn visible(m: &Maquette, e: &Element2D, type_id: i64, instance: i64) -> bool {
    let Some(nom) = m.params_embarques.get(&e.visible_si) else { return true };
    let vals = [instance, type_id].into_iter().filter_map(|x| m.parametres.get(&x)).flatten();
    for (p, b) in vals {
        if m.definitions.get(p).is_some_and(|d| &d.nom == nom) {
            return !matches!(b.selon(parametres::Stockage::Entier), Brut::Entier(0));
        }
    }
    true
}

/// Parts (triangles en m) du dessin du type `type_id` placé par (origine, axes) : traits
/// en rubans de `TRAIT` mm sur le papier (`echelle` : mm du modèle par mm du papier),
/// régions remplies, textes et étiquettes du document de la famille (sauf ceux que
/// masque un paramètre de visibilité de l'instance `instance` ou du type).
#[allow(clippy::too_many_arguments)]
fn dessin_place(m: &Maquette, type_id: i64, instance: i64, origine: DVec3, axes: DMat3, normale: DVec3, echelle: f64, valeur: &dyn Fn(i64) -> Option<String>) -> Vec<Part> {
    let place = |p: DVec3| origine + axes * p;
    let mut parts = Vec::new();
    let doc = m.types.get(&type_id).and_then(|t| m.familles.get(&t.famille)).and_then(|f| f.document);
    let elements: Vec<&Element2D> = doc.and_then(|d| m.textes_familles.get(&d)).map(Vec::as_slice).unwrap_or_default().iter().filter(|e| visible(m, e, type_id, instance)).collect();
    // régions remplies du document de la famille (couleurs de leur type), sous les traits ;
    // à défaut, faces du dessin du type, noires
    let regions: Vec<&Element2D> = elements.iter().copied().filter(|e| matches!(e.contenu, Contenu::Region { .. }) && e.geometrie.is_ok()).collect();
    if regions.is_empty() {
        for (c, tris) in m.dessins.get(&type_id).map(|d| d.faces.as_slice()).unwrap_or_default() {
            let color = c.map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0]).unwrap_or(NOIR);
            parts.push(Part { color, tris: en_m(tris.iter().map(|t| t.map(place)).collect()) });
        }
    }
    for e in regions {
        let (Contenu::Region { type_id: t }, Ok(g)) = (&e.contenu, &e.geometrie) else { continue };
        let face: Vec<[DVec3; 3]> = g.faces.iter().flat_map(|f| f.triangles.iter().map(|t| t.map(|i| place(g.sommets[i])))).collect();
        parts.extend(region(m, *t, e.masque, face, axes, echelle, normale));
    }
    if let Some(d) = m.dessins.get(&type_id) {
        let mut traits = Vec::new();
        for l in &d.traits {
            let pts: Vec<DVec3> = l.iter().map(|p| place(*p)).collect();
            vues::ruban(&pts, normale, TRAIT * echelle, &mut traits);
        }
        if !traits.is_empty() {
            parts.push(Part { color: NOIR, tris: traits });
        }
    }
    let mut glyphes: Vec<(Rgba, Vec<[DVec3; 3]>)> = Vec::new();
    for e in elements {
        let (texte, point, base, haut, largeur, alignement, vertical, gras, type_texte) = match &e.contenu {
            Contenu::Texte { texte, point, base, haut, largeur, alignement, vertical, gras, type_id, .. } => (texte.clone(), point, base, haut, largeur, alignement, vertical, gras, type_id),
            Contenu::Etiquette { params, point, base, haut, largeur, alignement, vertical, gras, type_id, .. } => (texte_etiquette(params, valeur), point, base, haut, largeur, alignement, vertical, gras, type_id),
            _ => continue,
        };
        if texte.is_empty() {
            continue;
        }
        let t = triangles_texte(&texte, place(*point), axes * *base, axes * *haut, *largeur, *alignement, *vertical, police(m, *type_texte), *gras);
        let c = couleur_texte(m, *type_texte);
        match glyphes.iter_mut().find(|g| g.0 == c) {
            Some(g) => g.1.extend(t),
            None => glyphes.push((c, t)),
        }
    }
    parts.extend(glyphes.into_iter().filter(|g| !g.1.is_empty()).map(|(color, t)| Part { color, tris: en_m(t) }));
    parts
}

/// Région remplie d'une famille placée (triangles en mm, repère de la feuille ou de la
/// vue) : blanche pour une région de masquage ; sinon arrière-plan uni, premier plan uni
/// (couleur de la sous-catégorie du type) ou hachuré, comme les régions des vues.
fn region(m: &Maquette, type_id: i64, masque: bool, face: Vec<[DVec3; 3]>, axes: DMat3, echelle: f64, normale: DVec3) -> Vec<Part> {
    let rgb = |c: [u8; 3]| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0];
    if masque {
        return vec![Part { color: [1.0, 1.0, 1.0, 1.0], tris: en_m(face) }];
    }
    let Some(t) = m.types_regions.get(&type_id) else { return vec![Part { color: NOIR, tris: en_m(face) }] };
    let avant = rgb(m.couleurs_categories.get(&t.categorie).copied().unwrap_or([0, 0, 0]));
    let mut parts = Vec::new();
    if m.motifs.get(&t.arriere).is_some_and(|x| x.grilles.is_empty()) {
        parts.push(Part { color: rgb(t.couleur_arriere), tris: en_m(face.clone()) });
    }
    match m.motifs.get(&t.avant) {
        Some(x) if x.grilles.is_empty() => parts.push(Part { color: avant, tris: en_m(face) }),
        Some(x) => {
            let (droite, haut) = ((axes * DVec3::X).normalize_or_zero(), (axes * DVec3::Y).normalize_or_zero());
            let mut tris = Vec::new();
            for [a, b] in vues::hachures(&face, x, echelle, droite, haut, 5_000) {
                vues::ruban(&[a, b], normale, 0.13 * echelle, &mut tris);
            }
            if !tris.is_empty() {
                parts.push(Part { color: avant, tris });
            }
        }
        None => {}
    }
    parts
}

/// Objet de la scène tracé pour une vue : élément Revit, titre, parts.
pub struct Objet2D {
    pub id: i64,
    /// « Cartouche », « Symbole », « Composant de détail » ou « Titre de vue ».
    pub genre: &'static str,
    pub titre: String,
    pub parts: Vec<Part>,
}

/// Instances propres à la vue `v` (cartouches et symboles d'une feuille, composants de
/// détail et annotations d'une vue) et titres de ses fenêtres de vue.
pub fn objets_vue(m: &Maquette, v: &Vue) -> Vec<Objet2D> {
    let normale = if v.direction.length() > 0.5 { v.direction.normalize() } else { DVec3::Z };
    let feuille = v.genre == "Feuille";
    let mut out = Vec::new();
    for i in m.instances_vue.iter().filter(|i: &&InstanceVue| i.vue == v.id && !v.masques.contains(&i.id)) {
        let valeur = |p: i64| if feuille { valeur_cartouche(m, v, i.id, p) } else { texte_param(m, i.id, p) };
        let parts = dessin_place(m, i.type_id, i.id, i.origine, i.axes, normale, v.echelle, &valeur);
        if !parts.is_empty() {
            let famille = m.types.get(&i.type_id).and_then(|t| m.familles.get(&t.famille));
            let titre = m.types.get(&i.type_id).map(|t| format!("{} : {}", famille.map(|f| f.nom.as_str()).unwrap_or("?"), t.nom)).unwrap_or_default();
            // genre d'après la catégorie : un composant de détail est découpé au cadrage
            let genre = match famille.map(|f| f.categorie) {
                Some(OST_DETAIL_COMPONENTS) => "Composant de détail",
                Some(OST_TITLE_BLOCKS) => "Cartouche",
                _ => "Symbole",
            };
            out.push(Objet2D { id: i.id, genre, titre: format!("{titre} [{}]", i.id), parts });
        }
    }
    if feuille {
        for (e, placee) in fenetres(m, v) {
            let Contenu::FenetreDeVue { titre, longueur_titre, type_id, .. } = &e.contenu else { continue };
            let Some(t) = m.types_fenetres.get(type_id).filter(|t| t.titre) else { continue };
            let valeur = |p: i64| valeur_titre(m, e, placee, p);
            let mut parts = dessin_place(m, t.famille_titre, e.id, *titre, DMat3::IDENTITY, normale, 1.0, &valeur);
            // ligne d'extension du titre, de la longueur enregistrée
            if t.ligne && *longueur_titre > 0.0 {
                let mut tris = Vec::new();
                vues::ruban(&[*titre, *titre + DVec3::X * *longueur_titre], normale, 2.0 * TRAIT, &mut tris);
                parts.push(Part { color: NOIR, tris });
            }
            out.push(Objet2D { id: e.id, genre: "Titre de vue", titre: format!("Titre de la vue {} ({}) [{}]", placee.nom, t.nom, e.id), parts });
        }
    }
    out
}

/// Passage du modèle à la feuille d'une fenêtre de vue (décalage et échelle de la
/// fenêtre, vue placée), cadrages et plans de découpe en profondeur.
pub fn fenetre(decalage: DVec3, echelle: f64, v: &Vue) -> NavFenetre {
    let (r, u, d) = (v.droite, v.haut, v.direction.normalize_or(DVec3::Z));
    let o = v.origine / 1000.0;
    // p (m) ↦ décalage / 1000 + échelle · ((p − o)·r, (p − o)·u, (p − o)·d)
    let lin = DMat3::from_cols(DVec3::new(r.x, u.x, d.x), DVec3::new(r.y, u.y, d.y), DVec3::new(r.z, u.z, d.z)) * echelle;
    let t = DVec3::new(decalage.x / 1000.0, decalage.y / 1000.0, 0.0) - lin * o;
    let vers_feuille = DMat4::from_cols(lin.x_axis.extend(0.0), lin.y_axis.extend(0.0), lin.z_axis.extend(0.0), t.extend(1.0));
    // contour de la vue (mm du papier, relatif à son origine) placé sur la feuille (m)
    let rect = |c: [f64; 4]| [(decalage.x + c[0]) / 1000.0, (decalage.y + c[1]) / 1000.0, (decalage.x + c[2]) / 1000.0, (decalage.y + c[3]) / 1000.0];
    let cadre_actif = v.cadre.filter(|c| c.actif[0] && c.actif[1]);
    let cadre = cadre_actif.map(|c| rect(c.contour(v)));
    // cadrage d'annotation : le cadrage élargi de ses décalages (gauche, droite, haut, bas)
    let cadre_annotations = cadre_actif.filter(|c| c.annotation_active).map(|c| {
        let (k, a) = (c.contour(v), c.annotation);
        rect([k[0] - a[0], k[1] - a[3], k[2] + a[1], k[3] + a[2]])
    });
    let mut plans = Vec::new();
    match (v.plage, v.cadre) {
        // plan : entre le bas de la vue et le plan de coupe (cotes absolues)
        (Some((coupe, bas)), _) => {
            plans.push(DVec4::new(0.0, 0.0, -1.0, coupe / 1000.0));
            plans.push(DVec4::new(0.0, 0.0, 1.0, -(bas - 10.0) / 1000.0));
        }
        // coupe, élévation : en arrière du plan de la vue, jusqu'à la limite de vue
        (None, Some(c)) if v.genre == "Coupe" || v.genre == "Élévation" => {
            let a = c.axes[2].normalize_or_zero();
            let o = c.origine / 1000.0;
            plans.push((a).extend(-a.dot(o) - (c.min[2] - 10.0) / 1000.0));
            if c.loin {
                plans.push((-a).extend(a.dot(o) + (c.max[2] + 10.0) / 1000.0));
            }
        }
        _ => {}
    }
    NavFenetre { vue: v.id, vers_feuille, cadre, cadre_annotations, plans, direction: d, masques: v.masques.clone() }
}
