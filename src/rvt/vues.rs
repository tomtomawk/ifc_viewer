//! Vues et éléments 2D propres à une vue (portage de `tools/rvt-research/vues.py` ;
//! docs/LISEZMOI-RVT.md § 7 « Vues et éléments 2D »).
//!
//! Une vue est un objet dérivé de `DBView` : nom `m_viewName`, échelle 1 / `m_scale`,
//! origine `m_origin`, repère `m_horzDir` (droite), `m_vertDir` (haut), `m_viewDir`
//! (vers l'observateur) ; le cadrage d'une coupe, d'une élévation ou d'une vue 3D est
//! dans son `Viewer` (`m_boundedSpace`). Un élément 2D a `Element.m_ownerDBViewId` ≠ −1 ;
//! ses coordonnées sont celles du modèle (plans, coupes) ou, pour une vue de dessin et
//! une feuille, celles de la vue (mm du papier pour une feuille).

use super::champs::{Obj, Record, Val};
use super::geometrie::FT;
use super::maquette::Geometrie;
use glam::DVec3;

pub struct Vue {
    pub id: i64,
    pub classe: String,
    pub nom: String,
    /// Genre lisible (« Plan d'étage », « Coupe », « Feuille »…).
    pub genre: &'static str,
    /// Dénominateur de l'échelle (100 pour 1:100), 1 pour une feuille.
    pub echelle: f64,
    /// Niveau d'un plan (`m_genElemId`), −1 sinon.
    pub niveau: i64,
    pub origine: DVec3,
    pub droite: DVec3,
    pub haut: DVec3,
    pub direction: DVec3,
    pub gabarit: bool,
    /// Famille propriétaire (`Element.m_famId`), pour écarter les vues des familles
    /// imbriquées rangées en ligne dans un `.rfa`.
    pub famille: i64,
    /// Cadrage (`Viewer.m_boundedSpace`) : origine, axes, longueur sur chaque axe (mm).
    pub cadrage: Option<(DVec3, [DVec3; 3], [f64; 3])>,
}

pub enum Contenu {
    /// Ligne ou arc de détail (`CurveElem`), polyligne en mm (arc facetté).
    Trait(Vec<DVec3>),
    /// Note de texte : texte, point d'ancrage, direction du texte et vers le haut,
    /// largeur et hauteur sur le papier (mm).
    Texte { texte: String, point: DVec3, base: DVec3, haut: DVec3, largeur: f64, hauteur: f64 },
    /// Cote : ligne (origine, direction), valeur et milieu de chaque segment (mm), points
    /// de référence.
    Cote { origine: DVec3, direction: Option<DVec3>, segments: Vec<(f64, DVec3)>, points: Vec<DVec3> },
    /// Région remplie : face plane de son `GElement`.
    Region,
    /// Fenêtre de vue d'une feuille : vue placée, décalage sur la feuille (mm), échelle.
    FenetreDeVue { vue: i64, decalage: DVec3, echelle: f64 },
}

pub struct Element2D {
    pub id: i64,
    pub classe: String,
    pub vue: i64,
    pub famille: i64,
    pub contenu: Contenu,
    /// Géométrie calculée (régions remplies).
    pub geometrie: Geometrie,
}

fn id_of(o: &Obj, name: &str) -> i64 {
    o.get(name).and_then(Val::element_id).unwrap_or(-1)
}

fn vec3(v: Option<&Val>) -> Option<DVec3> {
    match v?.reals().as_deref() {
        Some(&[x, y, z]) => Some(DVec3::new(x, y, z)),
        _ => None,
    }
}

/// Genre d'une vue d'après sa classe (et `m_planViewType`, `m_sectionViewType`).
fn genre(o: &Obj) -> &'static str {
    let int = |n| o.get(n).and_then(Val::int);
    match o.class_name() {
        "DBViewPlan" if int("m_planViewType") == Some(2) => "Plan de plafond",
        "DBViewPlan" => "Plan d'étage",
        "DBViewSection" if int("m_sectionViewType") == Some(0) => "Coupe",
        "DBViewSection" => "Élévation",
        "DBView3d" => "Vue 3D",
        "DBViewAnonDrafting" => "Vue de dessin",
        // les feuilles sont des DBViewDrafting (nom historique de la classe)
        "DBViewDrafting" => "Feuille",
        "DBViewSchedule" | "TableView" => "Nomenclature",
        "DBViewLegend" => "Légende",
        "DBViewProject" | "RbsDbViewSystemNavigator" => "Arborescence",
        _ => "Autre vue",
    }
}

pub fn vue(id: i64, r: &Record) -> Option<Vue> {
    let o = &r.root;
    let scale = o.get("m_scale").and_then(Val::real).filter(|&s| s > 0.0).unwrap_or(1.0);
    Some(Vue {
        id,
        classe: o.class_name().to_string(),
        nom: o.get("m_viewName").and_then(Val::str).unwrap_or_default().to_string(),
        genre: genre(o),
        echelle: (1.0 / scale).round(),
        niveau: if o.class_name() == "DBViewPlan" { id_of(o, "m_genElemId") } else { -1 },
        origine: vec3(o.get("m_origin"))? * FT,
        droite: vec3(o.get("m_horzDir"))?,
        haut: vec3(o.get("m_vertDir"))?,
        direction: vec3(o.get("m_viewDir"))?,
        gabarit: o.get("m_isTemplate").and_then(Val::int) == Some(1),
        famille: id_of(o, "m_famId"),
        cadrage: None,
    })
}

/// (vue, origine, axes, longueurs) du cadrage d'un `Viewer`.
pub fn cadrage(r: &Record) -> Option<(i64, DVec3, [DVec3; 3], [f64; 3])> {
    let o = &r.root;
    let b = o.get("m_boundedSpace")?.obj()?;
    let axes: Vec<DVec3> = b.get("m_basis")?.list().iter().filter_map(|v| vec3(Some(v))).collect();
    let bornes: Vec<f64> = b.get("m_boundOffset")?.list().iter().filter_map(|v| v.reals()?.first().copied()).collect();
    let (&[x, y, z], &[bx, by, bz]) = (axes.as_slice(), bornes.as_slice()) else { return None };
    Some((id_of(o, "m_dbViewId"), vec3(b.get("m_orig"))? * FT, [x, y, z], [bx * FT, by * FT, bz * FT]))
}

/// Points d'une `GLine` ou d'un `GArc` (arc facetté tous les 5° au plus), en mm.
fn polyligne(c: &Obj) -> Option<Vec<DVec3>> {
    let ends = c.get("m_endParams")?.reals()?;
    let (t0, t1) = (*ends.first()?, *ends.get(1)?);
    match c.class_name() {
        "GLine" => {
            let (o, d) = (vec3(c.get("m_origin"))?, vec3(c.get("m_dirVec"))?);
            Some(vec![(o + t0 * d) * FT, (o + t1 * d) * FT])
        }
        "GArc" => {
            let (ce, x, y) = (vec3(c.get("m_center"))?, vec3(c.get("m_xVec"))?, vec3(c.get("m_yVec"))?);
            let r = c.get("m_radius")?.real()?;
            let n = (((t1 - t0).abs() / 5f64.to_radians()).ceil() as usize).max(2);
            Some((0..=n).map(|k| t0 + (t1 - t0) * k as f64 / n as f64).map(|t| (ce + r * (t.cos() * x + t.sin() * y)) * FT).collect())
        }
        _ => None,
    }
}

/// Texte visible d'un RTF simple : `\'xx` en Windows-1252, `\par` en saut de ligne,
/// groupes d'en-tête (`{\fonttbl…}`, `{\colortbl…}`, `{\*…}`) et mots de contrôle retirés.
pub fn texte_rtf(b: &[u8]) -> String {
    let s: Vec<char> = b.iter().take_while(|&&c| c != 0).map(|&c| c as char).collect();
    // profondeur des accolades, et profondeur du groupe d'en-tête en cours (ignoré)
    let (mut out, mut i, mut profondeur) = (String::new(), 0, 0usize);
    let mut depth_ignore = None;
    while i < s.len() {
        match s[i] {
            '{' => {
                profondeur += 1;
                let ent: String = s[i + 1..(i + 10).min(s.len())].iter().collect();
                if depth_ignore.is_none() && (ent.starts_with("\\fonttbl") || ent.starts_with("\\colortbl") || ent.starts_with("\\*")) {
                    depth_ignore = Some(profondeur);
                }
                i += 1;
            }
            '}' => {
                if depth_ignore == Some(profondeur) {
                    depth_ignore = None;
                }
                profondeur = profondeur.saturating_sub(1);
                i += 1;
            }
            '\\' => {
                let mut j = i + 1;
                if s.get(j) == Some(&'\'') {
                    let hex: String = s.get(j + 1..j + 3).map(|h| h.iter().collect()).unwrap_or_default();
                    if depth_ignore.is_none() {
                        if let Ok(v) = u8::from_str_radix(&hex, 16) {
                            out.push(super::cp1252(v));
                        }
                    }
                    i = j + 3;
                    continue;
                }
                while j < s.len() && s[j].is_ascii_alphabetic() {
                    j += 1;
                }
                let mot: String = s[i + 1..j].iter().collect();
                while j < s.len() && (s[j].is_ascii_digit() || s[j] == '-') {
                    j += 1;
                }
                if s.get(j) == Some(&' ') {
                    j += 1;
                }
                if depth_ignore.is_none() && mot == "par" {
                    out.push('\n');
                }
                i = j.max(i + 2);
            }
            '\r' | '\n' => i += 1,
            c => {
                if depth_ignore.is_none() {
                    out.push(c);
                }
                i += 1;
            }
        }
    }
    out.trim().to_string()
}

/// Élément 2D d'une vue, None pour une classe non retenue ou une ligne d'esquisse sans
/// courbe.
pub fn element(id: i64, r: &Record) -> Option<Element2D> {
    let o = &r.root;
    let vue = id_of(o, "m_ownerDBViewId");
    if vue == -1 {
        return None;
    }
    let contenu = match o.class_name() {
        "CurveElem" => Contenu::Trait(polyligne(r.field_body(r.field_body(o, "m_pCurveDriver")?, "m_pCrv")?)?),
        "TextNote" => {
            let frags = o.get("m_arrTextFragments").map(Val::list).unwrap_or_default();
            let mut texte: String = frags.iter().filter_map(|f| f.obj()?.get("m_text")?.str()).collect();
            if texte.is_empty() {
                if let Some(rtf) = r.field_body(o, "m_oSavedRtfData") {
                    let b: Vec<u8> = rtf.get("m_arrRtfStream").map(Val::list).unwrap_or_default().iter().filter_map(|v| v.int().map(|c| c as u8)).collect();
                    texte = texte_rtf(&b);
                }
            }
            let reel = |n| o.get(n).and_then(Val::real).unwrap_or(0.0) * FT;
            Contenu::Texte { texte, point: vec3(o.get("m_orig"))? * FT, base: vec3(o.get("m_baseVec"))?, haut: vec3(o.get("m_upVec"))?, largeur: reel("m_lineWidth"), hauteur: reel("m_height") }
        }
        "LinearDimString" | "RadialDim" | "AngularDim" | "ArcLengthDim" => {
            let ligne = r.field_body(o, "m_pDimLine");
            let origine = ligne.and_then(|l| vec3(l.get("m_origin"))).map(|p| p * FT);
            let direction = ligne.filter(|l| l.class_name() == "GLine").and_then(|l| vec3(l.get("m_dirVec")));
            let segments = o
                .get("m_ArrSegInfo")
                .map(Val::list)
                .unwrap_or_default()
                .iter()
                .filter_map(|s| {
                    let s = s.obj()?;
                    let v = s.get("m_values")?.list().first()?.obj()?.get("m_value")?.real()?;
                    Some((v * FT, vec3(s.get("m_origin"))? * FT))
                })
                .collect::<Vec<_>>();
            let points = o.get("m_refPnts").map(Val::list).unwrap_or_default().iter().filter_map(|p| vec3(Some(p))).map(|p| p * FT).collect();
            Contenu::Cote { origine: origine.or_else(|| segments.first().map(|s| s.1))?, direction, segments, points }
        }
        "FilledRegion" => Contenu::Region,
        "Viewport" => Contenu::FenetreDeVue { vue: id_of(o, "m_dbViewId"), decalage: vec3(o.get("m_offset"))? * FT, echelle: o.get("m_scale").and_then(Val::real).unwrap_or(1.0) },
        _ => return None,
    };
    Some(Element2D { id, classe: o.class_name().to_string(), vue, famille: id_of(o, "m_famId"), contenu, geometrie: Err(String::new()) })
}

// ------------------------------------------------------------------ Tracé

/// Ruban de largeur `l` (mm) le long d'une polyligne, dans le plan de normale `n` ;
/// triangles en mètres.
pub fn ruban(points: &[DVec3], n: DVec3, l: f64, out: &mut Vec<[DVec3; 3]>) {
    for w in points.windows(2) {
        let d = w[1] - w[0];
        let cote = n.cross(d);
        if cote.length() < 1e-9 || d.length() < 1e-9 {
            continue;
        }
        let c = cote.normalize() * (l / 2.0);
        let [a, b, e, f] = [w[0] - c, w[1] - c, w[1] + c, w[0] + c].map(|p| p / 1000.0);
        out.push([a, b, e]);
        out.push([a, e, f]);
    }
}

/// Quadrilatère plein (mm → m).
pub fn quad(p: [DVec3; 4], out: &mut Vec<[DVec3; 3]>) {
    let [a, b, c, d] = p.map(|p| p / 1000.0);
    out.push([a, b, c]);
    out.push([a, c, d]);
}

/// Triangles d'un élément 2D (hors régions, maillées à part), pour une vue de normale
/// `n` à l'échelle `echelle` : traits de 0,25 mm sur le papier, cadre du texte, ligne et
/// repères de cote.
pub fn trace(e: &Element2D, n: DVec3, echelle: f64) -> Vec<[DVec3; 3]> {
    let l = 0.25 * echelle;
    let mut out = Vec::new();
    match &e.contenu {
        Contenu::Trait(p) => ruban(p, n, l, &mut out),
        Contenu::Texte { point, base, haut, largeur, hauteur, .. } => {
            let (w, h) = (*base * largeur * echelle, *haut * hauteur * echelle);
            quad([*point, *point + w, *point + w - h, *point - h], &mut out);
        }
        Contenu::Cote { origine, direction, segments, points } => {
            if let Some(d) = direction {
                // étendue : projection des points de référence (ou des milieux) sur la ligne
                let ts: Vec<f64> = points.iter().chain(segments.iter().map(|s| &s.1)).map(|p| (*p - *origine).dot(*d)).collect();
                let (t0, t1) = ts.iter().fold((f64::MAX, f64::MIN), |(a, b), &t| (a.min(t), b.max(t)));
                if t0 <= t1 {
                    ruban(&[*origine + t0 * *d, *origine + t1 * *d], n, l, &mut out);
                    // repères obliques à 45° sur chaque point de référence
                    let oblique = (*d + n.cross(*d)).normalize() * (1.5 * echelle);
                    for p in points {
                        let q = *origine + (*p - *origine).dot(*d) * *d;
                        ruban(&[q - oblique, q + oblique], n, l, &mut out);
                    }
                }
            }
        }
        Contenu::Region | Contenu::FenetreDeVue { .. } => {}
    }
    out
}
