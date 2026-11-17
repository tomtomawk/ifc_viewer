//! Export IFC4 (ReferenceView, géométrie tessellée, mm) : site, niveaux, classes, types,
//! jeux de propriétés et quantités, matériaux à couches et couleurs.
mod materiaux;
mod proprietes;
mod quantites;
mod site;
mod step;
mod styles;
#[cfg(test)]
mod tests;
mod types;

use modele::*;
use modele::{elements::pente, murs::jonctions, volumes::solides};
use std::collections::HashMap;
use step::{guid, ref_, refs, texte, Step};

/// Export IFC4 du projet ; l'en-tête nomme le fichier `modele.ifc`.
pub fn exporter(p: &Projet) -> Result<String, String> {
    exporter_fichier(p, "modele.ifc")
}

/// Export IFC4 du projet ; `nom_fichier` (nom seul, sans dossier) et l'heure UTC de
/// l'export vont dans `FILE_NAME` de l'en-tête.
pub fn exporter_fichier(p: &Projet, nom_fichier: &str) -> Result<String, String> {
    Ok(contenu(p)?.fichier(nom_fichier, &step::maintenant()))
}

/// Entités du fichier (section DATA), sans l'en-tête.
fn contenu(p: &Projet) -> Result<Step, String> {
    p.verifier()?;
    let mut w = Step { lignes: Vec::new() };
    let zero = w.add("IFCCARTESIANPOINT((0.,0.,0.))".into());
    let axe = w.add(format!("IFCAXIS2PLACEMENT3D(#{zero},$,$)"));
    let placement = w.add(format!("IFCLOCALPLACEMENT($,#{axe})"));
    let nord = site::nord(p).map_or("$".into(), |d| format!("#{}", w.add(d)));
    let contexte = w.add(format!(
        "IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,0.00001,#{axe},{nord})"
    ));
    // longueurs en mm ; aires et volumes des quantités en m² et m³
    let unite = w.add("IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.)".into());
    let aire = w.add("IFCSIUNIT(*,.AREAUNIT.,$,.SQUARE_METRE.)".into());
    let volume = w.add("IFCSIUNIT(*,.VOLUMEUNIT.,$,.CUBIC_METRE.)".into());
    let unites = w.add(format!("IFCUNITASSIGNMENT((#{unite},#{aire},#{volume}))"));
    let projet = w.add(format!(
        "IFCPROJECT('{}',$,{},$,$,$,$,(#{contexte}),#{unites})",
        guid(&format!("projet:{}", p.nom), None),
        texte(&p.nom)
    ));
    let bat = w.add(format!(
        "IFCBUILDING('{}',$,{},$,$,#{placement},$,$,.ELEMENT.,$,$,$)",
        guid(&format!("bat:{}", p.nom), None),
        texte(&p.nom)
    ));
    // projet -> site -> bâtiment -> niveaux
    let site = site::site(&mut w, p, placement);
    w.add(format!(
        "IFCRELAGGREGATES('{}',$,$,$,#{projet},(#{site}))",
        guid("rel-projet", None)
    ));
    w.add(format!(
        "IFCRELAGGREGATES('{}',$,$,$,#{site},(#{bat}))",
        guid("rel-site", None)
    ));
    let mut niveaux = HashMap::new();
    let mut ns = Vec::new();
    for n in &p.niveaux {
        let id = w.add(format!(
            "IFCBUILDINGSTOREY('{}',$,{},$,$,#{placement},$,$,.ELEMENT.,{})",
            guid(&n.id, n.origine.as_ref()),
            texte(&n.nom),
            format!("{:.6}", n.altitude)
        ));
        niveaux.insert(n.id.clone(), id);
        ns.push(id);
    }
    if !ns.is_empty() {
        w.add(format!(
            "IFCRELAGGREGATES('{}',$,$,$,#{bat},{})",
            guid("rel-niveaux", None),
            refs(&ns)
        ));
    }
    let mut types_ifc = types::Types::default();
    let mut styles = styles::Styles::default();
    let mut jeux_couches = materiaux::Couches::new(&mut w, p);
    // axes des murs (lignes de référence des jeux de couches)
    let contexte_axe = w.add(format!("IFCGEOMETRICREPRESENTATIONSUBCONTEXT('Axis','Model',*,*,*,*,#{contexte},$,.GRAPH_VIEW.,$)"));
    let mut contenus: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut produits = HashMap::new();
    let jonctions = jonctions::coupes(p);
    // solides de tous les murs calculés ensemble, consommés dans l'ordre de `p.murs`
    let mut solides_murs = solides::murs(p, &jonctions).into_iter();
    for m in &p.murs {
        let solide = solides_murs.next().ok_or("Mur absent.")??;
        let t = p.type_mur(&m.type_mur).ok_or("Type de mur absent.")?;
        // couleur : matériau d'un mur monocouche (comme la vue 3D)
        let materiau = match t.couches.as_slice() {
            [c] if c.fonction != FonctionCouche::LameAir => c.materiau.as_deref(),
            _ => None,
        };
        let axe = materiaux::axe_mur(&mut w, contexte_axe, &m.axe);
        let rep = styles.forme(&mut w, p, contexte, &[(materiau, &solide)], &[axe]);
        let id = w.add(format!(
            "IFCWALL('{}',$,{},$,$,#{placement},{},$,.NOTDEFINED.)",
            guid(&m.id, m.origine.as_ref()),
            texte(&m.nom),
            ref_(rep)
        ));
        contenus
            .entry(*niveaux.get(&m.niveau).unwrap_or(&bat))
            .or_default()
            .push(id);
        produits.insert(m.id.clone(), id);
        proprietes::mur(&mut w, p, m, id);
        quantites::mur(&mut w, p, m, &solide, id);
        let materiau = jeux_couches.usage_mur(&mut w, t);
        w.add(format!(
            "IFCRELASSOCIATESMATERIAL('{}',$,$,$,(#{id}),#{materiau})",
            guid(&format!("mat:{}", m.id), None)
        ));
        let predefini = if t.categorie == Some(CategorieMur::Cloison) { "PARTITIONING" } else { "NOTDEFINED" };
        types_ifc.lier("IFCWALLTYPE", &t.id, &t.nom, predefini, Some(jeux_couches.jeu(&t.id)), id);
    }
    for a in &p.assemblages {
        let id = w.add(format!(
            "IFCELEMENTASSEMBLY('{}',$,{},$,$,#{placement},$,$,$,.NOTDEFINED.)",
            guid(&a.id, a.origine.as_ref()),
            texte(&a.nom)
        ));
        let couches: Vec<_> = a
            .couches
            .iter()
            .filter_map(|id| produits.get(id).copied())
            .collect();
        for objets in contenus.values_mut() {
            objets.retain(|x| !couches.contains(x));
        }
        let niveau = p
            .couches_assemblage(&a.id)
            .first()
            .and_then(|m| niveaux.get(&m.niveau))
            .copied()
            .unwrap_or(bat);
        contenus.entry(niveau).or_default().push(id);
        w.add(format!(
            "IFCRELAGGREGATES('{}',$,$,$,#{id},{})",
            guid(&format!("couches:{}", a.id), None),
            refs(&couches)
        ));
        produits.insert(a.id.clone(), id);
    }
    for d in &p.dalles {
        let t = p.type_dalle(&d.type_dalle).unwrap();
        let z = p.niveau(&d.niveau).unwrap().altitude + d.decalage;
        // une part par couche, du dessus vers le bas, colorée par son matériau ; les lames
        // d'air décalent les couches suivantes sans géométrie (comme la vue 3D)
        let plans = pente::plans(d);
        let (_, mut haut) = d.decalages_couches(t.epaisseur());
        let dessus = z + haut;
        let mut couches = Vec::new();
        for c in &t.couches {
            let bas = haut - c.epaisseur;
            if c.fonction != FonctionCouche::LameAir {
                let tris: Vec<_> = pente::couche(&d.contours, &plans, z, bas, haut).into_iter().map(|t| t.map(|v| v * 1000.0)).collect();
                couches.push((c.materiau.as_deref(), tris));
            }
            haut = bas;
        }
        let parts: Vec<_> = couches.iter().map(|(m, t)| (*m, t.as_slice())).collect();
        let rep = styles.forme(&mut w, p, contexte, &parts, &[]);
        let (classe, genre) = match d.sorte {
            SorteDalle::Dalle => ("IFCSLAB", "FLOOR"),
            SorteDalle::Sol => ("IFCCOVERING", "FLOORING"),
            SorteDalle::Plafond => ("IFCCOVERING", "CEILING"),
            SorteDalle::Toiture => ("IFCROOF", "NOTDEFINED"),
            SorteDalle::TerrePlein => ("IFCSLAB", "BASESLAB"),
        };
        let id = w.add(format!(
            "{classe}('{}',$,{},$,$,#{placement},{},$,.{genre}.)",
            guid(&d.id, d.origine.as_ref()),
            texte(&d.nom),
            ref_(rep)
        ));
        contenus.entry(niveaux[&d.niveau]).or_default().push(id);
        proprietes::dalle(&mut w, p, d, classe, id);
        quantites::dalle(&mut w, p, d, classe, id);
        let materiau = jeux_couches.materiau_dalle(&mut w, d, classe, dessus);
        w.add(format!(
            "IFCRELASSOCIATESMATERIAL('{}',$,$,$,(#{id}),#{materiau})",
            guid(&format!("mat:{}", d.id), None)
        ));
        let classe_type = match classe { "IFCSLAB" => "IFCSLABTYPE", "IFCCOVERING" => "IFCCOVERINGTYPE", _ => "IFCROOFTYPE" };
        types_ifc.lier(classe_type, &t.id, &t.nom, "NOTDEFINED", Some(jeux_couches.jeu(&t.id)), id);
    }
    for b in &p.barres {
        let solide = solides::barre(p, b)?;
        let materiau = b.pose.type_poutre.as_deref().and_then(|t| p.type_poutre(t)).and_then(|t| t.materiau.as_deref());
        let rep = styles.forme(&mut w, p, contexte, &[(materiau, &solide)], &[]);
        let (classe, classe_type, predefini) = match b.genre {
            GenreBarre::Poteau => ("IFCCOLUMN", "IFCCOLUMNTYPE", "COLUMN"),
            GenreBarre::Poutre => ("IFCBEAM", "IFCBEAMTYPE", "BEAM"),
        };
        let id = w.add(format!("{classe}('{}',$,{},$,$,#{placement},{},$,.NOTDEFINED.)", guid(&b.id, None), texte(&b.nom), ref_(rep)));
        contenus.entry(niveaux[&b.niveau]).or_default().push(id);
        proprietes::barre(&mut w, p, b, id);
        quantites::barre(&mut w, b, &solide, id);
        if let Some(t) = b.pose.type_poutre.as_deref().and_then(|t| p.type_poutre(t)) {
            types_ifc.lier(classe_type, &t.id, &t.nom, predefini, None, id);
        }
    }
    for c in &p.composants {
        let solides = c.solides(p)?;
        let parts: Vec<_> = solides.iter().map(|(m, t)| (m.as_deref(), t.as_slice())).collect();
        let rep = styles.forme(&mut w, p, contexte, &parts, &[]);
        let id = w.add(format!("IFCFURNITURE('{}',$,{},$,$,#{placement},{},$,.NOTDEFINED.)", guid(&c.id, None), texte(&c.nom), ref_(rep)));
        contenus.entry(niveaux[&c.niveau]).or_default().push(id);
    }
    // pièces : volumes du sol au niveau supérieur (2,5 m au dernier), agrégés au niveau
    let mut espaces: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    for x in p.pieces.iter().filter(|x| x.fermee()) {
        let z = p.niveau(&x.niveau).ok_or("Pièce : niveau absent")?.altitude;
        let haut = p.niveaux.iter().map(|n| n.altitude).filter(|a| *a > z + 1.0).fold(f64::INFINITY, f64::min);
        let haut = if haut.is_finite() { haut } else { z + 2500.0 };
        let rep = w.mesh(&solides::prisme(&x.contour, z, haut), contexte);
        let id = w.add(format!("IFCSPACE('{}',$,{},$,$,#{placement},{},{},.ELEMENT.,.INTERNAL.,$)", guid(&x.id, None), texte(&x.numero), ref_(rep), texte(&x.nom)));
        espaces.entry(niveaux[&x.niveau]).or_default().push(id);
        quantites::piece(&mut w, x, id);
    }
    for (n, ids) in espaces {
        w.add(format!("IFCRELAGGREGATES('{}',$,$,$,#{n},{})", guid(&format!("espaces:{n}"), None), refs(&ids)));
    }
    for e in &p.escaliers {
        let rep = w.mesh(&e.solide(p)?, contexte);
        let id = w.add(format!("IFCSTAIR('{}',$,{},$,$,#{placement},{},$,.NOTDEFINED.)", guid(&e.id, None), texte(&e.nom), ref_(rep)));
        contenus.entry(niveaux[&e.niveau]).or_default().push(id);
        proprietes::escalier(&mut w, p, e, id);
    }
    for g in &p.garde_corps {
        let solides = g.solides(p)?;
        let parts: Vec<_> = solides.iter().map(|(m, _, t)| (m.as_deref(), t.as_slice())).collect();
        let rep = styles.forme(&mut w, p, contexte, &parts, &[]);
        let id = w.add(format!("IFCRAILING('{}',$,{},$,$,#{placement},{},$,.GUARDRAIL.)", guid(&g.id, None), texte(&g.nom), ref_(rep)));
        contenus.entry(niveaux[&g.niveau]).or_default().push(id);
        proprietes::garde_corps(&mut w, p, g, id);
        quantites::garde_corps(&mut w, g, id);
        if let Some(t) = g.type_garde_corps.as_ref().and_then(|t| p.types_garde_corps.iter().find(|x| &x.id == t)) {
            types_ifc.lier("IFCRAILINGTYPE", &t.id, &t.nom, "GUARDRAIL", None, id);
        }
    }
    for o in &p.ouvertures {
        let (axe, e, niveau) = solides::hote(p, &o.mur)?;
        let z = p.niveau(&niveau).unwrap().altitude + o.allege;
        let vide = solides::prisme(
            &solides::bande(
                &axe,
                o.abscisse - o.largeur / 2.0,
                o.abscisse + o.largeur / 2.0,
                e,
            )?,
            z,
            z + o.hauteur,
        );
        let rep = w.mesh(&vide, contexte);
        let id = w.add(format!(
            "IFCOPENINGELEMENT('{}',$,{},$,$,#{placement},{},$,.OPENING.)",
            guid(&format!("vide:{}", o.id), None),
            texte(&o.nom),
            ref_(rep)
        ));
        // IFC impose un IfcElement hôte ; un assemblage est un IfcElement.
        w.add(format!(
            "IFCRELVOIDSELEMENT('{}',$,$,$,#{},#{id})",
            guid(&format!("perc:{}", o.id), None),
            produits[&o.mur]
        ));
        if o.genre != GenreOuverture::Libre {
            let menuiserie = solides::ouverture_parts(p, o)?;
            let parts: Vec<_> = menuiserie.iter().map(|(_, m, t)| (m.as_deref(), t.as_slice())).collect();
            let rep = styles.forme(&mut w, p, contexte, &parts, &[]);
            let classe = if o.genre == GenreOuverture::Porte {
                "IFCDOOR"
            } else {
                "IFCWINDOW"
            };
            let element = w.add(format!(
                "{classe}('{}',$,{},$,$,#{placement},{},$,{},{},.NOTDEFINED.,.NOTDEFINED.,$)",
                guid(&o.id, o.origine.as_ref()),
                texte(&o.nom),
                ref_(rep),
                format!("{:.6}", o.hauteur),
                format!("{:.6}", o.largeur)
            ));
            contenus.entry(niveaux[&niveau]).or_default().push(element);
            proprietes::ouverture(&mut w, p, o, classe == "IFCDOOR", element);
            quantites::ouverture(&mut w, o, classe == "IFCDOOR", element);
            if let Some(t) = o.type_ouverture.as_ref().and_then(|t| p.types_ouvertures.iter().find(|x| &x.id == t)) {
                let (classe_type, predefini) = if classe == "IFCDOOR" { ("IFCDOORTYPE", "DOOR") } else { ("IFCWINDOWTYPE", "WINDOW") };
                types_ifc.lier(classe_type, &t.id, &t.nom, predefini, None, element);
            }
            w.add(format!(
                "IFCRELFILLSELEMENT('{}',$,$,$,#{id},#{element})",
                guid(&format!("rempl:{}", o.id), None)
            ));
        }
    }
    for t in &p.terrains {
        // solide de l'épaisseur du type, cotes NGF ramenées à l'origine du projet
        let rep = w.mesh(&t.solide(p.ngf(), p.type_terrain(t).epaisseur)?, contexte);
        let id = w.add(format!("IFCGEOGRAPHICELEMENT('{}',$,{},$,$,#{placement},{},$,.TERRAIN.)", guid(&t.id, None), texte(&t.nom), ref_(rep)));
        contenus.entry(site).or_default().push(id);
    }
    for limite in &p.limites_propriete {
        limite.verifier()?;
        let points: Vec<usize> = limite.contour.iter().map(|p| w.add(format!("IFCCARTESIANPOINT(({:.6},{:.6},{:.6}))", p[0], p[1], limite.altitude))).collect();
        let mut ferme = points.clone();
        ferme.push(points[0]);
        let ligne = w.add(format!("IFCPOLYLINE({})", refs(&ferme)));
        let rep = w.add(format!("IFCSHAPEREPRESENTATION(#{contexte},'Annotation','Curve3D',(#{ligne}))"));
        let rep = w.add(format!("IFCPRODUCTDEFINITIONSHAPE($,$,(#{rep}))"));
        let id = w.add(format!("IFCANNOTATION('{}',$,{},$,$,#{placement},#{rep})", guid(&limite.id, None), texte(&limite.nom)));
        contenus.entry(site).or_default().push(id);
    }
    for e in &p.importes {
        let rep = w.mesh(&solides::maillage(&e.maillage)?, contexte);
        let id = w.add(format!(
            "IFCBUILDINGELEMENTPROXY('{}',$,{},{},{},#{placement},{},$,.NOTDEFINED.)",
            guid(&e.id, e.origine.as_ref()),
            texte(&e.nom),
            texte(&e.raison),
            texte(&e.categorie),
            ref_(rep)
        ));
        contenus
            .entry(
                e.niveau
                    .as_ref()
                    .and_then(|n| niveaux.get(n))
                    .copied()
                    .unwrap_or(bat),
            )
            .or_default()
            .push(id);
    }
    types_ifc.ecrire(&mut w);
    let mut contenus: Vec<_> = contenus.into_iter().collect();
    contenus.sort_by_key(|(id, _)| *id);
    for (n, ids) in contenus {
        w.add(format!(
            "IFCRELCONTAINEDINSPATIALSTRUCTURE('{}',$,$,$,{},#{n})",
            guid(&format!("contenu:{n}"), None),
            refs(&ids)
        ));
    }
    Ok(w)
}
