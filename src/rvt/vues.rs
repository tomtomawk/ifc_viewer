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
use super::geometrie::{FT, MotifFace};
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
    /// Type de vue (`m_dbViewTypeId`, un `DBViewType` : « Plan d'étage », « Élévation
    /// 1 »…), −1 s'il n'y en a pas.
    pub type_vue: i64,
    /// Numéro d'une feuille (`m_sheetNumber`).
    pub numero: String,
    /// Cadrage complet de la vue (bornes min et max, actives ou non).
    pub cadre: Option<Cadre>,
    /// Plage d'un plan : cotes absolues (mm) du plan de coupe (`m_cutPlaneElev`) et du
    /// bas de la vue (`m_bottomClipElev`).
    pub plage: Option<(f64, f64)>,
    /// Révisions présentes sur une feuille (`m_explicitlyPresentRevisionIds`).
    pub revisions: Vec<i64>,
    /// Éléments masqués dans la vue (`m_oHiddenElementsViewSettings.m_hiddenElements`).
    pub masques: Vec<i64>,
    /// Catégories masquées dans la vue (`m_oaDrawFilters[0].m_categoryIds` ; contient
    /// toutes celles que donne `View.GetCategoryHidden`, vérifié sur trois vues).
    pub categories_masquees: Vec<i64>,
    /// Titre sur la feuille (`m_viewDescription`), affiché par le titre de la fenêtre de
    /// vue à la place du nom.
    pub titre_feuille: String,
}

/// Cadrage d'une vue (`Viewer.m_boundedSpace`) : origine `m_orig`, axes `m_basis`,
/// bornes de chaque axe (`m_boundOffset[i]` = [max, min], mm), actives si les deux
/// drapeaux de `m_boundActive[i]` valent 1 ; cadrage d'annotation actif
/// (`m_bAnnotationCropActive`) et ses décalages
/// (`m_pAnnotationCropOffsets.m_leftRightTopBottomOffsets`, mm du papier : gauche,
/// droite, haut, bas).
#[derive(Clone, Copy, Debug)]
pub struct Cadre {
    pub origine: DVec3,
    pub axes: [DVec3; 3],
    pub min: [f64; 3],
    pub max: [f64; 3],
    pub actif: [bool; 3],
    /// Limite de vue active (premier drapeau de `m_boundActive[2]` : borne max en
    /// profondeur ; coupes et élévations, toujours découpées au plan de la vue).
    pub loin: bool,
    pub annotation_active: bool,
    pub annotation: [f64; 4],
}

/// Type de fenêtre de vue (`m_attrId` d'un `Viewport`) : nom, titre affiché
/// (`m_showLabel`), type de la famille de titre (`m_familyTagId`), ligne d'extension
/// (`m_showExtensionLine`).
pub struct TypeFenetre {
    pub nom: String,
    pub titre: bool,
    pub famille_titre: i64,
    pub ligne: bool,
}

/// Paramètre affiché par une étiquette (`m_paramDataFormattingArr`) : paramètre,
/// préfixe, suffixe, texte d'exemple, saut de ligne avant.
pub struct ParamEtiquette {
    pub param: i64,
    pub prefixe: String,
    pub suffixe: String,
    pub exemple: String,
    pub a_la_ligne: bool,
}

/// Textes d'un segment de cote (`m_values[0].m_oTextFields`) : valeur remplacée
/// (`m_valueOverride`), préfixe, suffixe.
#[derive(Default, Clone)]
pub struct TexteCote {
    pub remplacement: String,
    pub prefixe: String,
    pub suffixe: String,
}

/// Alignement horizontal d'un texte : le point d'ancrage est à gauche, au milieu ou à
/// droite du cadre.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Alignement {
    Gauche,
    Centre,
    Droite,
}

/// Alignement vertical d'un texte : le point d'ancrage est en haut, au milieu ou en bas
/// du bloc de lignes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Vertical {
    Haut,
    Milieu,
    Bas,
}

/// Alignements d'un texte d'après `TextElement.m_flags` (valeurs de `TextAlignFlags` de
/// l'API : 0x40, 0x80, 0x100 à gauche, au centre, à droite ; 0x200, 0x400, 0x800 en
/// haut, au milieu, en bas ; vérifié sur les étiquettes d'un cartouche de Kerreut contre
/// l'export de la feuille par Revit), à défaut d'après le RTF.
fn alignements(o: &Obj, rtf: Alignement) -> (Alignement, Vertical) {
    let f = o.get("m_flags").and_then(Val::int).unwrap_or(0);
    let h = if f & 0x80 != 0 {
        Alignement::Centre
    } else if f & 0x100 != 0 {
        Alignement::Droite
    } else if f & 0x40 != 0 {
        Alignement::Gauche
    } else {
        rtf
    };
    let v = if f & 0x400 != 0 {
        Vertical::Milieu
    } else if f & 0x800 != 0 {
        Vertical::Bas
    } else {
        Vertical::Haut
    };
    (h, v)
}

pub enum Contenu {
    /// Ligne ou arc de détail (`CurveElem`), polyligne en mm (arc facetté).
    Trait(Vec<DVec3>),
    /// Note de texte : texte, point d'ancrage (`m_orig` : haut du cadre, à gauche, au
    /// milieu ou à droite selon l'alignement), direction du texte et vers le haut,
    /// largeur et hauteur du cadre sur le papier (mm), alignement, type (`m_attrId`).
    Texte { texte: String, point: DVec3, base: DVec3, haut: DVec3, largeur: f64, hauteur: f64, alignement: Alignement, vertical: Vertical, gras: bool, type_id: i64 },
    /// Cote : ligne (origine, direction), valeur et milieu de chaque segment (mm), points
    /// de référence (sur les éléments cotés), type (`m_styleSymbolId`), éléments cotés
    /// (`m_witnessRefs[].m_pWitnessRef.m_geomRef.m_elemId`).
    Cote { origine: DVec3, direction: Option<DVec3>, segments: Vec<(f64, DVec3)>, points: Vec<DVec3>, type_id: i64, references: Vec<i64>, textes: Vec<TexteCote> },
    /// Région remplie : face plane de son `GElement`, type (`m_attrId`, un
    /// `FilledRegionAttributes`).
    Region { type_id: i64 },
    /// Fenêtre de vue d'une feuille : vue placée, décalage sur la feuille (mm), échelle.
    /// Le décalage est la position sur la feuille de l'origine de la vue : un point p du
    /// modèle se place en décalage + échelle · ((p − origine)·droite, (p − origine)·haut).
    /// Titre : origine sur la feuille (`m_labelOrigin`) et longueur de sa ligne
    /// (`m_labelLength`), mm ; type de fenêtre `m_attrId`.
    FenetreDeVue { vue: i64, decalage: DVec3, echelle: f64, titre: DVec3, longueur_titre: f64, type_id: i64 },
    /// Étiquette d'une famille (`TagNote` : cartouche, titre de vue) : paramètres
    /// affichés, cadre comme un texte, type `m_attrId` (taille de la police).
    Etiquette { params: Vec<ParamEtiquette>, point: DVec3, base: DVec3, haut: DVec3, largeur: f64, hauteur: f64, alignement: Alignement, vertical: Vertical, gras: bool, type_id: i64 },
}

pub struct Element2D {
    pub id: i64,
    pub classe: String,
    pub vue: i64,
    pub famille: i64,
    pub contenu: Contenu,
    /// Géométrie calculée (régions remplies).
    pub geometrie: Geometrie,
    /// Paramètre de famille qui commande la visibilité de l'élément (`m_cellList` :
    /// `m_paramDrivenData` de propriété `IS_VISIBLE_PARAM`), −1 sinon.
    pub visible_si: i64,
    /// Région de masquage (`FilledRegion.m_bMasking`) : blanche.
    pub masque: bool,
}

/// Propriété « Visible » d'un élément (BuiltInParameter).
const IS_VISIBLE_PARAM: i64 = -1_006_205;

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
        type_vue: id_of(o, "m_dbViewTypeId"),
        numero: o.get("m_sheetNumber").and_then(Val::str).unwrap_or_default().to_string(),
        cadre: None,
        categories_masquees: o
            .get("m_oaDrawFilters")
            .map(Val::list)
            .unwrap_or_default()
            .first()
            .and_then(|f| r.deref(f))
            .and_then(|f| f.obj.get("m_categoryIds"))
            .map(Val::list)
            .unwrap_or_default()
            .iter()
            .filter_map(Val::element_id)
            .collect(),
        masques: r.field_body(o, "m_oHiddenElementsViewSettings").and_then(|h| h.get("m_hiddenElements")).map(Val::list).unwrap_or_default().iter().filter_map(Val::element_id).collect(),
        titre_feuille: o.get("m_viewDescription").and_then(Val::str).unwrap_or_default().to_string(),
        revisions: o.get("m_explicitlyPresentRevisionIds").map(Val::list).unwrap_or_default().iter().filter_map(Val::element_id).collect(),
        plage: match (o.get("m_cutPlaneElev").and_then(Val::real), o.get("m_bottomClipElev").and_then(Val::real)) {
            (Some(c), Some(b)) if o.class_name() == "DBViewPlan" => Some((c * FT, b * FT)),
            _ => None,
        },
    })
}

/// Cadrage complet d'un `Viewer` (voir [`Cadre`]) et sa vue.
pub fn cadre(r: &Record) -> Option<(i64, Cadre)> {
    let o = &r.root;
    let b = o.get("m_boundedSpace")?.obj()?;
    let axes: Vec<DVec3> = b.get("m_basis")?.list().iter().filter_map(|v| vec3(Some(v))).collect();
    let bornes: Vec<Vec<f64>> = b.get("m_boundOffset")?.list().iter().filter_map(Val::reals).collect();
    let actifs: Vec<Vec<f64>> = b.get("m_boundActive").map(Val::list).unwrap_or_default().iter().filter_map(Val::reals).collect();
    let (&[x, y, z], [bx, by, bz]) = (axes.as_slice(), bornes.as_slice()) else { return None };
    let borne = |v: &[f64], i: usize| v.get(i).copied().unwrap_or(0.0) * FT;
    let actif = |i: usize| actifs.get(i).is_some_and(|a| a.len() == 2 && a.iter().all(|&x| x == 1.0));
    let mut annotation = [0.0; 4];
    if let Some(a) = r.field_body(o, "m_pAnnotationCropOffsets").and_then(|a| a.get("m_leftRightTopBottomOffsets")?.reals()) {
        for (k, v) in a.iter().take(4).enumerate() {
            annotation[k] = v * FT;
        }
    }
    Some((
        id_of(o, "m_dbViewId"),
        Cadre {
            origine: vec3(b.get("m_orig"))? * FT,
            axes: [x, y, z],
            min: [borne(bx, 1), borne(by, 1), borne(bz, 1)],
            max: [borne(bx, 0), borne(by, 0), borne(bz, 0)],
            actif: [actif(0), actif(1), actif(2)],
            loin: actifs.get(2).and_then(|a| a.first()) == Some(&1.0),
            annotation_active: o.get("m_bAnnotationCropActive").and_then(Val::int) == Some(1),
            annotation,
        },
    ))
}

impl Cadre {
    /// Contour (umin, vmin, umax, vmax) du cadrage sur le papier d'une vue (mm, relatif
    /// à l'origine de la vue, axes droite et haut, échelle 1 / `echelle`) : projection
    /// des huit coins.
    pub fn contour(&self, v: &Vue) -> [f64; 4] {
        let mut c = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for &x in &[self.min[0], self.max[0]] {
            for &y in &[self.min[1], self.max[1]] {
                for &z in &[self.min[2], self.max[2]] {
                    let p = self.origine + x * self.axes[0] + y * self.axes[1] + z * self.axes[2] - v.origine;
                    let (u, w) = (p.dot(v.droite) / v.echelle, p.dot(v.haut) / v.echelle);
                    c = [c[0].min(u), c[1].min(w), c[2].max(u), c[3].max(w)];
                }
            }
        }
        c
    }
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

/// Alignement du premier paragraphe d'un RTF (`\qc` centré, `\qr` à droite) et gras
/// (``, pas `0`).
fn style_rtf(rtf: &Obj) -> (Alignement, bool) {
    let b: Vec<u8> = rtf.get("m_arrRtfStream").map(Val::list).unwrap_or_default().iter().filter_map(|v| v.int().map(|c| c as u8)).collect();
    let s = String::from_utf8_lossy(&b);
    // mot de contrôle entier : suivi d'autre chose qu'une lettre ou un chiffre
    let mot = |m: &str| s.match_indices(m).any(|(i, _)| !s[i + m.len()..].starts_with(|c: char| c.is_ascii_alphanumeric()));
    let alignement = if mot(r"\qc") {
        Alignement::Centre
    } else if mot(r"\qr") {
        Alignement::Droite
    } else {
        Alignement::Gauche
    };
    (alignement, mot(r""))
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
            let (rtf, gras) = r.field_body(o, "m_oSavedRtfData").map(style_rtf).unwrap_or((Alignement::Gauche, false));
            let (alignement, vertical) = alignements(o, rtf);
            Contenu::Texte { texte, point: vec3(o.get("m_orig"))? * FT, base: vec3(o.get("m_baseVec"))?, haut: vec3(o.get("m_upVec"))?, largeur: reel("m_lineWidth"), hauteur: reel("m_height"), alignement, vertical, gras, type_id: id_of(o, "m_attrId") }
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
            let references = o
                .get("m_witnessRefs")
                .map(Val::list)
                .unwrap_or_default()
                .iter()
                .filter_map(|w| r.field_body(w.obj()?, "m_pWitnessRef")?.get("m_geomRef")?.obj().map(|g| id_of(g, "m_elemId")))
                .collect();
            let textes = o
                .get("m_ArrSegInfo")
                .map(Val::list)
                .unwrap_or_default()
                .iter()
                .filter(|s| s.obj().and_then(|s| s.get("m_values")?.list().first()?.obj()?.get("m_value")?.real()).is_some())
                .map(|s| {
                    let t = s.obj().and_then(|s| r.field_body(s.get("m_values")?.list().first()?.obj()?, "m_oTextFields"));
                    let champ = |k| t.and_then(|t| t.get(k)?.str()).unwrap_or_default().to_string();
                    TexteCote { remplacement: champ("m_valueOverride"), prefixe: champ("m_prefix"), suffixe: champ("m_suffix") }
                })
                .collect();
            Contenu::Cote { origine: origine.or_else(|| segments.first().map(|s| s.1))?, direction, segments, points, type_id: id_of(o, "m_styleSymbolId"), references, textes }
        }
        "FilledRegion" => Contenu::Region { type_id: id_of(o, "m_attrId") },
        "Viewport" => Contenu::FenetreDeVue {
            vue: id_of(o, "m_dbViewId"),
            decalage: vec3(o.get("m_offset"))? * FT,
            echelle: o.get("m_scale").and_then(Val::real).unwrap_or(1.0),
            titre: vec3(o.get("m_labelOrigin")).unwrap_or_default() * FT,
            longueur_titre: o.get("m_labelLength").and_then(Val::real).unwrap_or(0.0) * FT,
            type_id: id_of(o, "m_attrId"),
        },
        "TagNote" => {
            let params = o
                .get("m_paramDataFormattingArr")
                .map(Val::list)
                .unwrap_or_default()
                .iter()
                .filter_map(|p| {
                    let p = p.obj()?;
                    let s = |k| p.get(k).and_then(Val::str).unwrap_or_default().to_string();
                    Some(ParamEtiquette { param: id_of(p, "m_paramId"), prefixe: s("m_prefix"), suffixe: s("m_suffix"), exemple: s("m_sampleText"), a_la_ligne: p.get("m_bLeadingLineBreak").and_then(Val::int) == Some(1) })
                })
                .collect();
            let reel = |n| o.get(n).and_then(Val::real).unwrap_or(0.0) * FT;
            let (rtf, gras) = r.field_body(o, "m_oSavedRtfData").map(style_rtf).unwrap_or((Alignement::Gauche, false));
            let (alignement, vertical) = alignements(o, rtf);
            Contenu::Etiquette { params, point: vec3(o.get("m_orig"))? * FT, base: vec3(o.get("m_baseVec"))?, haut: vec3(o.get("m_upVec"))?, largeur: reel("m_lineWidth"), hauteur: reel("m_height"), alignement, vertical, gras, type_id: id_of(o, "m_attrId") }
        }
        _ => return None,
    };
    // visibilité commandée par un paramètre de la famille (étiquette d'un titre de vue…)
    let mut visible_si = -1;
    if let Some(cl) = r.field_body(o, "m_cellList") {
        for c in cl.get("m_cells").map(Val::list).unwrap_or_default() {
            let Some(c) = r.deref(c) else { continue };
            for d in c.obj.get("m_paramDrivenData").map(Val::list).unwrap_or_default() {
                let Some(d) = d.obj() else { continue };
                if id_of(d, "m_elemPropId") == IS_VISIBLE_PARAM {
                    visible_si = id_of(d, "m_famParamId");
                }
            }
        }
    }
    Some(Element2D { id, classe: o.class_name().to_string(), vue, famille: id_of(o, "m_famId"), contenu, geometrie: Err(String::new()), visible_si, masque: o.get("m_bMasking").and_then(Val::int) == Some(1) })
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
        Contenu::Cote { origine, direction, segments, points, .. } => {
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
        Contenu::Region { .. } | Contenu::FenetreDeVue { .. } | Contenu::Etiquette { .. } => {}
    }
    out
}

// ------------------------------------------------------------------ Hachures

/// Famille de lignes parallèles d'un motif de remplissage (`FillGrid`), en mm : angle
/// (radians), origine, décalage entre deux lignes (`m_deltas[1]`), glissement le long
/// des lignes d'une ligne à la suivante (`m_deltas[0]`), tirets (longueurs alternées
/// trait / blanc, `m_segs` ; vide : trait continu).
pub struct GrilleMotif {
    pub angle: f64,
    pub origine: [f64; 2],
    pub glissement: f64,
    pub decalage: f64,
    pub tirets: Vec<f64>,
}

/// Motif de remplissage (`FillPatternElem` → `FillPattern`) ; sans grille : uni.
pub struct Motif {
    pub nom: String,
    /// Motif de modèle (`m_fpTarget` = 1, mesures en mm du modèle), sinon de dessin (mm
    /// du papier, à multiplier par l'échelle de la vue).
    pub modele: bool,
    pub grilles: Vec<GrilleMotif>,
}

/// Type de région remplie (`FilledRegionAttributes`) : motifs de premier plan et
/// d'arrière-plan ; la couleur de premier plan est celle de sa sous-catégorie
/// (`m_lineAndTextAttr.m_categoryId`, couleur du `GStyle`).
pub struct TypeRegion {
    pub nom: String,
    pub avant: i64,
    pub arriere: i64,
    pub couleur_arriere: [u8; 3],
    pub categorie: i64,
}

pub fn motif(r: &Record) -> Option<Motif> {
    let fp = r.field_body(&r.root, "m_pFillPattern")?;
    let mut grilles = Vec::new();
    for g in fp.get("m_gridsArr").map(Val::list).unwrap_or_default() {
        let Some(g) = r.deref(g).map(|b| &b.obj) else { continue };
        let o = g.get("m_origin")?.reals()?;
        let d = g.get("m_deltas")?.reals()?;
        grilles.push(GrilleMotif {
            angle: g.get("m_angle")?.real()?,
            origine: [*o.first()? * FT, *o.get(1)? * FT],
            glissement: *d.first()? * FT,
            decalage: *d.get(1)? * FT,
            tirets: g.get("m_segs").and_then(Val::reals).unwrap_or_default().iter().map(|s| s.abs() * FT).collect(),
        });
    }
    Some(Motif { nom: fp.get("m_name").and_then(Val::str).unwrap_or_default().to_string(), modele: fp.get("m_fpTarget").and_then(Val::int) == Some(1), grilles })
}

/// Type de cote (`DimensionStyle`), longueurs en mm du papier : format des valeurs
/// (`m_oLinFormatOptions` : unité, précision ; celui du projet si `m_bUseDefault`), texte
/// en gras et sous-catégorie (couleur) de `m_pLineAndTextAttr`, lignes d'attache de
/// longueur fixe (`m_dimWitnCntrlExtBelow` = 1, `DIM_WITNS_LINE_CNTRL`) ou jusqu'à
/// l'élément, paramètres `DIM_WITNS_LINE_EXTENSION_BELOW` (longueur fixe),
/// `WITNS_LINE_EXTENSION` (au-delà de la ligne de cote), `WITNS_LINE_GAP_TO_ELT`,
/// `DIM_LINE_EXTENSION`, `TEXT_DIST_TO_LINE`.
pub struct TypeCote {
    pub format: Option<(String, f64)>,
    pub gras: bool,
    pub categorie: i64,
    pub attache_fixe: bool,
    pub longueur_attache: f64,
    pub extension_attache: f64,
    pub ecart_element: f64,
    pub extension_ligne: f64,
    pub decalage_texte: f64,
}

/// Format (unité, précision) d'un `FormatOptions`, None s'il suit le projet.
pub fn format(f: &Obj) -> Option<(String, f64)> {
    if f.get("m_bUseDefault").and_then(Val::int) == Some(1) {
        return None;
    }
    let u = f.get("m_unitTypeId")?.obj()?.get("m_typeId")?.str()?.to_string();
    Some((u, f.get("m_accuracy").and_then(Val::real).unwrap_or(1.0)))
}

pub fn type_cote(r: &Record) -> Option<TypeCote> {
    let o = &r.root;
    let mut p: std::collections::HashMap<i64, f64> = Default::default();
    for x in r.field_body(o, "m_pParamValueSetDouble").and_then(|s| s.get("m_paramSet")).map(Val::list).unwrap_or_default() {
        if let Some(x) = x.obj() {
            p.insert(id_of(x, "m_paramId"), x.get("m_value").and_then(Val::real).unwrap_or(0.0) * FT);
        }
    }
    let attr = r.field_body(o, "m_pLineAndTextAttr");
    Some(TypeCote {
        format: r.field_body(o, "m_oLinFormatOptions").and_then(format),
        gras: attr.and_then(|a| a.get("m_bBold")?.int()) == Some(1),
        categorie: attr.map(|a| id_of(a, "m_categoryId")).unwrap_or(-1),
        attache_fixe: o.get("m_dimWitnCntrlExtBelow").and_then(Val::int) == Some(1),
        longueur_attache: p.get(&-1_006_433).copied().unwrap_or(0.0),
        extension_attache: p.get(&-1_006_404).copied().unwrap_or(0.0),
        ecart_element: p.get(&-1_006_405).copied().unwrap_or(0.0),
        extension_ligne: p.get(&-1_006_431).copied().unwrap_or(0.0),
        decalage_texte: p.get(&-1_006_401).copied().unwrap_or(0.0),
    })
}

/// Format des longueurs du projet (`UnitsElem.m_units.m_formatOptionsMap`, entrée
/// `autodesk.spec.aec:length`).
pub fn format_projet(r: &Record) -> Option<(String, f64)> {
    let u = r.root.get("m_units")?.obj()?;
    for e in u.get("m_formatOptionsMap")?.list() {
        let e = e.obj()?;
        let spec = e.get("first").and_then(Val::obj).and_then(|f| f.get("m_typeId")?.str()).unwrap_or_default();
        if spec.starts_with("autodesk.spec.aec:length-") {
            return format(e.get("second")?.obj()?);
        }
    }
    None
}

/// Texte d'une longueur (mm) selon un format (unité, précision) ; mètres au centimètre
/// près à défaut.
pub fn texte_longueur(v: f64, format: Option<&(String, f64)>) -> String {
    let (unite, precision) = format.map(|(u, a)| (u.as_str(), *a)).unwrap_or(("meters", 0.01));
    let unite = unite.rsplit(':').next().unwrap_or("").split('-').next().unwrap_or("");
    let x = match unite {
        "millimeters" => v,
        "centimeters" => v / 10.0,
        "decimeters" => v / 100.0,
        _ => v / 1000.0,
    };
    let precision = if precision > 0.0 { precision } else { 1.0 };
    let arrondi = (x / precision).round() * precision;
    let decimales = (-precision.log10()).ceil().max(0.0) as usize;
    format!("{arrondi:.decimales$}")
}

pub fn type_region(r: &Record) -> Option<TypeRegion> {
    let o = &r.root;
    let c = o.get("m_backgroundPatternColor").and_then(Val::int).unwrap_or(0xFFFFFF);
    Some(TypeRegion {
        nom: r.field_body(o, "m_symbolInfo")?.get("m_name")?.str()?.to_string(),
        avant: id_of(o, "m_fillPatternElemId"),
        arriere: id_of(o, "m_backgroundPatternElemId"),
        couleur_arriere: [c as u8, (c >> 8) as u8, (c >> 16) as u8],
        categorie: o.get("m_lineAndTextAttr").and_then(Val::obj).map(|a| id_of(a, "m_categoryId")).unwrap_or(-1),
    })
}

/// Intervalles de t où p + t·d est dans le triangle (a, b, c) du plan (u, v).
fn dans_triangle(p: [f64; 2], d: [f64; 2], t: [[f64; 2]; 3]) -> Option<(f64, f64)> {
    let aire = (t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[1][1] - t[0][1]) * (t[2][0] - t[0][0]);
    if aire.abs() < 1e-12 {
        return None;
    }
    let (mut t0, mut t1) = (f64::MIN, f64::MAX);
    for i in 0..3 {
        let (a, b) = (t[i], t[(i + 1) % 3]);
        // demi-plan intérieur : signe(aire) × ((b − a) × (x − a)) ≥ 0
        let (ex, ey) = (b[0] - a[0], b[1] - a[1]);
        let f0 = aire.signum() * (ex * (p[1] - a[1]) - ey * (p[0] - a[0]));
        let f1 = aire.signum() * (ex * d[1] - ey * d[0]);
        if f1.abs() < 1e-15 {
            if f0 < 0.0 {
                return None;
            }
        } else {
            let t = -f0 / f1;
            if f1 > 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t0 < t1).then_some((t0, t1))
}

/// Traits d'un motif de hachures découpés sur une région (triangles en mm), dans le
/// repère (droite, haut) de la vue ; motif de dessin mis à l'échelle de la vue.
/// Segments en mm, au plus `max` par région.
pub fn hachures(triangles: &[[DVec3; 3]], m: &Motif, echelle: f64, droite: DVec3, haut: DVec3, max: usize) -> Vec<[DVec3; 2]> {
    let Some(p0) = triangles.first().map(|t| t[0]) else { return Vec::new() };
    let n = droite.cross(haut);
    let w = p0.dot(n);
    let uv = |p: DVec3| [p.dot(droite), p.dot(haut)];
    let tris: Vec<[[f64; 2]; 3]> = triangles.iter().map(|t| t.map(uv)).collect();
    let f = if m.modele { 1.0 } else { echelle };
    let pt = |q: [f64; 2]| q[0] * droite + q[1] * haut + w * n;
    hachures_plan(&tris, m, f, max).into_iter().map(|[a, b]| [pt(a), pt(b)]).collect()
}

/// Traits d'un motif de modèle sur une face plane (triangles en mm), calé comme dans
/// Revit par `cal` (point (x, y) du motif en origine + x·u + y·v) et décalés de `ecart`
/// mm vers l'extérieur de la face ; au plus `max` traits, en mètres.
pub fn motif_sur_face(tris: &[[DVec3; 3]], cal: &MotifFace, m: &Motif, ecart: f64, max: usize) -> Vec<[DVec3; 2]> {
    let Some(t0) = tris.first() else { return Vec::new() };
    let n = (t0[1] - t0[0]).cross(t0[2] - t0[0]).normalize_or_zero();
    let (uu, uv, vv) = (cal.u.dot(cal.u), cal.u.dot(cal.v), cal.v.dot(cal.v));
    let det = uu * vv - uv * uv;
    if det.abs() < 1e-12 || n == DVec3::ZERO {
        return Vec::new();
    }
    // coordonnées du motif d'un point de la face (moindres carrés dans le plan de u, v)
    let plan = |p: DVec3| {
        let r = p - cal.origine;
        let (a, b) = (r.dot(cal.u), r.dot(cal.v));
        [(vv * a - uv * b) / det, (uu * b - uv * a) / det]
    };
    let t2: Vec<[[f64; 2]; 3]> = tris.iter().map(|t| t.map(plan)).collect();
    let pt = |q: [f64; 2]| (cal.origine + q[0] * cal.u + q[1] * cal.v + n * ecart) / 1000.0;
    hachures_plan(&t2, m, 1.0, max).into_iter().map(|[a, b]| [pt(a), pt(b)]).collect()
}

/// Cœur de [`hachures`] dans le plan du motif : triangles en (x, y), mesures du motif
/// multipliées par `f` ; segments en (x, y).
pub fn hachures_plan(tris: &[[[f64; 2]; 3]], m: &Motif, f: f64, max: usize) -> Vec<[[f64; 2]; 2]> {
    let mut out = Vec::new();
    for g in &m.grilles {
        let (dec, gl) = (g.decalage * f, g.glissement * f);
        if dec.abs() < 1e-6 {
            continue;
        }
        let (d, nr) = ([g.angle.cos(), g.angle.sin()], [-g.angle.sin(), g.angle.cos()]);
        let o = [g.origine[0] * f, g.origine[1] * f];
        let s: Vec<f64> = tris.iter().flatten().map(|q| (q[0] - o[0]) * nr[0] + (q[1] - o[1]) * nr[1]).collect();
        let (smin, smax) = s.iter().fold((f64::MAX, f64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
        let (k0, k1) = ((smin / dec).floor() as i64, (smax / dec).ceil() as i64);
        if k1 < k0 || (k1 - k0) as usize > max {
            continue;
        }
        let tirets: Vec<f64> = g.tirets.iter().map(|x| x * f).collect();
        let periode: f64 = tirets.iter().sum();
        for k in k0..=k1 {
            let p = [o[0] + k as f64 * (dec * nr[0] + gl * d[0]), o[1] + k as f64 * (dec * nr[1] + gl * d[1])];
            let mut iv: Vec<(f64, f64)> = tris.iter().filter_map(|t| dans_triangle(p, d, *t)).collect();
            iv.sort_by(|a, b| a.0.total_cmp(&b.0));
            // réunion des intervalles des triangles voisins
            let mut union: Vec<(f64, f64)> = Vec::new();
            for (a, b) in iv {
                match union.last_mut() {
                    Some(l) if a <= l.1 + 1e-6 => l.1 = l.1.max(b),
                    _ => union.push((a, b)),
                }
            }
            for (a, b) in union {
                let mut morceaux = Vec::new();
                if periode > 1e-6 {
                    // tirets : phase 0 au point p de la ligne
                    let mut m0 = (a / periode).floor() * periode;
                    while m0 < b && morceaux.len() < max {
                        let mut x = m0;
                        for (i, l) in tirets.iter().enumerate() {
                            if i % 2 == 0 && x + l > a && x < b {
                                morceaux.push((x.max(a), (x + l).min(b)));
                            }
                            x += l;
                        }
                        m0 += periode;
                    }
                } else {
                    morceaux.push((a, b));
                }
                for (a, b) in morceaux {
                    let pt = |t: f64| [p[0] + t * d[0], p[1] + t * d[1]];
                    out.push([pt(a), pt(b)]);
                    if out.len() >= max {
                        return out;
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn carre() -> Vec<[DVec3; 3]> {
        let p = |x, y| DVec3::new(x, y, 0.0);
        vec![[p(0.0, 0.0), p(1000.0, 0.0), p(1000.0, 1000.0)], [p(0.0, 0.0), p(1000.0, 1000.0), p(0.0, 1000.0)]]
    }

    fn longueur(s: &[[DVec3; 2]]) -> f64 {
        s.iter().map(|[a, b]| a.distance(*b)).sum()
    }

    /// Hachures découpées sur un carré de 1 m : longueur totale = aire / espacement.
    #[test]
    fn hachures_carre() {
        let g = |angle: f64, tirets: Vec<f64>| GrilleMotif { angle: angle.to_radians(), origine: [0.0, 0.0], glissement: 0.0, decalage: 3.0, tirets };
        // motif de dessin de 3 mm à 1:100 : 300 mm entre les traits
        let diag = Motif { nom: "Diagonale haut".into(), modele: false, grilles: vec![g(45.0, vec![])] };
        let l = longueur(&hachures(&carre(), &diag, 100.0, DVec3::X, DVec3::Y, 10_000));
        assert!((l - 1e6 / 300.0).abs() < 300.0 * 2f64.sqrt(), "{l}");
        // traits horizontaux sur les lignes 0, 300, 600, 900 : 4 × 1 000 mm
        let horiz = Motif { nom: "h".into(), modele: false, grilles: vec![g(0.0, vec![])] };
        let s = hachures(&carre(), &horiz, 100.0, DVec3::X, DVec3::Y, 10_000);
        assert!((longueur(&s) - 4000.0).abs() < 1e-6, "{}", longueur(&s));
        // tirets de 1 mm et blancs de 1 mm (100 mm à 1:100) : la moitié
        let tir = Motif { nom: "t".into(), modele: false, grilles: vec![g(0.0, vec![1.0, 1.0])] };
        assert!((longueur(&hachures(&carre(), &tir, 100.0, DVec3::X, DVec3::Y, 10_000)) - 2000.0).abs() < 1e-6);
        // motif de modèle : pas d'échelle (3 mm entre les traits)
        let modele = Motif { nom: "m".into(), modele: true, grilles: vec![g(90.0, vec![])] };
        assert_eq!(hachures(&carre(), &modele, 100.0, DVec3::X, DVec3::Y, 100_000).len(), 334);
    }

    #[test]
    fn rtf_simple() {
        // RTF d'une note de texte de FEN_2PF.rfa (fins de ligne omises)
        let rtf = br"{\rtf1\ansi\ansicpg1252\deff0\nouicompat\deflang1033{\fonttbl{\f0\fnil\fcharset0 Arial;}}{\colortbl ;\red0\green0\blue0;}{\*\generator Riched20 10.0.18362}\viewkind4\uc1 \pard\cf1\fs48 Ext\'e9rieur\par}";
        assert_eq!(texte_rtf(rtf), "Extérieur");
    }
}
