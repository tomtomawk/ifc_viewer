//! Adaptation du modèle natif vers la chaîne de rendu commune.
use glam::{DVec3, Vec3, Vec4};
use noyau::Id;
use ifc_base::semantique::IfcModel;
use scene::*;
pub use modele::identifiants::{id_natif, numero_vue};
use crate::formats::ifc::geom::{Part, ProductGeom};
use modele::{Projet, SorteDalle};
use noyau::chrono::chrono;
use crate::formats::ifc::step::{Entity, StepFile, Value};
use rayon::prelude::*;
use std::path::Path;

/// Vue de départ d'un projet natif vide, sans inventer de géométrie ni de source IFC.
#[cfg(test)]
pub fn projet_vide(projet: &modele::Projet, path: &std::path::Path) -> Result<Scene, String> {
    depuis_natif(projet, path)
}

pub fn depuis_natif(projet: &Projet, path: &Path) -> Result<Scene, String> {
    projet.verifier()?;
    depuis_natif_verifie(projet, path)
}

/// `depuis_natif` pour un projet que l'appelant vient de vérifier (`Projet::verifier`).
pub fn depuis_natif_verifie(projet: &Projet, path: &Path) -> Result<Scene, String> {
    let mut file = StepFile { schema: "Modèle natif v0".into(), entities: Default::default() };
    let mut entite = |id: u64, classe: &str, guid: &str, nom: &str, n: usize| -> Result<(), String> {
        let mut args = vec![Value::Null; n.max(3)];
        args[0] = Value::Str(guid.into());
        args[2] = Value::Str(nom.into());
        if file.entities.insert(id, Entity { ty: classe.into(), args }).is_some() { return Err("Identifiants natifs dupliqués.".into()); }
        Ok(())
    };
    let racine = id_natif("projet", "racine");
    entite(racine, "IFCPROJECT", "projet-natif", &projet.nom, 9)?;
    for n in &projet.niveaux {
        if !n.altitude.is_finite() { return Err("Altitude de niveau non valide.".into()); }
        entite(id_natif("niveau", &n.id), "IFCBUILDINGSTOREY", &n.id, &n.nom, 10)?;
    }
    for t in &projet.types_murs { entite(id_natif("type-mur", &t.id), "IFCWALLTYPE", &t.id, &t.nom, 10)?; }
    for a in &projet.assemblages { entite(id_natif("assemblage", &a.id), "IFCELEMENTASSEMBLY", &a.id, &a.nom, 9)?; }
    for m in &projet.murs { entite(id_natif("mur", &m.id), "IFCWALL", &m.id, &m.nom, 9)?; }
    for t in &projet.types_dalles { entite(id_natif("type-dalle", &t.id), "IFCSLABTYPE", &t.id, &t.nom, 10)?; }
    for d in &projet.dalles {
        let classe = match d.sorte {
            SorteDalle::Dalle | SorteDalle::TerrePlein => "IFCSLAB",
            SorteDalle::Sol | SorteDalle::Plafond => "IFCCOVERING",
            SorteDalle::Toiture => "IFCROOF",
        };
        entite(id_natif("dalle", &d.id), classe, &d.id, &d.nom, 9)?;
        // remblai d'un terre-plein : élément du site à part, sélectionnable
        if modele::elements::terre_plein::remblai(projet, d).is_some_and(|r| !r.tris.is_empty()) {
            entite(id_natif("remblai", &d.id), "IFCGEOGRAPHICELEMENT", &format!("remblai-{}", d.id), &format!("Remblai · {}", d.nom), 9)?;
        }
    }
    for t in &projet.terrains { entite(id_natif("terrain", &t.id), "IFCGEOGRAPHICELEMENT", &t.id, &t.nom, 9)?; }
    for l in &projet.limites_propriete { entite(id_natif("limite_propriete", &l.id), "IFCGEOGRAPHICELEMENT", &l.id, &l.nom, 9)?; }
    for o in &projet.ouvertures {
        let classe = match o.genre {
            modele::GenreOuverture::Porte => "IFCDOOR",
            modele::GenreOuverture::Fenetre => "IFCWINDOW",
            modele::GenreOuverture::Libre => "IFCOPENINGELEMENT",
        };
        entite(id_natif("ouverture", &o.id), classe, &o.id, &o.nom, 9)?;
    }
    for b in &projet.barres {
        let classe = match b.genre { modele::GenreBarre::Poteau => "IFCCOLUMN", modele::GenreBarre::Poutre => "IFCBEAM" };
        entite(id_natif("barre", &b.id), classe, &b.id, &b.nom, 9)?;
    }
    for e in &projet.escaliers {
        entite(id_natif("escalier", &e.id), "IFCSTAIR", &e.id, &e.nom, 9)?;
    }
    for t in &projet.types_composants {
        entite(id_natif("type-composant", &t.id), "IFCFURNITURETYPE", &t.id, &t.nom, 10)?;
    }
    for c in &projet.composants {
        entite(id_natif("composant", &c.id), "IFCFURNISHINGELEMENT", &c.id, &c.nom, 9)?;
    }
    for a in &projet.arbres {
        entite(id_natif("arbre", &a.id), "IFCGEOGRAPHICELEMENT", &a.id, &a.nom, 9)?;
    }
    for g in &projet.garde_corps {
        entite(id_natif("garde_corps", &g.id), "IFCRAILING", &g.id, &g.nom, 9)?;
    }
    for o in &projet.importes {
        // topographie de Revit : élément géographique, comme un terrain natif (visible dans
        // les plans de tous les niveaux dont la plage de vue la touche)
        let topo = o.origine.as_ref().is_some_and(|o| o.classe.starts_with("Toposolid") || o.classe == "SiteSurface");
        let classe = o.origine.as_ref().map(|o| o.classe.to_uppercase()).filter(|c| c.starts_with("IFC"))
            .unwrap_or_else(|| if topo { "IFCGEOGRAPHICELEMENT".into() } else if o.categorie.to_uppercase().starts_with("IFC") { o.categorie.to_uppercase() } else { "IFCBUILDINGELEMENTPROXY".into() });
        entite(id_natif("importe", &o.id), &classe, &o.id, &o.nom, 9)?;
    }
    for n in &projet.niveaux { file.entities.get_mut(&id_natif("niveau", &n.id)).unwrap().args[9] = Value::Real(n.altitude); }
    let mut model = IfcModel::new(file);
    model.length_scale = 0.001;
    for n in &projet.niveaux {
        let id = id_natif("niveau", &n.id);
        model.parent.insert(id, racine);
        model.children.entry(racine).or_default().push(id);
    }
    // assemblages sous le niveau de leur première couche ; couches sous l'assemblage
    for a in &projet.assemblages {
        let Some(premiere) = projet.couches_assemblage(&a.id).first().map(|m| m.niveau.clone()) else { continue };
        let (id, niveau) = (id_natif("assemblage", &a.id), id_natif("niveau", &premiere));
        model.parent.insert(id, niveau);
        model.children.entry(niveau).or_default().push(id);
        model.type_of.insert(id, id_natif("type-mur", &a.type_mur));
    }
    let mut t_etape = std::time::Instant::now();
    let mut couches_coupe = Vec::new();
    // bandes en mètres dans le repère du fichier (dalles, ossatures), ramenées à la scène
    let mut bandes_dalles: Vec<(u64, Vec<CoucheCoupe>)> = Vec::new();
    let mut geoms = Vec::new();
    // matériau de chaque partie des éléments (jonctions sans arête entre mêmes matériaux)
    let mut materiaux_parts: Vec<Vec<Option<String>>> = Vec::new();
    let mut table_motifs: Vec<(String, [f32; 4])> = Vec::new();
    // apparences de rendu (onglet Apparence) indexées par les couleurs de rendu
    let mut table_apparences: Vec<(String, modele::Apparence)> = Vec::new();
    // jonctions (noyaux prolongés) ; les couches moins prioritaires s'arrêtent sur les
    // faces des autres (solides)
    let (jonctions, _) = modele::murs::jonctions::jonctions(projet);
    chrono("scène : jonctions", &mut t_etape);
    // couches d'un même assemblage consécutives (sélection d'un seul tenant)
    let mut ordre: Vec<&modele::Mur> = projet.murs.iter().filter(|m| m.assemblage.is_none()).collect();
    for a in &projet.assemblages {
        ordre.extend(projet.couches_assemblage(&a.id));
    }
    ordre.extend(projet.murs.iter().filter(|m| m.assemblage.as_ref().is_some_and(|a| projet.assemblage(a).is_none_or(|x| !x.couches.contains(&m.id)))));
    // solides calculés ensemble (volumes bruts partagés, en parallèle), consommés dans l'ordre
    let mut solides: std::collections::HashMap<&str, Result<Vec<modele::volumes::solides::Tri>, String>> =
        projet.murs.iter().map(|m| m.id.as_str()).zip(modele::volumes::solides::murs(projet, &jonctions)).collect();
    chrono("scène : solides des murs", &mut t_etape);
    for m in ordre {
        let type_mur = projet.type_mur(&m.type_mur).ok_or("Type de mur introuvable.")?;
        let solide = solides.remove(m.id.as_str()).ok_or("Mur en double.")??;
        let id = id_natif("mur", &m.id);
        let parent = match m.assemblage.as_ref().filter(|a| projet.assemblage(a).is_some()) {
            Some(a) => id_natif("assemblage", a),
            None => id_natif("niveau", &m.niveau),
        };
        model.parent.insert(id, parent);
        model.children.entry(parent).or_default().push(id);
        model.type_of.insert(id, id_natif("type-mur", &m.type_mur));
        // matériau d'un mur monocouche (couche d'assemblage) : couleur et motif de coupe
        let materiau = match type_mur.couches.as_slice() {
            [c] => c.materiau.as_ref().map(|n| projet.materiau(n).cloned().unwrap_or_else(|| modele::murs::assemblage::materiau_par_defaut(n))),
            _ => None,
        };
        let couleur = materiau.as_ref().map_or([0.72, 0.72, 0.69, 1.0], |x| {
            let [r, g, b] = x.couleur.map(|c| c as f32 / 255.0);
            [r, g, b, 1.0]
        });
        let couleur = opacite(materiau.as_ref(), couleur);
        let neutre = Vec4::new(0.0, 0.0, 0.0, 1.0);
        if let Some(x) = &materiau {
            let mut bandes = coupe_materiau(projet, x, vec![neutre, neutre], type_mur.couches.first().map_or(noyau::aretes::TRAIT_FORT_MM, |c| projet.trait_couche(c)));
            if let modele::Axe::Arc { centre, .. } = m.axe {
                for bande in &mut bandes { bande.centre_isolation = Some([centre[0] * 0.001, centre[1] * 0.001]); }
            }
            // ossature : la couche découpée en membrures et travées de remplissage
            let cases = modele::murs::ossature::bandes_mur(projet, m);
            if let (Some(o), false) = (modele::murs::ossature::ossature_mur(projet, m), cases.is_empty()) {
                let membrure = projet.materiau(o.materiau()).cloned().unwrap_or_else(|| modele::murs::assemblage::materiau_par_defaut(o.materiau()));
                // membrures à la plume de la couche, remplissage à une plume plus fine
                let trait_mm = bandes[0].trait_mm;
                let trait_remplissage = projet.epaisseurs_plumes()[(o.plume_remplissage() - 1) as usize];
                let decoupees = cases.iter().flat_map(|case| {
                    let plans: Vec<Vec4> = case.plans.iter().map(|q| Vec4::new(q[0] as f32, q[1] as f32, q[2] as f32, (q[3] * 0.001) as f32)).collect();
                    if case.membrure { coupe_materiau(projet, &membrure, plans, trait_mm) } else { bandes.iter().cloned().map(|mut b| { b.plans = plans.clone(); b.trait_mm = trait_remplissage; b }).collect() }
                }).collect();
                bandes_dalles.push((id, decoupees));
                // 3D : les surfaces du solide réparties entre remplissage et membrures
                let (remplissage, membrures) = modele::murs::ossature::repartir(&solide, &cases);
                if !membrures.is_empty() && !remplissage.is_empty() {
                    let [r, g, b] = membrure.couleur.map(|c| c as f32 / 255.0);
                    let couleur_membrure = opacite(Some(&membrure), [r, g, b, 1.0]);
                    let en_m = |t: Vec<[glam::DVec3; 3]>| t.into_iter().map(|t| t.map(|v| v * 0.001)).collect();
                    materiaux_parts.push(vec![Some(x.nom.clone()), Some(membrure.nom.clone())]);
                    geoms.push(ProductGeom {
                        id,
                        parts: vec![Part { color: couleur, tris: en_m(remplissage) }, Part { color: couleur_membrure, tris: en_m(membrures) }],
                        lignes: Vec::new(),
                        rendu: vec![Some(rendu_materiau(x, couleur, &mut table_apparences)), Some(rendu_materiau(&membrure, couleur_membrure, &mut table_apparences))],
                        motifs: vec![motif_surface(projet, x, &mut table_motifs), motif_surface(projet, &membrure, &mut table_motifs)],
                    });
                    continue;
                }
            } else {
                couches_coupe.push((id, bandes));
            }
        } else if type_mur.couches.len() == 1 {
            // mur sans matériau : poché et hachures par défaut, mais en bande de coupe, pour
            // que les jonctions entre murs identiques se lisent sans trait
            couches_coupe.push((id, vec![CoucheCoupe {
                plans: vec![neutre, neutre],
                couleur: [184, 184, 176, 255],
                fond: [255, 255, 255, 255],
                motif_fond: None,
                motif: None,
                couleur_motif: [70, 70, 70, 255],
                hachure_defaut: true,
                isolation: false,
                centre_isolation: None,
                trait_mm: noyau::aretes::TRAIT_FORT_MM,
            }]));
        }
        let tris = solide.into_iter().map(|t| t.map(|v| v * 0.001)).collect();
        materiaux_parts.push(vec![materiau.as_ref().map(|m| m.nom.clone())]);
        geoms.push(ProductGeom { id, parts: vec![Part { color: couleur, tris }], lignes: Vec::new(), rendu: vec![materiau.as_ref().map(|m| rendu_materiau(m, couleur, &mut table_apparences))], motifs: vec![materiau.as_ref().and_then(|m| motif_surface(projet, m, &mut table_motifs))] });
    }
    chrono("scène : mise en objets des murs", &mut t_etape);
    // dalles : un prisme par couche, du dessus (niveau + décalage) vers le bas
    for d in &projet.dalles {
        let t = projet.type_dalle(&d.type_dalle).ok_or_else(|| format!("{} : type de dalle introuvable.", d.nom))?;
        let alt = projet.niveau(&d.niveau).ok_or_else(|| format!("{} : niveau introuvable.", d.nom))?.altitude;
        let id = id_natif("dalle", &d.id);
        let niveau = id_natif("niveau", &d.niveau);
        model.parent.insert(id, niveau);
        model.children.entry(niveau).or_default().push(id);
        model.type_of.insert(id, id_natif("type-dalle", &d.type_dalle));
        // couches du haut vers le bas, décalées de la surface de référence (sous-face ou
        // dessus, en pente si des arêtes en définissent une)
        let plans = modele::elements::pente::plans(d);
        let z0 = alt + d.decalage;
        let (_, mut e) = d.decalages_couches(t.epaisseur());
        let (mut parts, mut rendu, mut bandes, mut noms, mut motifs) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for c in &t.couches {
            let (haut, bas) = (e, e - c.epaisseur);
            e = bas;
            let tris = modele::elements::pente::couche(&d.contours, &plans, z0, bas, haut);
            if tris.is_empty() { continue; }
            let m = c.materiau.as_ref().map(|n| projet.materiau(n).cloned().unwrap_or_else(|| modele::murs::assemblage::materiau_par_defaut(n)));
            let couleur = m.as_ref().map_or([0.70, 0.70, 0.68, 1.0], |x| {
                let [r, g, b] = x.couleur.map(|c| c as f32 / 255.0);
                [r, g, b, 1.0]
            });
            let couleur = opacite(m.as_ref(), couleur);
            if let Some(m) = &m {
                // bandes de coupe entre le dessous et le dessus de la couche (m, repère du fichier)
                let solives = c.ossature.as_ref().filter(|o| o.valide()).map(|o| {
                    let membrure = projet.materiau(o.materiau()).cloned().unwrap_or_else(|| modele::murs::assemblage::materiau_par_defaut(o.materiau()));
                    (membrure, modele::murs::ossature::bandes_dalle(o, &d.contours))
                }).filter(|(_, cases)| !cases.is_empty());
                for jeu in modele::elements::pente::bandes_contours(&d.contours, &plans, z0, bas, haut) {
                    match &solives {
                        Some((membrure, cases)) => for case in cases {
                            let mut jeu = jeu.clone();
                            jeu.extend(case.plans.iter().map(|q| Vec4::new(q[0] as f32, q[1] as f32, q[2] as f32, (q[3] * 0.001) as f32)));
                            let trait_mm = if case.membrure { projet.trait_couche(c) } else { projet.epaisseurs_plumes()[(c.ossature.as_ref().map_or(2, |o| o.plume_remplissage()) - 1) as usize] };
                            bandes.extend(coupe_materiau(projet, if case.membrure { membrure } else { m }, jeu, trait_mm));
                        },
                        None => bandes.extend(coupe_materiau(projet, m, jeu, projet.trait_couche(c))),
                    }
                }
            }
            // solives : les surfaces de la couche réparties entre remplissage et membrures
            let solives = m.as_ref().zip(c.ossature.as_ref().filter(|o| o.valide())).and_then(|(_, o)| {
                let cases = modele::murs::ossature::bandes_dalle(o, &d.contours);
                let en_mm: Vec<[glam::DVec3; 3]> = tris.iter().map(|t| t.map(|v| v * 1000.0)).collect();
                let (remplissage, membrures) = modele::murs::ossature::repartir(&en_mm, &cases);
                (!membrures.is_empty() && !remplissage.is_empty()).then(|| (projet.materiau(o.materiau()).cloned().unwrap_or_else(|| modele::murs::assemblage::materiau_par_defaut(o.materiau())), remplissage, membrures))
            });
            rendu.push(m.as_ref().map(|m| rendu_materiau(m, couleur, &mut table_apparences)));
            noms.push(m.as_ref().map(|m| m.nom.clone()));
            motifs.push(m.as_ref().and_then(|m| motif_surface(projet, m, &mut table_motifs)));
            if let Some((membrure, remplissage, membrures)) = solives {
                let en_m = |t: Vec<[glam::DVec3; 3]>| t.into_iter().map(|t| t.map(|v| v * 0.001)).collect();
                parts.push(Part { color: couleur, tris: en_m(remplissage) });
                let [r, g, b] = membrure.couleur.map(|c| c as f32 / 255.0);
                let couleur_membrure = opacite(Some(&membrure), [r, g, b, 1.0]);
                rendu.push(Some(rendu_materiau(&membrure, couleur_membrure, &mut table_apparences)));
                noms.push(Some(membrure.nom.clone()));
                motifs.push(motif_surface(projet, &membrure, &mut table_motifs));
                parts.push(Part { color: couleur_membrure, tris: en_m(membrures) });
                continue;
            }
            parts.push(Part { color: couleur, tris });
        }
        if parts.is_empty() {
            return Err(format!("{} : contour non valide.", d.nom));
        }
        if !bandes.is_empty() { bandes_dalles.push((id, bandes)); }
        materiaux_parts.push(noms);
        geoms.push(ProductGeom { id, parts, lignes: Vec::new(), rendu, motifs });
        // terre-plein : remblai sous la plate-forme et ses talus jusqu'au terrain naturel,
        // et fouilles rebouchées ; élément du site à part (sélection, volume, type de
        // terrain), couleur et poché du matériau de son type
        if let Some(r) = modele::elements::terre_plein::remblai(projet, d).filter(|r| !r.tris.is_empty()) {
            let id = id_natif("remblai", &d.id);
            model.parent.insert(id, racine);
            model.children.entry(racine).or_default().push(id);
            let nom = projet.type_remblai(d).materiau.unwrap_or_else(|| "Remblai".into());
            let m = projet.materiau(&nom).cloned().unwrap_or_else(|| modele::murs::assemblage::materiau_par_defaut(&nom));
            let [cr, cg, cb] = m.couleur.map(|c| c as f32 / 255.0);
            let couleur = opacite(Some(&m), [cr, cg, cb, 1.0]);
            let neutre = Vec4::new(0.0, 0.0, 0.0, 1.0);
            couches_coupe.push((id, coupe_materiau(projet, &m, vec![neutre, neutre], noyau::aretes::TRAIT_FORT_MM)));
            materiaux_parts.push(vec![Some(m.nom.clone())]);
            geoms.push(ProductGeom { id, parts: vec![Part { color: couleur, tris: r.tris.iter().map(|t| t.map(|v| v * 0.001)).collect() }], lignes: Vec::new(), rendu: vec![Some(rendu_materiau(&m, couleur, &mut table_apparences))], motifs: vec![motif_surface(projet, &m, &mut table_motifs)] });
        }
    }
    chrono("scène : dalles", &mut t_etape);
    // menuiseries calculées en parallèle, rangées dans l'ordre des ouvertures
    let mut menuiseries = {
        use rayon::prelude::*;
        projet.ouvertures.par_iter().map(|o| modele::volumes::solides::ouverture_parts(projet, o)).collect::<Vec<_>>().into_iter()
    };
    for o in &projet.ouvertures {
        let id = id_natif("ouverture", &o.id);
        let (_, _, niveau) = modele::volumes::solides::hote(projet, &o.mur)?;
        let parent = id_natif("niveau", &niveau);
        model.parent.insert(id, parent);
        model.children.entry(parent).or_default().push(id);
        // famille : couleur de la menuiserie, vitrage transparent, appui minéral
        use modele::famille::Role;
        let menuiserie = o.type_ouverture.as_ref().and_then(|t| projet.types_ouvertures.iter().find(|x| &x.id == t)).and_then(|t| t.modele.as_ref().map(|m| m.couleur).or(t.famille.as_ref().map(|f| f.couleur)))
            .map_or([0.55, 0.40, 0.24, 1.0], |c| { let [r, g, b] = c.map(|c| c as f32 / 255.0); [r, g, b, 1.0] });
        let parts: Vec<Part> = menuiseries.next().ok_or("Menuiserie absente.")??.into_iter().map(|(role, materiau, tris)| Part {
            // paramètre matériau : couleur du matériau du projet (le vitrage reste translucide)
            color: match (materiau.as_ref().and_then(|m| projet.materiau(m)), role) {
                (Some(m), r) => { let [cr, cg, cb] = m.couleur.map(|c| c as f32 / 255.0); [cr, cg, cb, if r == Role::Vitrage { 0.35 } else { 1.0 }] }
                (None, Role::Vitrage) => [0.62, 0.78, 0.86, 0.35],
                (None, Role::Appui) => [0.78, 0.76, 0.72, 1.0],
                (None, _) => menuiserie,
            },
            tris: tris.into_iter().map(|t| t.map(|v| v * 0.001)).collect(),
        }).collect();
        materiaux_parts.push(vec![None; parts.len()]);
        geoms.push(ProductGeom { id, parts, lignes: Vec::new(), rendu: Vec::new(), motifs: Vec::new() });
    }
    chrono("scène : ouvertures", &mut t_etape);
    for b in &projet.barres {
        let id = id_natif("barre", &b.id);
        let parent = id_natif("niveau", &b.niveau);
        model.parent.insert(id, parent);
        model.children.entry(parent).or_default().push(id);
        let tris = modele::volumes::solides::barre(projet, b)?.into_iter().map(|t| t.map(|v| v * 0.001)).collect();
        materiaux_parts.push(vec![None]);
        geoms.push(ProductGeom { id, parts: vec![Part { color: [0.65,0.68,0.72,1.0], tris }], lignes: Vec::new(), rendu: Vec::new(), motifs: Vec::new() });
    }
    // arbres : tronc (matériau « Écorce ») et houppier (« Feuillage »), couleurs par
    // défaut sans ces matériaux
    for a in &projet.arbres {
        use modele::elements::arbre::{MATERIAU_ECORCE, MATERIAU_FEUILLAGE};
        let id = id_natif("arbre", &a.id);
        let parent = id_natif("niveau", &a.niveau);
        model.parent.insert(id, parent);
        model.children.entry(parent).or_default().push(id);
        let z0 = projet.niveau(&a.niveau).map_or(0.0, |n| n.altitude);
        // proxy (style Graphique, plans) et remplacement (style Réaliste : branches et
        // plans de feuillage découpés), choisis par le shader selon le style (4ᵉ octet du
        // motif : 1 Graphique seulement, 2 Réaliste seulement)
        let (tronc, houppier) = a.solides(z0);
        let r = a.remplacement(z0);
        let defaut = |nom: &str, couleur: [u8; 3]| modele::Materiau { nom: nom.into(), couleur, ..Default::default() };
        let ecorce = projet.materiau(MATERIAU_ECORCE).cloned().unwrap_or_else(|| defaut(MATERIAU_ECORCE, [92, 76, 62]));
        let feuillage = projet.materiau(MATERIAU_FEUILLAGE).cloned().unwrap_or_else(|| defaut(MATERIAU_FEUILLAGE, a.essence.couleur()));
        let mut parts = Vec::new();
        let mut rendu = Vec::new();
        let mut motifs = Vec::new();
        for (m, tris, style) in [(&ecorce, tronc, 1u8), (&feuillage, houppier, 1), (&ecorce, r.bois, 2), (&feuillage, r.feuillage, 2)] {
            let [cr, cg, cb] = m.couleur.map(|x| x as f32 / 255.0);
            let couleur = opacite(Some(m), [cr, cg, cb, 1.0]);
            rendu.push(Some(rendu_materiau(m, couleur, &mut table_apparences)));
            motifs.push(Some([0, 0, 0, style]));
            parts.push(Part { color: couleur, tris: tris.into_iter().map(|t| t.map(|v| v * 0.001)).collect() });
        }
        materiaux_parts.push(vec![Some(MATERIAU_ECORCE.into()), Some(MATERIAU_FEUILLAGE.into()), Some(MATERIAU_ECORCE.into()), Some(MATERIAU_FEUILLAGE.into())]);
        geoms.push(ProductGeom { id, parts, lignes: Vec::new(), rendu, motifs });
    }
    for c in &projet.composants {
        let id = id_natif("composant", &c.id);
        let parent = id_natif("niveau", &c.niveau);
        model.parent.insert(id, parent);
        model.children.entry(parent).or_default().push(id);
        model.type_of.insert(id, id_natif("type-composant", &c.type_composant));
        let couleur = projet.types_composants.iter().find(|t| t.id == c.type_composant).map_or([0.77, 0.63, 0.44, 1.0], |t| {
            let [r, g, b] = t.modele.couleur.map(|x| x as f32 / 255.0);
            [r, g, b, 1.0]
        });
        let parts: Vec<Part> = c.solides(projet)?.into_iter().map(|(materiau, tris)| Part {
            color: materiau.as_ref().and_then(|m| projet.materiau(m)).map_or(couleur, |m| { let [r, g, b] = m.couleur.map(|x| x as f32 / 255.0); [r, g, b, 1.0] }),
            tris: tris.into_iter().map(|t| t.map(|v| v * 0.001)).collect(),
        }).collect();
        materiaux_parts.push(vec![None; parts.len()]);
        geoms.push(ProductGeom { id, parts, lignes: Vec::new(), rendu: Vec::new(), motifs: Vec::new() });
    }
    for e in &projet.escaliers {
        let id = id_natif("escalier", &e.id);
        let parent = id_natif("niveau", &e.niveau);
        model.parent.insert(id, parent);
        model.children.entry(parent).or_default().push(id);
        let tris = e.solide(projet)?.into_iter().map(|t| t.map(|v| v * 0.001)).collect();
        materiaux_parts.push(vec![None]);
        geoms.push(ProductGeom { id, parts: vec![Part { color: [0.74, 0.73, 0.70, 1.0], tris }], lignes: Vec::new(), rendu: Vec::new(), motifs: Vec::new() });
    }
    for g in &projet.garde_corps {
        let id = id_natif("garde_corps", &g.id);
        let parent = id_natif("niveau", &g.niveau);
        model.parent.insert(id, parent);
        model.children.entry(parent).or_default().push(id);
        // couleur du matériau de chaque élément (métal sombre à défaut), vitrage translucide
        let parts: Vec<Part> = g.solides(projet)?.into_iter().map(|(materiau, vitre, t)| {
            let m = materiau.map(|n| projet.materiau(&n).cloned().unwrap_or_else(|| modele::murs::assemblage::materiau_par_defaut(&n)));
            let [r, v, b] = m.as_ref().map_or(if vitre { [158, 199, 219] } else { [77, 79, 84] }, |m| m.couleur).map(|c| c as f32 / 255.0);
            Part { color: [r, v, b, if vitre { 0.35 } else { 1.0 }], tris: t.into_iter().map(|t| t.map(|v| v * 0.001)).collect() }
        }).collect();
        materiaux_parts.push(vec![None; parts.len()]);
        geoms.push(ProductGeom { id, parts, lignes: Vec::new(), rendu: Vec::new(), motifs: Vec::new() });
    }
    // terrains : solide de l'épaisseur du type, sous la surface (cotes NGF ramenées à
    // l'origine du projet) ; couleur et poché du matériau du type
    for t in &projet.terrains {
        let id = id_natif("terrain", &t.id);
        model.parent.insert(id, racine);
        model.children.entry(racine).or_default().push(id);
        let type_terrain = projet.type_terrain(t);
        let tris = modele::elements::terre_plein::solide_terrain(projet, t, type_terrain.epaisseur)?.into_iter().map(|t| t.map(|v| v * 0.001)).collect();
        let du_gabarit = type_terrain.materiau.as_ref().filter(|n| projet.materiau(n).is_none()).map(|n| modele::murs::assemblage::materiau_par_defaut(n));
        let materiau = type_terrain.materiau.as_ref().and_then(|n| projet.materiau(n)).or(du_gabarit.as_ref());
        let couleur = materiau.map_or([0.48, 0.62, 0.38, 1.0], |m| { let [r, g, b] = m.couleur.map(|c| c as f32 / 255.0); [r, g, b, 1.0] });
        let couleur = opacite(materiau, couleur);
        if let Some(m) = materiau {
            let neutre = Vec4::new(0.0, 0.0, 0.0, 1.0);
            couches_coupe.push((id, coupe_materiau(projet, m, vec![neutre, neutre], noyau::aretes::TRAIT_FORT_MM)));
        }
        materiaux_parts.push(vec![materiau.map(|m| m.nom.clone())]);
        geoms.push(ProductGeom { id, parts: vec![Part { color: couleur, tris }], lignes: Vec::new(), rendu: vec![materiau.map(|m| rendu_materiau(m, couleur, &mut table_apparences))], motifs: vec![materiau.and_then(|m| motif_surface(projet, m, &mut table_motifs))] });
    }
    for l in &projet.limites_propriete {
        let id = id_natif("limite_propriete", &l.id);
        model.parent.insert(id, racine);
        model.children.entry(racine).or_default().push(id);
        let segs = (0..l.contour.len()).map(|i| [l.contour[i], l.contour[(i + 1) % l.contour.len()]].map(|p| glam::DVec3::new(p[0], p[1], l.altitude) * 0.001)).collect();
        materiaux_parts.push(Vec::new());
        geoms.push(ProductGeom { id, parts: Vec::new(), lignes: vec![crate::formats::ifc::geom::Ligne { color: [0.75, 0.16, 0.16, 1.0], segs }], rendu: Vec::new(), motifs: Vec::new() });
    }
    for o in &projet.importes {
        let id = id_natif("importe", &o.id);
        let parent = match &o.niveau {
            Some(n) if projet.niveau(n).is_some() => id_natif("niveau", n),
            Some(_) => return Err(format!("{} : niveau introuvable.", o.nom)),
            None => racine,
        };
        model.parent.insert(id, parent);
        model.children.entry(parent).or_default().push(id);
        let tris = modele::volumes::solides::maillage(&o.maillage)?.into_iter().map(|t| t.map(|v| v * 0.001)).collect();
        materiaux_parts.push(vec![None]);
        geoms.push(ProductGeom { id, parts: vec![Part { color: [0.65,0.68,0.72,1.0], tris }], lignes: Vec::new(), rendu: Vec::new(), motifs: Vec::new() });
    }
    chrono("scène : autres éléments", &mut t_etape);
    let mut aretes = aretes_jointes(&geoms, &materiaux_parts);
    chrono("scène : arêtes", &mut t_etape);
    // terrains : apparence lissée, sans arête
    for t in &projet.terrains {
        aretes.insert(id_natif("terrain", &t.id), aretes_terrain(projet, t));
    }
    // arbres : surfaces courbes, arêtes sur la silhouette seulement
    for a in &projet.arbres {
        let z0 = projet.niveau(&a.niveau).map_or(0.0, |n| n.altitude);
        let (t, h) = a.solides(z0);
        let tris: Vec<[glam::DVec3; 3]> = t.into_iter().chain(h).map(|t| t.map(|v| v * 0.001)).collect();
        aretes.insert(id_natif("arbre", &a.id), aretes_silhouette(&tris));
    }
    let mut scene = crate::formats::ifc::scene::assemble_avec(model, geoms, path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| projet.nom.clone()), aretes);
    chrono("scène : assemblage", &mut t_etape);
    for a in &projet.arbres {
        if let Some(&o) = scene.object_of.get(&id_natif("arbre", &a.id)) {
            let r = scene.objects[o as usize].opaque;
            lisser(&mut scene.vertices[r.start as usize..(r.start + r.count) as usize]);
        }
    }
    for t in &projet.terrains {
        if let Some(&o) = scene.object_of.get(&id_natif("terrain", &t.id)) {
            let r = scene.objects[o as usize].opaque;
            lisser(&mut scene.vertices[r.start as usize..(r.start + r.count) as usize]);
        }
    }
    scene.couches_coupe.extend(couches_coupe);
    scene.table_motifs = table_motifs.into_iter().map(|t| t.1).collect();
    // textures en chemin relatif : depuis le dossier du projet
    let dossier = path.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    scene.apparences = table_apparences
        .into_iter()
        .map(|(_, mut a)| {
            for t in a.textures.values_mut() {
                if std::path::Path::new(&t.chemin).is_relative() {
                    t.chemin = dossier.join(&t.chemin).to_string_lossy().into_owned();
                }
            }
            a
        })
        .collect();
    // plans des bandes ramenés au repère de la scène (origine retirée)
    let o = scene.origin;
    for (id, mut bandes) in bandes_dalles {
        for b in &mut bandes {
            for pl in &mut b.plans {
                pl.w += (pl.truncate().as_dvec3().dot(o)) as f32;
            }
        }
        scene.couches_coupe.insert(id, bandes);
    }
    if scene.objects.is_empty() { scene.min = Vec3::splat(-5.0); scene.max = Vec3::splat(5.0); }
    scene.navigateur = Some(navigateur(projet));
    chrono("scène : finitions", &mut t_etape);
    Ok(scene)
}

/// Normales lissées : chaque sommet prend la moyenne des normales des facettes qui le
/// partagent, à moins de 45° de la sienne (le dessus d'un terrain s'arrondit, ses
/// côtés restent nets).
fn lisser(sommets: &mut [Vertex]) {
    let cle = |v: &Vertex| v.pos.map(|x| (x * 1000.0).round() as i64);
    let normale = |v: &Vertex| Vec3::new(v.normal[0] as f32, v.normal[1] as f32, v.normal[2] as f32) / 127.0;
    let mut groupes: std::collections::HashMap<[i64; 3], Vec<usize>> = std::collections::HashMap::new();
    for (i, v) in sommets.iter().enumerate() {
        groupes.entry(cle(v)).or_default().push(i);
    }
    let normales: Vec<Vec3> = sommets.iter().map(normale).collect();
    for indices in groupes.values() {
        for &i in indices {
            let n = normales[i];
            let somme: Vec3 = indices.iter().map(|&j| normales[j]).filter(|m| m.dot(n) > std::f32::consts::FRAC_1_SQRT_2).sum();
            let m = somme.normalize_or(n);
            let v = &mut sommets[i];
            v.normal = [(m.x * 127.0) as i8, (m.y * 127.0) as i8, (m.z * 127.0) as i8, v.normal[3]];
        }
    }
}

/// Transparence de l'apparence (ou carte de transparence, de découpe) : la partie passe
/// avec les faces transparentes.
fn opacite(m: Option<&modele::Materiau>, mut c: [f32; 4]) -> [f32; 4] {
    use modele::CarteTexture;
    if let Some(a) = m.map(|m| &m.apparence) {
        c[3] = (1.0 - a.transparence).clamp(0.05, 1.0);
        // la découpe reste opaque (pixels retirés) : seule la transparence passe avec les
        // faces transparentes
        if c[3] > 0.98 && a.textures.contains_key(&CarteTexture::Transparence) {
            c[3] = 0.98;
        }
    }
    c
}

/// Couleur de rendu d'un matériau (style Réaliste). Un matériau dont l'apparence est
/// réglée y porte son rang dans la table des apparences, codé dans l'alpha (128 + rang,
/// au-delà de 1 : voir `assemble_avec`) ; sa couleur est alors la teinte de l'apparence.
fn rendu_materiau(m: &modele::Materiau, couleur: [f32; 4], table: &mut Vec<(String, modele::Apparence)>) -> [f32; 4] {
    if m.apparence.est_defaut() {
        return couleur;
    }
    let rang = match table.iter().position(|t| t.0 == m.nom) {
        Some(k) => k,
        None => {
            table.push((m.nom.clone(), m.apparence.clone()));
            table.len() - 1
        }
    };
    if rang >= noyau::constantes::MAX_APPARENCES {
        return couleur;
    }
    let [r, g, b] = m.apparence.teinte.unwrap_or(m.couleur).map(|c| c as f32 / 255.0);
    [r, g, b, 128.0 + rang as f32]
}

/// Motifs de surface d'un matériau pour le rendu : [premier plan | arrière-plan << 4,
/// couleur du premier plan]. Premier plan : 0 aucun, 15 uni, sinon rang + 1 dans la
/// table ; arrière-plan : 0 uni (couleur des faces), sinon rang + 1 dans la table.
fn motif_surface(projet: &Projet, m: &modele::Materiau, table: &mut Vec<(String, [f32; 4])>) -> Option<[u8; 4]> {
    use modele::MotifCoupe;
    let mut rang = |x: &MotifCoupe| -> u8 {
        let Some(d) = projet.definition_motif(x) else { return 0 };
        if let Some(k) = table.iter().position(|t| t.0 == d.nom) {
            return k as u8 + 1;
        }
        if table.len() >= 14 {
            return 1;
        }
        let f = |k: usize| d.lignes.get(k).map_or([0.0, 0.0], |l| [l.angle.to_radians() as f32, l.pas as f32]);
        let ([a, b], [c, e]) = (f(0), f(1));
        table.push((d.nom.clone(), [a, b, c, e]));
        table.len() as u8
    };
    let premier = match &m.surface_motif {
        None | Some(MotifCoupe::Vide) | Some(MotifCoupe::Isolation) => 0,
        Some(MotifCoupe::Uni) => 15,
        Some(x) => rang(x),
    };
    let arriere = match m.fond_surface() {
        MotifCoupe::Uni => 0,
        x => rang(&x),
    };
    if premier == 0 && arriere == 0 {
        return None;
    }
    let [r, g, b] = m.encre_surface();
    Some([premier | (arriere << 4), r, g, b])
}

/// Arêtes des éléments calculées par matériau : là où deux éléments d'un même matériau se
/// joignent (murs raccordés, dalle sous un mur), pas d'arête sur les faces qui se
/// prolongent ; les parties sans matériau restent à part.
fn aretes_jointes(geoms: &[ProductGeom], materiaux: &[Vec<Option<String>>]) -> std::collections::HashMap<Id, noyau::aretes::Aretes> {
    use std::collections::HashMap;
    let mut groupes: HashMap<String, Vec<([DVec3; 3], usize)>> = HashMap::new();
    for (k, (g, noms)) in geoms.iter().zip(materiaux).enumerate() {
        for (i, p) in g.parts.iter().enumerate() {
            let cle = noms.get(i).cloned().flatten().map_or_else(|| format!("\u{0}{k}:{i}"), |n| n);
            groupes.entry(cle).or_default().extend(p.tris.iter().map(|t| (*t, k)));
        }
    }
    let mut par_objet: HashMap<Id, noyau::aretes::Aretes> = geoms.iter().map(|g| (g.id, noyau::aretes::Aretes::default())).collect();
    let resultats: Vec<_> = groupes.into_par_iter().map(|(_, tris)| noyau::aretes::aretes_jointes(&tris)).collect();
    for (vives, douces) in resultats {
        for (s, k) in vives {
            if let Some(a) = par_objet.get_mut(&geoms[k].id) { a.vives.push(s); }
        }
        for (d, k) in douces {
            if let Some(a) = par_objet.get_mut(&geoms[k].id) { a.douces.push(d); }
        }
    }
    par_objet
}

/// Arborescence du projet natif : vue 3D, vues enregistrées (plans d'étage, coupes),
/// types de murs et de dalles, matériaux et éléments qui les portent.
fn navigateur(projet: &Projet) -> Navigateur {
    use modele::GenreVue;
    let num = numero_vue;
    let vue = |revit_id: i64, genre: &str, nom: &str, echelle: f64| NavVue {
        revit_id,
        phase: 0,
        filtre_phase: -1,
        coupe: None,
        limite_basse: None,
        plan_coupe: None,
        cadrage: Vec::new(),
        tableau: None,
        entite: None,
        genre: genre.into(),
        type_vue: String::new(),
        nom: nom.into(),
        echelle,
        detail: 2,
        numero: String::new(),
        haut: DVec3::Z,
        direction: DVec3::new(-1.0, -1.0, 1.0).normalize(),
        etage: None,
        placees: Vec::new(),
        fenetres: Vec::new(),
        primitives: Vec::new(),
    };
    let mut vues = vec![vue(num("3d"), "Vue 3D", "Vue 3D", 100.0)];
    for v in &projet.vues {
        let mut n = vue(num(&v.id), "", &v.nom, v.echelle);
        n.type_vue = v.type_vue.clone();
        match &v.genre {
            GenreVue::Plan { niveau, coupe, bas_niveau, bas_decalage } => {
                n.limite_basse = projet.niveau(bas_niveau).map(|l| (l.altitude + bas_decalage) / 1000.0);
                n.genre = "Plan d'étage".into();
                n.etage = Some(id_natif("niveau", niveau));
                n.coupe = Some(coupe / 1000.0);
                n.direction = DVec3::Z;
                let (sin, cos) = v.orientation_plan.sin_cos();
                n.haut = DVec3::new(-sin, cos, 0.0);
            }
            GenreVue::Coupe { debut, fin, profondeur, elevation } => {
                n.genre = if *elevation { "Élévation" } else { "Coupe" }.into();
                let (a, b) = (glam::DVec2::from_array(*debut) / 1000.0, glam::DVec2::from_array(*fin) / 1000.0);
                let u = (b - a).normalize_or_zero();
                let gauche = u.perp();
                // l'observateur est à droite de la ligne et regarde vers sa gauche
                n.direction = -gauche.extend(0.0);
                n.haut = DVec3::Z;
                n.plan_coupe = Some(gauche.extend(0.0).extend(-gauche.dot(a)));
                // profondeur, puis les deux bouts de la ligne
                let plan = |n: glam::DVec2, p: glam::DVec2| n.extend(0.0).extend(-n.dot(p));
                n.cadrage = vec![plan(-gauche, a + gauche * (profondeur / 1000.0)), plan(u, a), plan(-u, b)];
            }
            // vue caméra : vue 3D (point de vue appliqué à l'ouverture)
            GenreVue::Camera { oeil, cible, .. } => {
                n.genre = "Vue 3D".into();
                n.direction = -(glam::DVec3::from_array(*cible) - glam::DVec3::from_array(*oeil)).normalize_or(DVec3::Y);
            }
        }
        vues.push(n);
    }
    // jeux de feuilles : une vue par jeu (toutes ses feuilles à la suite) ; identifiant
    // tiré du rang du jeu, stable si on le renomme
    for (k, jeu) in modele::documentation::jeux(projet).iter().enumerate() {
        vues.push(vue(num(&format!("jeu:{k}")), "Jeu de feuilles", jeu, 1.0));
    }
    let types = |categorie: &str, nom: &str, ts: &[modele::TypeCompose], genre: &str| NavFamille {
        categorie: categorie.into(),
        nom: nom.into(),
        types: ts.iter().map(|t| (t.nom.clone(), Some(id_natif(genre, &t.id)))).collect(),
    };
    let mut familles = Vec::new();
    if !projet.types_murs.is_empty() {
        familles.push(types("Murs", "Mur de base", &projet.types_murs, "type-mur"));
    }
    // types d'éléments horizontaux rangés par catégorie de leurs éléments (sols par défaut)
    for (categorie, famille) in [("Sols", "Sol"), ("Plafonds", "Plafond composé"), ("Toits", "Toit de base"), ("Terre-pleins", "Terre-plein")] {
        let ts: Vec<modele::TypeCompose> = projet
            .types_dalles
            .iter()
            .filter(|t| projet.dalles.iter().find(|d| d.type_dalle == t.id).map_or("Sols", |d| d.sorte.categorie()) == categorie)
            .cloned()
            .collect();
        if !ts.is_empty() {
            familles.push(types(categorie, famille, &ts, "type-dalle"));
        }
    }
    let materiaux = projet
        .materiaux
        .iter()
        .map(|m| {
            let porte = |t: Option<&modele::TypeCompose>| t.is_some_and(|t| t.couches.iter().any(|c| c.materiau.as_deref() == Some(m.nom.as_str())));
            let mut elements: Vec<Id> = projet.murs.iter().filter(|x| porte(projet.type_mur(&x.type_mur))).map(|x| id_natif("mur", &x.id)).collect();
            elements.extend(projet.dalles.iter().filter(|x| porte(projet.type_dalle(&x.type_dalle))).map(|x| id_natif("dalle", &x.id)));
            NavMateriau { nom: m.nom.clone(), couleur: m.couleur, transparence: 0.0, elements }
        })
        .collect();
    Navigateur {
        phases: Vec::new(),
        filtres_phase: Default::default(),
        phases_objets: Default::default(),
        variantes_secondaires: Default::default(),
        vues,
        familles,
        groupes: Vec::new(),
        materiaux,
    }
}

/// Poché de coupe d'un matériau entre deux plans (neutres : tout l'élément) : premier
/// plan (hachures du projet, aplat ou ligne d'isolant) et arrière-plan (aplat ou
/// hachures), en mm du papier.
fn coupe_materiau(projet: &Projet, m: &modele::Materiau, plans: Vec<Vec4>, trait_mm: f64) -> Vec<CoucheCoupe> {
    use modele::MotifCoupe;
    use noyau::motif::{GrilleMotif, Motif};
    let [r, g, b] = m.couleur;
    // hachures d'une définition ; aplat : sans grille ; rien : None
    let grilles = |x: &MotifCoupe| -> Option<Vec<GrilleMotif>> {
        match x {
            MotifCoupe::Uni => Some(Vec::new()),
            MotifCoupe::Vide | MotifCoupe::Isolation => None,
            x => projet.definition_motif(x).map(|d| {
                d.lignes.iter().map(|l| GrilleMotif { angle: l.angle.to_radians(), origine: l.origine, glissement: l.glissement, decalage: l.pas, tirets: l.tirets.iter().map(|t| t.abs()).collect() }).collect()
            }),
        }
    };
    let motif = grilles(&m.motif);
    let [er, eg, eb] = m.encre_coupe();
    let (motif_fond, [fr, fg, fb]) = m.fond_coupe();
    vec![CoucheCoupe {
        plans,
        couleur: [r, g, b, 255],
        fond: [fr, fg, fb, 255],
        motif_fond: grilles(&motif_fond).map(|grilles| std::sync::Arc::new(Motif { nom: format!("{} (arrière-plan)", m.nom), modele: false, grilles })),
        motif: motif.map(|grilles| std::sync::Arc::new(Motif { nom: m.nom.clone(), modele: false, grilles })),
        couleur_motif: [er, eg, eb, 255],
        hachure_defaut: false,
        isolation: m.motif == MotifCoupe::Isolation,
        centre_isolation: None,
        trait_mm,
    }]
}

/// Traits d'un terrain en plan : périmètre de sa surface et courbes de niveau à un pas
/// rond (une douzaine au plus). La surface lissée n'a pas d'arête : sans ces traits, la
/// partie vue sous le plan de coupe (faces blanches) ne se distingue pas du papier.
/// Arêtes d'une surface courbe (arbre) : toutes « douces », tracées sur la silhouette
/// seulement ; bords libres en arêtes vives.
fn aretes_silhouette(tris: &[[glam::DVec3; 3]]) -> noyau::aretes::Aretes {
    use glam::DVec3;
    let cle = |p: DVec3| (p * 10_000.0).round().as_i64vec3().to_array();
    let mut bords: std::collections::HashMap<([i64; 3], [i64; 3]), ([DVec3; 2], Vec<DVec3>)> = std::collections::HashMap::new();
    for t in tris {
        let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
        for i in 0..3 {
            let (a, b) = (t[i], t[(i + 1) % 3]);
            let (ka, kb) = (cle(a), cle(b));
            let k = if ka < kb { (ka, kb) } else { (kb, ka) };
            bords.entry(k).or_insert(([a, b], Vec::new())).1.push(n);
        }
    }
    let mut out = noyau::aretes::Aretes::default();
    for (seg, normales) in bords.into_values() {
        match normales[..] {
            [n1, n2] => out.douces.push((seg, n1, n2)),
            _ => out.vives.push(seg),
        }
    }
    out
}

fn aretes_terrain(projet: &Projet, t: &modele::elements::site::Terrain) -> noyau::aretes::Aretes {
    use glam::DVec3;
    let decalage = DVec3::new(0.0, 0.0, -projet.ngf());
    let m = |p: DVec3| p * 0.001;
    let mut vives: Vec<[DVec3; 2]> = (0..t.contour.len()).map(|i| [t.contour[i], t.contour[(i + 1) % t.contour.len()]].map(|p| m(DVec3::from_array(p) + decalage))).collect();
    // dessus du terrain, creusé par les terre-pleins : faces tournées vers le haut du solide
    let dessus = if projet.dalles.iter().any(|d| d.sorte == SorteDalle::TerrePlein) {
        modele::elements::terre_plein::solide_terrain(projet, t, projet.type_terrain(t).epaisseur)
            .map(|v| v.into_iter().filter(|t| (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero().z > 0.05).collect::<Vec<_>>())
    } else {
        t.triangles().map(|v| v.into_iter().map(|t| t.map(|p| p + decalage)).collect())
    };
    if let Ok(tris) = dessus {
        let z = tris.iter().flatten().map(|p| p.z);
        let (bas, haut) = z.fold((f64::MAX, f64::MIN), |(a, b), z| (a.min(z), b.max(z)));
        if haut - bas > 1.0 {
            let etendue = haut - bas;
            let pas = [50.0, 100.0, 200.0, 250.0, 500.0, 1000.0, 2000.0, 5000.0, 10_000.0].into_iter().find(|p| etendue / p <= 12.0).unwrap_or(20_000.0);
            for tri in &tris {
                let zmax = tri.iter().map(|p| p.z).fold(f64::MIN, f64::max);
                let mut z = (tri.iter().map(|p| p.z).fold(f64::MAX, f64::min) / pas).ceil() * pas;
                while z < zmax {
                    let pts: Vec<DVec3> = (0..3).filter_map(|i| {
                        let (a, b) = (tri[i], tri[(i + 1) % 3]);
                        ((a.z - z) * (b.z - z) < 0.0).then(|| a + (b - a) * ((z - a.z) / (b.z - a.z)))
                    }).collect();
                    if let [a, b] = pts[..] {
                        vives.push([m(a), m(b)]);
                    }
                    z += pas;
                }
            }
        }
    }
    noyau::aretes::Aretes { vives, douces: Vec::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use modele::{ElementImporte, Maillage, Origine};

    /// Topographie importée de Revit : élément géographique, que les plans d'étage gardent
    /// (la plage de vue décide ensuite de ce qui est vu).
    #[test]
    fn topographie_revit_importee_est_geographique() {
        let mut p = Projet::nouveau("T");
        let tris = [[DVec3::ZERO, DVec3::new(1000.0, 0.0, 0.0), DVec3::new(0.0, 1000.0, 500.0)]];
        for (id, classe) in [("a", "Toposolid"), ("b", "SiteSurface"), ("c", "Wall")] {
            let origine = Some(Origine { source: "rvt".into(), id: id.into(), guid: None, classe: classe.into() });
            p.importes.push(ElementImporte { id: id.into(), nom: id.into(), categorie: "Autres".into(), niveau: None, maillage: Maillage::depuis_triangles(&tris), raison: String::new(), origine });
        }
        let s = depuis_natif(&p, std::path::Path::new("t.modele.json")).unwrap();
        let classe = |id: &str| &*s.objects[s.object_of[&id_natif("importe", id)] as usize].class;
        assert_eq!(classe("a"), "IFCGEOGRAPHICELEMENT");
        assert_eq!(classe("b"), "IFCGEOGRAPHICELEMENT");
        assert_eq!(classe("c"), "IFCBUILDINGELEMENTPROXY");
    }

    /// Terrain recoupant un mur : en plan, le clic sur le mur le sélectionne (le terrain
    /// n'est pris que faute d'autre objet) et le terrain porte ses traits (périmètre et
    /// courbes de niveau).
    #[test]
    fn terrain_sous_le_mur_et_ses_traits() {
        use modele::{creation::ParametresMur, elements::site::Terrain};
        let mut p = Projet::nouveau("T");
        p.ajouter_mur([0.0, 0.0], [5000.0, 0.0], &ParametresMur { epaisseur: 200.0, ..Default::default() }).unwrap();
        let z = |x: f64| 200.0 + x / 10.0;
        p.terrains.push(Terrain { id: "t".into(), nom: "Topo".into(), contour: vec![[-5000.0, -5000.0, z(-5000.0)], [10000.0, -5000.0, z(10000.0)], [10000.0, 5000.0, z(10000.0)], [-5000.0, 5000.0, z(-5000.0)]], points: vec![], type_terrain: None });
        let s = depuis_natif(&p, std::path::Path::new("t.modele.json")).unwrap();
        let visible = vec![true; s.objects.len()];
        let plan = glam::Vec4::new(0.0, 0.0, -1.0, 1.2 - s.origin.z as f32);
        let haut = glam::Vec3::new(2.5 - s.origin.x as f32, -s.origin.y as f32, 10.0);
        let (i, _) = s.pick(haut, glam::Vec3::NEG_Z, &visible, Some(plan), None).unwrap();
        assert_eq!(&*s.objects[i as usize].class, "IFCWALL");
        // hors du mur, le terrain reste sélectionnable
        let loin = glam::Vec3::new(-2.0 - s.origin.x as f32, 3.0 - s.origin.y as f32, 10.0);
        let (i, _) = s.pick(loin, glam::Vec3::NEG_Z, &visible, Some(plan), None).unwrap();
        assert_eq!(&*s.objects[i as usize].class, "IFCGEOGRAPHICELEMENT");
        let topo = s.objects.iter().find(|o| &*o.class == "IFCGEOGRAPHICELEMENT").unwrap();
        assert!(topo.aretes.count > 8, "{}", topo.aretes.count);
    }
}

