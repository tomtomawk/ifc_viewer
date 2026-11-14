//! Adaptation du modèle natif vers la chaîne de rendu commune.
use glam::{Vec3, Vec4};
use ifc_base::semantique::IfcModel;
use scene::*;
use modele::identifiants::id_natif;
use formats::ifc::geom::{Part, ProductGeom};
use modele::{Projet, SorteDalle};
use noyau::chrono::chrono;
use ifc_base::step::{Entity, StepFile, Value};
use std::path::Path;

mod aretes;
mod materiaux;
mod navigateur;

use aretes::{aretes_jointes, aretes_silhouette, aretes_terrain};
use materiaux::{coupe_materiau, motif_surface, opacite, rendu_materiau};
use navigateur::navigateur;

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
        geoms.push(ProductGeom { id, parts: Vec::new(), lignes: vec![formats::ifc::geom::Ligne { color: [0.75, 0.16, 0.16, 1.0], segs }], rendu: Vec::new(), motifs: Vec::new() });
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
    let mut scene = formats::ifc::assemblage::assemble_avec(model, geoms, path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| projet.nom.clone()), aretes);
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

#[cfg(test)]
mod tests;
