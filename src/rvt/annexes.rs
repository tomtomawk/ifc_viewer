//! Objets annexes du document (LISEZMOI-RVT.md du dossier Decodage RVT, § 7 « Objets
//! annexes ») : groupes, composants de légende, contraintes d'alignement, motifs de
//! ligne, tables d'épaisseurs de ligne, tables de notes et de codes d'assemblage, et
//! objets nommés (cas et natures de charge, gabarits de tableaux électriques, réglages
//! du soleil, substituts de famille, plannings et types d'espace CVC…).
//!
//! Groupe (`ElementGroup`) : cellule de `m_cellList` portant `m_groupId` (le type de
//! groupe) et `m_trf` (repère : origine du groupe), cellule `m_pElemIdsPairSet` (membres :
//! `m_data[].m_elementId`) ; chaque membre porte à son tour une cellule `m_groupId` = le
//! groupe (`groupe_de`).
//!
//! Composant de légende (`LegendComponent`, `m_bIsForTypePreview` 0 ; à 1 : aperçu d'un
//! type, vignette `m_oPreviewImage`) : type affiché `m_componentType`, vue
//! `m_ownerDBViewId`, vue du composant `m_componentViewType` (−8 plan d'étage…),
//! longueur `m_hostLength`, niveau de détail `m_detailLevel`.
//!
//! Contrainte d'alignement (`Alignment`, dérivée de `Dimension`) : éléments contraints
//! `m_witnessRefs[].m_pWitnessRef.m_geomRef.m_elemId`.

use super::champs::{Obj, Record, Val};
use super::geometrie::FT;
use super::medias::{raster, Raster};
use glam::DVec3;

pub struct Groupe {
    pub id: i64,
    pub type_id: i64,
    /// Repère du groupe (`m_trf` de sa cellule : origine en mm, axes en colonnes) ; le
    /// point du groupe est origine + axes · origine d'insertion de son type.
    pub origine: DVec3,
    pub axes: glam::DMat3,
    pub membres: Vec<i64>,
    pub vue: i64,
    pub niveau: i64,
    pub decalage: f64,
}

pub struct ComposantLegende {
    pub id: i64,
    pub vue: i64,
    pub type_id: i64,
    pub vue_composant: i64,
    pub longueur: f64,
    pub detail: i64,
    /// Aperçu d'un type (vignette), et non composant placé dans une légende.
    pub apercu_type: bool,
    pub image: Option<Raster>,
}

pub struct Contrainte {
    pub id: i64,
    pub vue: i64,
    pub references: Vec<i64>,
}

pub struct MotifLigne {
    pub id: i64,
    pub nom: String,
    /// Segments : genre (`m_type` : 0 trait, 1 espace, 2 point), longueur (mm).
    pub segments: Vec<(i64, f64)>,
}

pub struct TableEpaisseurs {
    pub id: i64,
    /// Épaisseurs des plumes 1 à 16 (mm) : modèle par échelle (1:n), perspective,
    /// annotation.
    pub modele: Vec<(i64, Vec<f64>)>,
    pub perspective: Vec<f64>,
    pub annotation: Vec<f64>,
}

pub struct TableCles {
    pub id: i64,
    pub genre: &'static str,
    pub nom: String,
    /// Entrées : clé, clé parente, texte.
    pub entrees: Vec<(String, String, String)>,
}

pub struct ObjetNomme {
    pub id: i64,
    pub genre: &'static str,
    pub nom: String,
    pub props: Vec<(&'static str, String)>,
}

/// Option de variante (`DesignOption` : `m_strName`, jeu `m_DesignOptionSetId`) ou jeu
/// de variantes (`DesignOptionSet` : `m_strName`, options `m_sDesignOptions`, option
/// principale `m_mainDesignOption`). Lecture établie sur le schéma ; aucun fichier du
/// corpus n'a de variante pour la vérifier contre Revit.
pub struct Variante {
    pub id: i64,
    pub nom: String,
    /// Option : son jeu ; jeu : son option principale.
    pub lien: i64,
    pub jeu: bool,
}

/// Liaison d'un paramètre de projet ou partagé à une catégorie (`ParamBinding` :
/// `m_paramId`, `m_categoryId`, `m_elemOrSymbol` : 1 occurrence, 0 type).
pub struct Liaison {
    pub param: i64,
    pub categorie: i64,
    pub occurrence: bool,
}

pub enum Annexe {
    Liaison(Liaison),
    Variante(Variante),
    Groupe(Groupe),
    Legende(ComposantLegende),
    Contrainte(Contrainte),
    Motif(MotifLigne),
    Epaisseurs(TableEpaisseurs),
    Cles(TableCles),
    Nomme(ObjetNomme),
}

/// Classes lues par `lire`.
pub const CLASSES: &[&str] = &[
    "ParamBinding",
    "DesignOption",
    "DesignOptionSet",
    "ElementGroup",
    "LegendComponent",
    "Alignment",
    "ArcAlignment",
    "LinePatternElem",
    "PenWidthTableElem",
    "KeynoteTable",
    "AssemblyCodeTable",
    "LoadCaseElem",
    "LoadNatureElem",
    "PanelScheduleTemplate",
    "FamilySurrogate",
    "FamSymSurrogate",
    "SunAndShadowSettings",
    "TrueNorth",
    "AutoCamSettingsElem",
    "BuildingOperatingYearSchedule",
    "HVACLoadSpaceTypeElem",
    "HVACLoadBuildingTypeElem",
    "HVACLoadScheduleElem",
    "SketchGrid",
    "ExtentElem",
    "DBDrawing",
    "DaylightSourceIdSet",
    "StructSettingsElem",
    "ExportDWGSettings",
    "PropertySetElement",
    "PropertySetLibrary",
    "ControlIconElem",
    "RbsWireSizesElem",
    "RbsPipeSizesElem",
    "PipeSegment",
    "DefaultDivideSettings",
    "BaseLevelTracker",
    "ModelClipBox",
    "CustomElement",
    "Dwg2dExportUserSettingsData",
    "STEPExportSettings",
    "ExportPDFSettings",
    "AreaSettingsElem",
    "CoordinateSystemDisplayElem",
    "SketchGridAppearanceParentElem",
    "RvtLinkInstanceAppearanceParentElem",
    "AutoJoinTracker",
    "FaceSplitter",
];

fn corps<'r, 's>(r: &'r Record<'s>, v: &'r Val<'s>) -> Option<&'r Obj<'s>> {
    r.deref(v).map(|b| &b.obj).or(v.obj())
}

fn id(o: &Obj, k: &str) -> i64 {
    o.get(k).and_then(Val::element_id).unwrap_or(-1)
}

fn texte(o: &Obj, k: &str) -> String {
    o.get(k).and_then(Val::str).unwrap_or_default().to_string()
}

fn reel(o: &Obj, k: &str) -> f64 {
    o.get(k).and_then(Val::real).unwrap_or(0.0)
}

fn entier(o: &Obj, k: &str) -> i64 {
    o.get(k).and_then(Val::int).unwrap_or(0)
}

fn nombre(v: f64) -> String {
    crate::step::format_real((v * 1e6).round() / 1e6)
}

/// Cellules de `Element.m_cellList`.
fn cellules<'r, 's>(r: &'r Record<'s>, o: &'r Obj<'s>) -> Vec<&'r Obj<'s>> {
    o.get("m_cellList").and_then(|v| corps(r, v)).and_then(|c| c.get("m_cells")).map(Val::list).unwrap_or_default().iter().filter_map(|v| corps(r, v)).collect()
}

/// Groupe d'un élément membre (cellule `ElementGroupMembership` de `m_cellList`, champ
/// `m_groupId`), None hors groupe. D'autres cellules portent aussi un `m_groupId`
/// (`SketchMembership` : esquisse, `SecretStyleMembership` : style, `StairsElementMembership`,
/// `Path3dMembership`) : elles ne disent pas l'appartenance à un groupe.
pub fn groupe_de(r: &Record) -> Option<i64> {
    cellules(r, &r.root).iter().filter(|c| c.class_name() == "ElementGroupMembership").find_map(|c| c.get("m_groupId").and_then(Val::element_id)).filter(|&g| g != -1)
}

/// Variante d'un élément (`Element.m_designOptionId`), None dans le modèle principal
/// (−1) ou dans une famille (−4).
pub fn variante_de(r: &Record) -> Option<i64> {
    r.root.get("m_designOptionId").and_then(Val::element_id).filter(|&o| o >= 0)
}

/// Entrées d'une table à clés (`KeyBasedTreeEntryTable.m_oKeyBasedTreeEntries`).
fn entrees(r: &Record, o: &Obj) -> Vec<(String, String, String)> {
    let Some(t) = o.get("m_oKeyBasedTreeEntries").and_then(|v| corps(r, v)) else { return Vec::new() };
    t.get("m_keyBasedTreeEntrySet")
        .map(Val::list)
        .unwrap_or_default()
        .iter()
        .filter_map(|v| corps(r, v))
        .filter_map(|e| e.get("m_oEntry").and_then(|v| corps(r, v)))
        .map(|e| (texte(e, "m_key"), texte(e, "m_parentKey"), texte(e, "m_keynoteText")))
        .collect()
}

fn pens(o: Option<&Obj>) -> Vec<f64> {
    o.and_then(|p| p.get("m_pens")).and_then(Val::reals).unwrap_or_default().iter().map(|x| x * FT).collect()
}

pub fn lire(eid: i64, r: &Record) -> Option<Annexe> {
    let o = &r.root;
    let vue = id(o, "m_ownerDBViewId");
    let nomme = |genre: &'static str, nom: String, props: Vec<(&'static str, String)>| Some(Annexe::Nomme(ObjetNomme { id: eid, genre, nom, props }));
    match o.class_name() {
        "ParamBinding" => Some(Annexe::Liaison(Liaison { param: id(o, "m_paramId"), categorie: id(o, "m_categoryId"), occurrence: entier(o, "m_elemOrSymbol") == 1 })),
        "DesignOption" => Some(Annexe::Variante(Variante { id: eid, nom: texte(o, "m_strName"), lien: id(o, "m_DesignOptionSetId"), jeu: false })),
        "DesignOptionSet" => Some(Annexe::Variante(Variante { id: eid, nom: texte(o, "m_strName"), lien: id(o, "m_mainDesignOption"), jeu: true })),
        "ElementGroup" => {
            let cs = cellules(r, o);
            let cellule = cs.iter().find(|c| c.get("m_groupId").is_some())?;
            let trf = cellule.get("m_trf").and_then(|v| corps(r, v))?;
            let v3 = |v: Option<&Val>| v.and_then(Val::reals).filter(|v| v.len() == 3).map(|v| DVec3::new(v[0], v[1], v[2]));
            let origine = v3(trf.get("m_or"))? * FT;
            let l = trf.get("m_3x3")?.list();
            let axes = glam::DMat3::from_cols(v3(l.first())?, v3(l.get(1))?, v3(l.get(2))?).transpose();
            let membres = cs
                .iter()
                .filter_map(|c| c.get("m_pElemIdsPairSet").and_then(|v| corps(r, v)))
                .flat_map(|p| p.get("m_data").map(Val::list).unwrap_or_default().iter().filter_map(|v| corps(r, v)).filter_map(|d| d.get("m_elementId").and_then(Val::element_id)).collect::<Vec<_>>())
                .collect();
            Some(Annexe::Groupe(Groupe { id: eid, type_id: id(cellule, "m_groupId"), origine, axes, membres, vue, niveau: id(o, "m_assocLevelId"), decalage: reel(o, "m_offsetFromLevel") * FT }))
        }
        "LegendComponent" => Some(Annexe::Legende(ComposantLegende {
            id: eid,
            vue,
            type_id: id(o, "m_componentType"),
            vue_composant: entier(o, "m_componentViewType"),
            longueur: reel(o, "m_hostLength") * FT,
            detail: entier(o, "m_detailLevel"),
            apercu_type: entier(o, "m_bIsForTypePreview") == 1,
            image: o.get("m_oPreviewImage").and_then(|v| corps(r, v)).and_then(raster),
        })),
        "Alignment" | "ArcAlignment" => {
            let references = o
                .get("m_witnessRefs")
                .map(Val::list)
                .unwrap_or_default()
                .iter()
                .filter_map(|v| corps(r, v))
                .filter_map(|w| w.get("m_pWitnessRef").and_then(|v| corps(r, v)))
                .filter_map(|w| w.get("m_geomRef").and_then(|v| corps(r, v)))
                .map(|g| id(g, "m_elemId"))
                .collect();
            Some(Annexe::Contrainte(Contrainte { id: eid, vue, references }))
        }
        "LinePatternElem" => {
            let p = r.field_body(o, "m_pLinePattern")?;
            let segments = p.get("m_segs").map(Val::list).unwrap_or_default().iter().filter_map(|v| corps(r, v)).map(|s| (entier(s, "m_type"), reel(s, "m_data") * FT)).collect();
            // motif importé d'un DWG : « IMPORT:NOM » dans le fichier, « NOM » dans Revit
            let nom = texte(p, "m_name");
            let nom = nom.strip_prefix("IMPORT:").map(str::to_string).unwrap_or(nom);
            Some(Annexe::Motif(MotifLigne { id: eid, nom, segments }))
        }
        "PenWidthTableElem" => {
            let t = r.field_body(o, "m_pPenWidthTable")?;
            let modele = t.get("m_modelPenInfo").map(Val::list).unwrap_or_default().iter().filter_map(|v| corps(r, v)).map(|p| (entier(p, "m_invertedScale"), pens(Some(p)))).collect();
            let sous = |k: &str| t.get(k).and_then(|v| corps(r, v));
            Some(Annexe::Epaisseurs(TableEpaisseurs { id: eid, modele, perspective: pens(sous("m_perspectiveModelPenInfo")), annotation: pens(sous("m_draftPenInfo")) }))
        }
        "KeynoteTable" => Some(Annexe::Cles(TableCles { id: eid, genre: "Table des notes d'identification", nom: texte(o, "m_name"), entrees: entrees(r, o) })),
        "AssemblyCodeTable" => Some(Annexe::Cles(TableCles { id: eid, genre: "Table des codes d'assemblage", nom: String::new(), entrees: entrees(r, o) })),
        "LoadCaseElem" => nomme("Cas de charge", texte(o, "m_name"), vec![("Numéro", entier(o, "m_number").to_string()), ("Nature", id(o, "m_natureId").to_string()), ("Catégorie", id(o, "m_categoryId").to_string())]),
        "LoadNatureElem" => nomme("Nature de charge", texte(o, "m_name"), vec![]),
        "PanelScheduleTemplate" => {
            let sections = o.get("m_oTableData").and_then(|v| corps(r, v)).and_then(|t| t.get("m_SectionData")).map(|s| s.list().len()).unwrap_or(0);
            nomme("Gabarit de tableau électrique", texte(o, "m_name"), vec![("Catégorie", id(o, "m_categoryId").to_string()), ("Sections", sections.to_string())])
        }
        "FamilySurrogate" | "FamSymSurrogate" => {
            let genre = if o.class_name() == "FamilySurrogate" { "Substitut de famille" } else { "Substitut de type" };
            nomme(genre, texte(o, "m_name"), vec![("Catégorie", id(o, "m_categoryId").to_string()), ("Élément", id(o, "m_elemId").to_string())])
        }
        "SunAndShadowSettings" => nomme(
            "Réglage du soleil",
            texte(o, "m_sName"),
            vec![("Azimut", format!("{}°", nombre(reel(o, "m_dAzimuth").to_degrees()))), ("Altitude", format!("{}°", nombre(reel(o, "m_dAltitude").to_degrees()))), ("Genre", entier(o, "m_type").to_string()), ("Vue", vue.to_string())],
        ),
        "TrueNorth" => nomme("Nord géographique", String::new(), vec![("Angle", format!("{}°", nombre(reel(o, "m_angle").to_degrees()))), ("Profondeur de coupe du terrain", format!("{} mm", nombre(reel(o, "m_pocheDepth") * FT)))]),
        "AutoCamSettingsElem" => nomme("Vue d'accueil (ViewCube)", String::new(), vec![("Accueil défini", entier(o, "m_hasSetHome").to_string()), ("Boussole", entier(o, "m_showCompass").to_string())]),
        "BuildingOperatingYearSchedule" => nomme("Planning de fonctionnement", texte(o, "m_scheduleName"), vec![("Jours", o.get("m_daySchedules").map(|v| v.list().len()).unwrap_or(0).to_string())]),
        "HVACLoadSpaceTypeElem" | "HVACLoadBuildingTypeElem" => nomme(
            if o.class_name() == "HVACLoadSpaceTypeElem" { "Type d'espace (charges CVC)" } else { "Type de bâtiment (charges CVC)" },
            texte(o, "m_name"),
            vec![("Densité d'éclairage", nombre(reel(o, "m_LightingLoadDensity"))), ("Densité de puissance", nombre(reel(o, "m_PowerLoadDensity"))), ("Surface par personne", nombre(reel(o, "m_dAreaPerPerson")))],
        ),
        "HVACLoadScheduleElem" => nomme("Planning CVC", texte(o, "m_name"), vec![]),
        "SketchGrid" => nomme("Grille de travail", String::new(), vec![("Vue", id(o, "m_dbviewId").to_string())]),
        "ExtentElem" => nomme("Étendue de vue", String::new(), vec![("Vue", id(o, "m_dbViewId").to_string())]),
        "DBDrawing" => nomme("Dessin", String::new(), vec![("Fenêtres", o.get("m_viewports").map(|v| v.list().len()).unwrap_or(0).to_string())]),
        "ExportDWGSettings" => nomme("Réglage d'export DWG", texte(o, "m_name"), vec![]),
        "ControlIconElem" => nomme("Commande de retournement", String::new(), vec![("Vue", vue.to_string())]),
        c => nomme(libelle(c), String::new(), vec![]),
    }
}

/// Classes reconnues par leur seul nom (libellé), sans champ lu : exclues de la mesure
/// stricte du décodage.
pub const NOMMEES_SEULEMENT: &[&str] = &[
    "DaylightSourceIdSet",
    "StructSettingsElem",
    "PropertySetElement",
    "PropertySetLibrary",
    "RbsWireSizesElem",
    "RbsPipeSizesElem",
    "PipeSegment",
    "DefaultDivideSettings",
    "BaseLevelTracker",
    "ModelClipBox",
    "CustomElement",
    "Dwg2dExportUserSettingsData",
    "STEPExportSettings",
    "ExportPDFSettings",
    "AreaSettingsElem",
    "CoordinateSystemDisplayElem",
    "SketchGridAppearanceParentElem",
    "RvtLinkInstanceAppearanceParentElem",
    "AutoJoinTracker",
    "FaceSplitter",
];

/// Libellé français des réglages lus sans champ particulier.
fn libelle(c: &str) -> &'static str {
    match c {
        "DaylightSourceIdSet" => "Sources de lumière du jour",
        "StructSettingsElem" => "Réglages de structure",
        "PropertySetElement" => "Jeu de propriétés physiques ou thermiques",
        "PropertySetLibrary" => "Bibliothèque de jeux de propriétés",
        "RbsWireSizesElem" => "Tailles de fils",
        "RbsPipeSizesElem" => "Tailles de canalisations",
        "PipeSegment" => "Segment de canalisation",
        "DefaultDivideSettings" => "Réglages de division",
        "BaseLevelTracker" => "Suivi des niveaux de base",
        "ModelClipBox" => "Zone de découpe",
        "CustomElement" => "Élément personnalisé",
        "Dwg2dExportUserSettingsData" => "Réglages d'export DWG 2D",
        "STEPExportSettings" => "Réglage d'export STEP",
        "ExportPDFSettings" => "Réglage d'export PDF",
        "AreaSettingsElem" => "Réglages des surfaces et volumes",
        "CoordinateSystemDisplayElem" => "Affichage du système de coordonnées",
        "SketchGridAppearanceParentElem" => "Apparence des grilles de travail",
        "RvtLinkInstanceAppearanceParentElem" => "Apparence des liens",
        "AutoJoinTracker" => "Suivi des jonctions automatiques",
        "FaceSplitter" => "Division de face",
        _ => "Réglage",
    }
}
