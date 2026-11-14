//! Lecture d'un fichier Revit en maquette : décodage des enregistrements, extraction des éléments.
use super::*;

impl Emprise {
    pub fn contient(&self, p: DVec3) -> bool {
        let dans = |t: &[DVec2; 3]| {
            let s = |a: DVec2, b: DVec2| (b - a).perp_dot(p.truncate() - a);
            let (u, v, w) = (s(t[0], t[1]), s(t[1], t[2]), s(t[2], t[0]));
            (u >= 0.0 && v >= 0.0 && w >= 0.0) || (u <= 0.0 && v <= 0.0 && w <= 0.0)
        };
        p.z >= self.zmin - 1.0 && p.z <= self.zmax + 1.0 && self.triangles.iter().any(dans)
    }
}

impl Trace {
    pub(super) fn lire(c: &Obj) -> Option<Trace> {
        let v = |name| c.get(name).and_then(vec3);
        match c.class_name() {
            "GLine" => Some(Trace::Ligne { origine: v("m_origin")? * FT, direction: v("m_dirVec")? * FT }),
            "GArc" => Some(Trace::Arc { centre: v("m_center")? * FT, rayon: reel(c, "m_radius")? * FT, x: v("m_xVec")?, y: v("m_yVec")? }),
            _ => None,
        }
    }

    /// Point de paramètre t, en mm.
    pub fn point(&self, t: f64) -> DVec3 {
        match *self {
            Trace::Ligne { origine, direction } => origine + t * direction,
            Trace::Arc { centre, rayon, x, y } => centre + rayon * (t.cos() * x + t.sin() * y),
        }
    }
}

/// Lit le schéma, la table des éléments et les partitions, puis les éléments et leur
/// géométrie.
pub fn lire(path: &Path, progress: &dyn Fn(String)) -> Result<Maquette, String> {
    let mut cfb = Cfb::open(path)?;
    // document de famille : flux PartAtom (un projet ou un gabarit a ProjectInformation)
    let est_famille = cfb.entries().iter().any(|(p, dir, _)| !dir && p == "/PartAtom");
    let raw = cfb.read("Formats/Latest")?.ok_or("flux Formats/Latest absent")?;
    let sch = Schema::parse(&inflate(&unpad(&raw)).map_err(|e| format!("Formats/Latest : {e}"))?)?;
    let ids8 = sch.ids8();
    let table = elem_table(&cfb.read("Global/ElemTable")?.ok_or("flux Global/ElemTable absent")?)?;
    // épisodes de création : UniqueId et IfcGUID (règle vérifiée sur les formats 2024 et
    // suivants, ElemTable de 40 octets)
    let episodes = if ids8 { cfb.read("Global/History")?.and_then(|h| episodes(&h).ok()).unwrap_or_default() } else { Vec::new() };
    let sortes: HashMap<u16, Sorte> = sch.classes.iter().filter_map(|c| Some((c.id, Sorte::de(&sch, c.id, &c.name)?))).collect();

    let mut parts: Vec<(u64, String)> = cfb
        .entries()
        .into_iter()
        .filter(|(p, dir, _)| !dir && p.starts_with("/Partitions/"))
        .filter_map(|(p, _, _)| Some((p.rsplit('/').next()?.parse().ok()?, p)))
        .collect();
    parts.sort();

    // types (Symbol) : leur GElement porte la géométrie des GInstance (familles, marches
    // d'escalier assemblé `StairsTriserSymbol`…)
    let classes_types: std::collections::HashSet<u16> = sch.classes.iter().filter(|c| sch.inherits(c.id, "Symbol")).map(|c| c.id).collect();
    let mut types_famille: std::collections::HashSet<i64> = Default::default();
    let mut objets: HashMap<i64, (Sorte, Vec<u8>)> = HashMap::new();
    // textes, étiquettes et polices des documents de famille embarqués (hors ElemTable)
    let marqueurs = sch.by_name("ContentMarker").zip(sch.by_name("ContentKey")).map(|(a, b)| (a.id, b.id));
    let classes_embarquees: std::collections::HashSet<u16> = ["TextNote", "TagNote", "FontElem", "ParamElemFamily", "FilledRegion", "FilledRegionAttributes", "FillPatternElem", "GStyleElem", "TextNoteAttributes", "TagNoteAttributes"].iter().filter_map(|n| sch.by_name(n)).map(|c| c.id).collect();
    let mut embarques: HashMap<i64, ([u8; 16], Vec<u8>)> = HashMap::new();
    let classe_region = sch.by_name("FilledRegion").map(|c| c.id);
    let mut regions_embarquees: std::collections::HashSet<i64> = Default::default();
    let mut complements: HashMap<i64, Vec<u8>> = HashMap::new();
    let mut geometries_types: HashMap<i64, Vec<u8>> = HashMap::new();
    for (k, (_, p)) in parts.iter().enumerate() {
        progress(format!("Partition {} / {} ({p})…", k + 1, parts.len()));
        let raw = cfb.read(p)?.ok_or_else(|| format!("flux {p} illisible"))?;
        let (sec, debuts) = sections_documents(&raw, marqueurs).map_err(|e| format!("{p} : {e}"))?;
        let vide = Vec::new();
        let donnees = sec.get(&0x66).unwrap_or(&vide);
        let debuts = debuts.get(&0x66).map(Vec::as_slice).unwrap_or_default();
        for (eid, body) in records(donnees, true, ids8).map_err(|e| format!("{p}, section 0x66 : {e}"))? {
            if !table.contains_key(&eid) {
                if let (Some(c), Some(doc)) = (champs::class_id(body), document_de(debuts, body.as_ptr() as usize - donnees.as_ptr() as usize)) {
                    if classes_embarquees.contains(&c) {
                        embarques.insert(eid, (doc, body.to_vec()));
                        if Some(c) == classe_region {
                            regions_embarquees.insert(eid);
                        }
                    }
                }
                continue;
            }
            let Some(c) = champs::class_id(body) else { continue };
            if classes_types.contains(&c) {
                types_famille.insert(eid);
            }
            if let Some(&s) = sortes.get(&c).filter(|s| est_famille || !s.de_famille()) {
                objets.insert(eid, (s, body.to_vec()));
            }
        }
        for (eid, body) in records(sec.get(&0x67).unwrap_or(&vide), true, ids8).map_err(|e| format!("{p}, section 0x67 : {e}"))? {
            if objets.get(&eid).is_some_and(|o| o.0.a_geometrie()) {
                complements.insert(eid, body.to_vec());
            } else if types_famille.contains(&eid) {
                geometries_types.insert(eid, body.to_vec());
            } else if regions_embarquees.contains(&eid) {
                complements.insert(eid, body.to_vec());
            }
        }
    }

    // Les niveaux d'abord (altitudes), puis chaque objet est lu et réduit aussitôt à ses
    // champs utiles : garder tous les enregistrements lus coûtait 6 Go à Saint-Malo.
    progress(format!("Lecture de {} objets…", objets.len()));
    let mut ids: Vec<i64> = objets.keys().copied().collect();
    ids.sort_unstable();
    let lire_objet = |eid: &i64| -> Option<Result<(Sorte, Record), (i64, String)>> {
        let (s, body) = &objets[eid];
        match champs::read(&sch, body) {
            Ok(r) if place(&r.root) => Some(Ok((*s, r))),
            Ok(_) => None,
            Err(e) => Some(Err((*eid, e))),
        }
    };
    let mut illisibles = Vec::new();
    let mut niveaux = Vec::new();
    let mut parametres: HashMap<i64, Vec<(i64, Brut)>> = HashMap::new();
    for eid in ids.iter().filter(|e| objets[e].0 == Sorte::Niveau) {
        match lire_objet(eid) {
            Some(Ok((_, r))) => {
                niveaux.extend(niveau(*eid, &r));
                parametres.insert(*eid, parametres::valeurs(&r));
            }
            Some(Err(e)) => illisibles.push(e),
            None => {}
        }
    }
    niveaux.sort_by(|a, b| a.altitude.total_cmp(&b.altitude).then(a.id.cmp(&b.id)));
    let altitude: HashMap<i64, f64> = niveaux.iter().map(|n| (n.id, n.altitude)).collect();
    let herite = |c: &str, base: &str| sch.by_name(c).is_some_and(|c| sch.inherits(c.id, base));
    let extraits: Vec<Result<(i64, Extrait, Vec<(i64, Brut)>, (Option<i64>, Option<i64>)), (i64, String)>> = ids
        .par_iter()
        .filter(|e| objets[e].0 != Sorte::Niveau)
        .filter_map(|eid| Some(lire_objet(eid)?.map(|(s, r)| (*eid, extraire(*eid, s, &r, &altitude, &herite), parametres::valeurs(&r), (crate::rvt::annexes::groupe_de(&r), crate::rvt::annexes::variante_de(&r))))))
        .collect();
    drop(objets);

    let (mut types, mut familles, mut materiaux) = (HashMap::new(), HashMap::new(), HashMap::new());
    let (mut murs, mut dalles, mut toits, mut escaliers, mut grilles, mut pieces) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut toposolides, mut surfaces_topo) = (Vec::new(), Vec::new());
    let (mut volees, mut instances, mut instances_de_vue, mut garde_corps) = (HashMap::new(), Vec::new(), 0, Vec::new());
    let (mut formes, mut categories) = (Vec::new(), HashMap::new());
    let (mut vues, mut cadrages, mut elements_2d) = (Vec::new(), HashMap::new(), Vec::new());
    let (mut motifs, mut types_regions, mut couleurs_categories) = (HashMap::new(), HashMap::new(), HashMap::new());
    let (mut definitions, mut phases) = (HashMap::new(), HashMap::new());
    let mut coord = Coordonnees::default();
    let (mut ordre_phases, mut filtres_phase) = (Vec::new(), HashMap::new());
    let mut unites = parametres::Unites::default();
    let mut lignes_zones: HashMap<(i64, i64), Vec<(DVec2, DVec2)>> = HashMap::new();
    let mut topologies: Vec<crate::rvt::zones::Topologie> = Vec::new();
    let (mut plans_reference, mut plans_construction, mut apparences) = (Vec::new(), Vec::new(), HashMap::new());
    let (mut rasters, mut types_images, mut images) = (HashMap::new(), Vec::new(), Vec::new());
    let (mut types_imports, mut imports, mut types_liens, mut liens) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut groupes, mut legendes, mut contraintes, mut motifs_ligne) = (Vec::new(), Vec::new(), Vec::new(), HashMap::new());
    let (mut epaisseurs, mut tables_cles, mut objets_nommes) = (Vec::new(), Vec::new(), Vec::new());
    let mut variantes: HashMap<i64, crate::rvt::annexes::Variante> = HashMap::new();
    let mut liaisons: HashMap<i64, (Vec<i64>, bool)> = HashMap::new();
    let (mut instances_vue, mut polices, mut types_fenetres, mut revisions, mut info_projet) = (Vec::new(), HashMap::new(), HashMap::new(), HashMap::new(), -1);
    let (mut types_cotes, mut format_longueurs) = (HashMap::new(), None);
    let mut lignes_esquisse: std::collections::HashSet<i64> = Default::default();
    let mut groupe_de: HashMap<i64, i64> = HashMap::new();
    let mut variante_de: HashMap<i64, i64> = HashMap::new();
    for x in extraits {
        let x = x.map(|(eid, x, vals, (groupe, variante))| {
            if !vals.is_empty() {
                parametres.insert(eid, vals);
            }
            if let Some(g) = groupe {
                groupe_de.insert(eid, g);
            }
            if let Some(o) = variante {
                variante_de.insert(eid, o);
            }
            x
        });
        match x {
            Err(e) => illisibles.push(e),
            Ok(Extrait::Rien) => {}
            Ok(Extrait::Type(id, t)) => drop(types.insert(id, t)),
            Ok(Extrait::Famille(id, f)) => drop(familles.insert(id, f)),
            Ok(Extrait::Materiau(id, m)) => drop(materiaux.insert(id, m)),
            Ok(Extrait::Mur(m)) => murs.push(m),
            Ok(Extrait::Dalle(d)) => dalles.push(d),
            Ok(Extrait::Toposolide(d)) => toposolides.push(d),
            Ok(Extrait::SurfaceTopo(x)) => surfaces_topo.push(x),
            Ok(Extrait::Toit(t)) => toits.push(t),
            Ok(Extrait::Escalier(e)) => escaliers.push(e),
            Ok(Extrait::Volee(id, esc)) => drop(volees.insert(id, esc)),
            Ok(Extrait::GardeCorps(g)) => garde_corps.push(g),
            Ok(Extrait::Grille(g)) => grilles.push(g),
            Ok(Extrait::Instance(i, h)) => instances.push((i, h)),
            Ok(Extrait::InstanceDeVue(i)) => {
                instances_de_vue += 1;
                instances_vue.push(i);
            }
            Ok(Extrait::Piece(p)) => pieces.push(p),
            Ok(Extrait::Topologie(t)) => topologies.push(t),
            Ok(Extrait::PlanReference(p)) => plans_reference.push(p),
            Ok(Extrait::PlanConstruction(p)) => plans_construction.push(p),
            Ok(Extrait::Apparence(id, a)) => drop(apparences.insert(id, a)),
            Ok(Extrait::Annexe(x)) => {
                use crate::rvt::annexes::Annexe;
                match x {
                    Annexe::Variante(v) => drop(variantes.insert(v.id, v)),
                    Annexe::Liaison(l) => {
                        let e = liaisons.entry(l.param).or_insert((Vec::new(), l.occurrence));
                        e.0.push(l.categorie);
                    }
                    Annexe::Groupe(g) => groupes.push(g),
                    Annexe::Legende(l) => legendes.push(l),
                    Annexe::Contrainte(c) => contraintes.push(c),
                    Annexe::Motif(mo) => drop(motifs_ligne.insert(mo.id, mo)),
                    Annexe::Epaisseurs(e) => epaisseurs.push(e),
                    Annexe::Cles(c) => tables_cles.push(c),
                    Annexe::Nomme(n) => objets_nommes.push(n),
                }
            }
            Ok(Extrait::Media(x)) => {
                use crate::rvt::medias::Media;
                // les types gardent aussi leur nom parmi les types (nom des éléments)
                let mut type_nomme = |id: i64, nom: &str, classe: &str| {
                    types.insert(id, TypeElem { nom: nom.to_string(), classe: classe.into(), epaisseur: None, couches: None, couches_variables: false, famille: -1, largeur: None, hauteur: None, categorie_texte: -1, origine_insertion: None });
                };
                match x {
                    Media::Raster(id, r) => drop(rasters.insert(id, r)),
                    Media::TypeImage(t) => {
                        type_nomme(t.id, &t.nom, "ImageSymbol");
                        types_images.push(t)
                    }
                    Media::InstanceImage(i) => images.push(i),
                    Media::TypeImport(t) => {
                        type_nomme(t.id, &t.nom, "MasterImportSymbol");
                        types_imports.push(t)
                    }
                    Media::InstanceImport(i) => imports.push(i),
                    Media::TypeLien(t) => {
                        type_nomme(t.id, &t.nom, "RvtLinkSymbol");
                        types_liens.push(t)
                    }
                    Media::InstanceLien(i) => liens.push(i),
                }
            }
            Ok(Extrait::LigneZone(sc, niv, a, b)) => lignes_zones.entry((sc, niv)).or_insert_with(Vec::new).push((a, b)),
            Ok(Extrait::Forme(f)) => formes.push(f),
            Ok(Extrait::Categorie(id, nom)) => drop(categories.insert(id, nom)),
            Ok(Extrait::Vue(v)) => vues.push(v),
            Ok(Extrait::Cadrage(v, c, k)) => drop(cadrages.insert(v, (c, k))),
            Ok(Extrait::Element2D(e)) => elements_2d.push(e),
            Ok(Extrait::Motif(id, x)) => drop(motifs.insert(id, x)),
            Ok(Extrait::TypeRegion(id, x)) => drop(types_regions.insert(id, x)),
            Ok(Extrait::Style(cat, c)) => drop(couleurs_categories.insert(cat, c)),
            Ok(Extrait::Parametre(id, d)) => drop(definitions.insert(id, d)),
            Ok(Extrait::Phase(id, nom)) => drop(phases.insert(id, nom)),
            Ok(Extrait::OrdrePhases(l)) => ordre_phases = l,
            Ok(Extrait::FiltrePhase(id, nom, p)) => drop(filtres_phase.insert(id, (nom, p))),
            Ok(Extrait::Coordonnee(x)) => match x {
                coordonnees::Piece::Emplacement(e) => coord.emplacements.push(e),
                coordonnees::Piece::Lieu(id, l) => drop(coord.lieux.insert(id, l)),
                coordonnees::Piece::Point(true, p) => coord.point_topo = Some(p),
                coordonnees::Piece::Point(false, p) => coord.point_base = Some(p),
                coordonnees::Piece::Actif(a, p) => (coord.actif, coord.projet) = (a, p),
            },
            Ok(Extrait::Police(t, x)) => drop(polices.insert(t, x)),
            Ok(Extrait::TypeFenetre(id, x)) => drop(types_fenetres.insert(id, x)),
            Ok(Extrait::Revision(id, x)) => drop(revisions.insert(id, x)),
            Ok(Extrait::InfoProjet(id)) => info_projet = id,
            Ok(Extrait::TypeCote(id, x)) => drop(types_cotes.insert(id, x)),
            Ok(Extrait::Unites(f, u)) => {
                format_longueurs = f;
                unites = u;
            }
            Ok(Extrait::Esquisse(l)) => lignes_esquisse.extend(l),
        }
    }
    let famille_document = if est_famille { familles.iter().filter(|(_, f): &(&i64, &Famille)| f.parent == -1).map(|(&id, _)| id).min() } else { None };
    // vues et éléments 2D du document : ceux des familles imbriquées rangées en ligne
    // dans un .rfa (symboles d'annotation) sont écartés
    let du_document = |f: i64| f == -1 || familles.get(&f).is_some_and(|x: &Famille| x.parent == -1);
    vues.retain(|v: &Vue| du_document(v.famille));
    for v in &mut vues {
        if let Some((c, k)) = cadrages.remove(&v.id) {
            v.cadrage = Some(c);
            v.cadre = k;
        }
    }
    vues.sort_by_key(|v| v.id);
    let ids_vues: std::collections::HashSet<i64> = vues.iter().map(|v| v.id).collect();
    // les lignes d'une esquisse (contour d'une région remplie…) ne sont pas dans la vue
    // (absentes du collecteur de la vue dans l'API)
    elements_2d.retain(|e: &Element2D| du_document(e.famille) && ids_vues.contains(&e.vue) && !(e.classe == "CurveElem" && lignes_esquisse.contains(&e.id)));
    elements_2d.sort_by_key(|e| e.id);
    // formes affichées : hors combinaisons (le solide est celui de la combinaison) et
    // hors familles imbriquées (symboles d'annotation)
    let membres: std::collections::HashSet<i64> = formes.iter().flat_map(|f: &Forme| f.membres.iter().copied()).collect();
    formes.retain(|f| !membres.contains(&f.id) && (f.famille == -1 || familles.get(&f.famille).is_some_and(|x: &Famille| x.parent == -1)));
    // volées et paliers rattachés par m_stairsId ; point des instances hébergées par un mur
    for e in &mut escaliers {
        let mut parties: BTreeSet<i64> = e.parties.iter().copied().collect();
        parties.extend(volees.iter().filter(|&(_, &s)| s == e.id).map(|(&v, _)| v));
        parties.retain(|p| volees.contains_key(p));
        e.parties = parties.into_iter().collect();
    }
    let traces: HashMap<i64, Trace> = murs.iter().filter_map(|m| Some((m.id, m.trace?))).collect();
    let instances: Vec<Instance> = instances
        .into_iter()
        .map(|(mut i, hote)| {
            if let (Some(t), Some((h, elevation))) = (traces.get(&i.hote), hote) {
                let q = t.point(h);
                i.point = DVec3::new(q.x, q.y, altitude.get(&i.niveau).copied().unwrap_or(0.0) + elevation);
            }
            i
        })
        .collect();

    // textes, étiquettes et polices des documents de famille embarqués
    let mut textes_familles: HashMap<[u8; 16], Vec<Element2D>> = HashMap::new();
    let mut params_embarques: HashMap<i64, String> = HashMap::new();
    let mut categories_textes: HashMap<i64, i64> = HashMap::new();
    let mut ids_embarques: Vec<&i64> = embarques.keys().collect();
    ids_embarques.sort_unstable();
    for eid in ids_embarques {
        let (doc, body) = &embarques[eid];
        let Ok(r) = champs::read(&sch, body) else { continue };
        if r.root.class_name() == "FontElem" {
            if let Extrait::Police(t, x) = extraire(*eid, Sorte::Police, &r, &altitude, &herite) {
                polices.insert(t, x);
            }
        } else if r.root.class_name() == "ParamElemFamily" {
            if let Some(d) = parametres::definition(&r, &herite) {
                params_embarques.insert(*eid, d.nom);
            }
        } else if matches!(r.root.class_name(), "TextNoteAttributes" | "TagNoteAttributes") {
            if let Some(a) = r.root.get("m_lineAndTextAttr").and_then(Val::obj) {
                categories_textes.insert(*eid, id_of(a, "m_categoryId"));
            }
        } else if r.root.class_name() == "FilledRegionAttributes" {
            types_regions.extend(vues::type_region(&r).map(|x| (*eid, x)));
        } else if r.root.class_name() == "FillPatternElem" {
            motifs.extend(vues::motif(&r).map(|x| (*eid, x)));
        } else if r.root.class_name() == "GStyleElem" {
            if let Extrait::Style(cat, c) = extraire(*eid, Sorte::Style, &r, &altitude, &herite) {
                couleurs_categories.insert(cat, c);
            }
        } else if let Some(e) = vues::element(*eid, &r) {
            textes_familles.entry(*doc).or_default().push(e);
        }
    }
    drop(embarques);
    // dessin 2D des types placés dans une vue et des familles de titre de vue
    let mut types_2d: Vec<i64> = instances_vue.iter().map(|i: &InstanceVue| i.type_id).chain(types_fenetres.values().map(|t: &TypeFenetre| t.famille_titre)).collect();
    types_2d.sort_unstable();
    types_2d.dedup();
    let bib_2d = geometrie::Bibliotheque::dessins(&sch, &geometries_types);
    let dessins: HashMap<i64, geometrie::Dessin> = types_2d.par_iter().filter_map(|&t| Some((t, bib_2d.dessin(t)?))).collect();
    drop(bib_2d);

    // Géométrie, en parallèle.
    let mut a_mailler: Vec<i64> = murs.iter().map(|m| m.id).chain(dalles.iter().map(|d| d.id)).chain(toits.iter().map(|t| t.id)).collect();
    a_mailler.extend(toposolides.iter().map(|t| t.id).chain(surfaces_topo.iter().map(|x| x.id)));
    a_mailler.extend(escaliers.iter().flat_map(|e| e.parties.iter().copied()));
    a_mailler.extend(garde_corps.iter().map(|g| g.id));
    a_mailler.extend(instances.iter().map(|i| i.id).chain(pieces.iter().map(|p| p.id)));
    a_mailler.extend(formes.iter().map(|f| f.id));
    a_mailler.extend(elements_2d.iter().filter(|e| matches!(e.contenu, Contenu::Region { .. })).map(|e| e.id));
    let regions_familles: Vec<i64> = textes_familles.values().flatten().filter(|e| matches!(e.contenu, Contenu::Region { .. })).map(|e| e.id).collect();
    a_mailler.extend(&regions_familles);
    progress(format!("Géométrie de {} éléments…", a_mailler.len()));
    progress(format!("Géométrie de {} types ({} Mo)…", geometries_types.len(), geometries_types.values().map(Vec::len).sum::<usize>() >> 20));
    let bib = geometrie::Bibliotheque::new(&sch, &geometries_types);
    let mut regions: std::collections::HashSet<i64> = elements_2d.iter().filter(|e| matches!(e.contenu, Contenu::Region { .. })).map(|e| e.id).collect();
    regions.extend(regions_familles);
    // une région remplie est une face ouverte (feuille), pas un solide
    let mut geoms: HashMap<i64, Geometrie> = a_mailler.par_iter().map(|&eid| (eid, maillage(&sch, &complements, &bib, eid, regions.contains(&eid)))).collect();
    let mut prendre = |eid: i64| geoms.remove(&eid).unwrap_or_else(|| Err("non maillé".into()));

    let mut m = Maquette {
        niveaux,
        types,
        familles,
        materiaux,
        murs,
        dalles,
        toposolides,
        surfaces_topo,
        toits,
        escaliers,
        garde_corps,
        grilles,
        instances,
        pieces,
        formes,
        categories,
        vues,
        elements_2d,
        motifs,
        types_regions,
        couleurs_categories,
        definitions,
        parametres,
        phases,
        unites,
        categorie_de: HashMap::new(),
        emprises: Vec::new(),
        plans_reference,
        plans_construction,
        apparences,
        rasters,
        types_images,
        images,
        types_imports,
        imports,
        types_liens,
        liens,
        groupes,
        groupe_de,
        variantes,
        variante_de,
        liaisons,
        legendes,
        contraintes,
        motifs_ligne,
        epaisseurs,
        tables_cles,
        objets_nommes,
        type_de_element: HashMap::new(),
        ordre_phases,
        filtres_phase,
        coordonnees: {
            coord.emplacements.sort_by_key(|e| e.id);
            coord
        },
        famille_document,
        episodes,
        episode_de: if ids8 { table } else { HashMap::new() },
        instances_de_vue,
        instances_vue,
        dessins,
        textes_familles,
        polices,
        params_embarques,
        categories_textes,
        types_fenetres,
        revisions,
        info_projet,
        types_cotes,
        format_longueurs,
        lignes_esquisse,
        illisibles,
    };
    for x in &mut m.murs {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.dalles {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.toposolides {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.surfaces_topo {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.toits {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.instances {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.garde_corps {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.pieces {
        x.geometrie = prendre(x.id);
        // surface et périmètre calculés par Revit (ROOM_AREA, ROOM_PERIMETER) : face
        // inférieure du volume de la pièce
        if let Ok(g) = &x.geometrie {
            let (aire, perimetre) = pied_de_piece(g);
            let v = m.parametres.entry(x.id).or_default();
            v.retain(|(p, _)| *p != -1_006_902 && *p != -1_006_917);
            v.push((-1_006_902, Brut::Reel(aire / (FT * FT))));
            v.push((-1_006_917, Brut::Reel(perimetre / FT)));
        }
    }
    // surface et périmètre des zones de surface (ROOM_AREA, ROOM_PERIMETER) : face des
    // lignes de zone de leur schéma et de leur niveau qui contient leur point
    // (circuit de la topologie du niveau, sinon arrangement des lignes) ; pièce : surface
    // de son circuit, périmètre de son maillage (celui du circuit en diffère)
    let topologie_de: HashMap<i64, &crate::rvt::zones::Topologie> = topologies.iter().flat_map(|t| t.pieces.iter().map(move |&p| (p, t))).collect();
    for x in m.pieces.iter().filter(|x| x.genre != "zone") {
        let mesure = topologie_de.get(&x.id).and_then(|t| t.mesure(x.circuit)).map(|(a, p)| (a, p * FT));
        let mesure = match (mesure, x.genre) {
            (Some(m), _) => Some(m),
            (None, "surface") => (|| {
                let (a, p) = crate::rvt::zones::surface(lignes_zones.get(&(x.schema, x.niveau))?, x.point?.truncate())?;
                Some((a / (FT * FT), p))
            })(),
            _ => None,
        };
        // pièce ou zone non placée : surface nulle, niveau « Non placée » (ROOM_LEVEL_ID)
        if x.point.is_none() {
            let v = m.parametres.entry(x.id).or_default();
            v.retain(|(p, _)| *p != -1_006_902 && *p != -1_006_916);
            v.push((-1_006_902, Brut::Reel(0.0)));
            v.push((-1_006_916, Brut::Texte("Non placée".into())));
            continue;
        }
        let Some((aire, perimetre)) = mesure else { continue };
        let v = m.parametres.entry(x.id).or_default();
        v.retain(|(p, _)| *p != -1_006_902 && (x.genre == "piece" || *p != -1_006_917));
        v.push((-1_006_902, Brut::Reel(aire)));
        if x.genre == "surface" {
            v.push((-1_006_917, Brut::Reel(perimetre / FT)));
        }
    }
    // élévation des niveaux (LEVEL_ELEV) depuis la base de leur type
    // (`LevelAttributes.m_baseElevation`, LEVEL_RELATIVE_BASE_TYPE) : 0 point de base du
    // projet, 1 point topographique (cote partagée)
    for n in &m.niveaux {
        let base = m.parametres.get(&n.type_id).and_then(|v| v.iter().find(|(p, _)| *p == -1_007_109)).map(|(_, b)| b.clone());
        let p = DVec3::new(0.0, 0.0, n.altitude);
        let elevation = match base {
            Some(Brut::Entier(1)) => m.coordonnees.partage(p).map(|q| q.z),
            _ => Some(n.altitude - m.coordonnees.point_base.map(|b| b.z).unwrap_or(0.0)),
        };
        if let (Some(e), Some(v)) = (elevation, m.parametres.get_mut(&n.id)) {
            v.retain(|(p, _)| *p != -1_007_102);
            v.push((-1_007_102, Brut::Reel(e / FT)));
        }
    }
    // emprises des pièces de la dernière phase (pièces de départ et d'arrivée)
    let derniere = m.ordre_phases.last().copied();
    let emprises = m
        .pieces
        .iter()
        .filter(|x| x.genre == "piece" && (derniere.is_none() || Some(x.phase) == derniere))
        .filter_map(|x| Some(emprise(x.id, x.geometrie.as_ref().ok()?)))
        .collect();
    m.emprises = emprises;
    for x in &mut m.formes {
        x.geometrie = prendre(x.id);
    }
    for x in m.elements_2d.iter_mut().chain(m.textes_familles.values_mut().flatten()).filter(|e| matches!(e.contenu, Contenu::Region { .. })) {
        x.geometrie = prendre(x.id);
    }
    for x in &mut m.escaliers {
        let mut tout: Option<Maillage> = None;
        let mut erreur = String::from("aucune volée ni palier");
        for &p in &x.parties {
            match prendre(p) {
                Ok(g) => match &mut tout {
                    Some(t) => t.ajouter(g),
                    None => tout = Some(g),
                },
                Err(e) => erreur = format!("partie {p} : {e}"),
            }
        }
        x.geometrie = tout.ok_or(erreur);
    }
    m.indexer();
    Ok(m)
}

/// Maillage du complément 0x67 d'un élément, géométrie de ses familles comprise ;
/// `feuilles` : garder aussi les faces ouvertes (régions remplies).
pub(super) fn maillage(sch: &Schema, complements: &HashMap<i64, Vec<u8>>, bib: &geometrie::Bibliotheque, eid: i64, feuilles: bool) -> Geometrie {
    let b = complements.get(&eid).ok_or_else(|| format!("élément {eid} sans enregistrement 0x67"))?;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| champs::read(sch, b).and_then(|rec| geometrie::maillage(&rec, feuilles, bib))))
        .unwrap_or_else(|_| Err("erreur interne".into()))
        .and_then(|g| if g.faces.is_empty() { Err("aucun solide fermé dans le GElement".into()) } else { Ok(g) })
}

/// Surface (mm²) et périmètre (mm) d'une pièce : triangles horizontaux tournés vers le
/// bas au plus bas du volume, et longueur de leurs arêtes non partagées.
pub(super) fn pied_de_piece(g: &Maillage) -> (f64, f64) {
    let zmin = g.sommets.iter().map(|p| p.z).fold(f64::MAX, f64::min);
    let mut aire = 0.0;
    let mut aretes: HashMap<(usize, usize), usize> = HashMap::new();
    for f in &g.faces {
        for t in &f.triangles {
            let [a, b, c] = t.map(|i| g.sommets[i]);
            let n = (b - a).cross(c - a);
            if n.z < 0.0 && n.z.abs() > 0.999 * n.length() && (a.z - zmin).abs() < 1.0 {
                aire += n.length() / 2.0;
                for (i, j) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    *aretes.entry((i.min(j), i.max(j))).or_default() += 1;
                }
            }
        }
    }
    let perimetre = aretes.iter().filter(|e| *e.1 == 1).map(|e| g.sommets[e.0.0].distance(g.sommets[e.0.1])).sum();
    (aire, perimetre)
}

/// Texte de « Est une référence » d'un plan de référence (codes relevés dans Revit 2026,
/// `rvt-reference-plans-apparences`) ; code brut pour un code non relevé.
pub(super) fn texte_reference(code: i64) -> String {
    match code {
        0 => "Gauche".into(),
        1 => "Centre (gauche/droite)".into(),
        2 => "Droite".into(),
        6 => "Bas".into(),
        8 => "Haut".into(),
        12 => "Pas référence".into(),
        14 => "Référence moins importante".into(),
        c => format!("code {c}"),
    }
}

pub(super) fn emprise(piece: i64, g: &Maillage) -> Emprise {
    let zmin = g.sommets.iter().map(|p| p.z).fold(f64::MAX, f64::min);
    let zmax = g.sommets.iter().map(|p| p.z).fold(f64::MIN, f64::max);
    let mut triangles = Vec::new();
    for f in &g.faces {
        for t in &f.triangles {
            let [a, b, c] = t.map(|i| g.sommets[i]);
            let n = (b - a).cross(c - a);
            if n.z < 0.0 && n.z.abs() > 0.999 * n.length() && (a.z - zmin).abs() < 1.0 {
                triangles.push([a.truncate(), b.truncate(), c.truncate()]);
            }
        }
    }
    Emprise { piece, triangles, zmin, zmax }
}

/// Élément placé : hors d'un groupe non placé.
pub(super) fn place(o: &Obj) -> bool {
    o.get("m_unplacedOwnerId").and_then(Val::element_id).is_none_or(|g| g == -1)
}

pub(super) fn id_of(o: &Obj, name: &str) -> i64 {
    o.get(name).and_then(Val::element_id).unwrap_or(-1)
}

pub(super) fn reel(o: &Obj, name: &str) -> Option<f64> {
    o.get(name).and_then(Val::real)
}

pub(super) fn vec3(v: &Val) -> Option<DVec3> {
    match v.reals().as_deref() {
        Some(&[x, y, z]) => Some(DVec3::new(x, y, z)),
        _ => None,
    }
}

/// Valeurs des jeux de paramètres double, entier, chaîne et ElementId, par id de
/// paramètre.
pub(super) fn params<'r, 's>(r: &'r Record<'s>, o: &Obj<'s>) -> HashMap<i64, &'r Val<'s>> {
    let mut out = HashMap::new();
    for k in ["m_pParamValueSetDouble", "m_pParamValueSetInt", "m_pParamValueSetAString", "m_pParamValueSetElementId"] {
        let Some(ps) = r.field_body(o, k) else { continue };
        for x in ps.get("m_paramSet").map(Val::list).unwrap_or_default() {
            if let Some((id, v)) = x.obj().and_then(|x| Some((x.get("m_paramId").and_then(Val::element_id)?, x.get("m_value")?))) {
                out.insert(id, v);
            }
        }
    }
    out
}

pub(super) fn reel_p(p: &HashMap<i64, &Val>, k: i64) -> f64 {
    p.get(&k).and_then(|v| v.real()).unwrap_or(0.0)
}

pub(super) fn texte_p(p: &HashMap<i64, &Val>, k: i64) -> String {
    p.get(&k).and_then(|v| v.str()).unwrap_or_default().to_string()
}

/// Cellules de données de l'élément (`Element.m_cellList`).
pub(super) fn cellules<'r, 's>(r: &'r Record<'s>, o: &Obj<'s>) -> Vec<&'r Obj<'s>> {
    let Some(cl) = r.field_body(o, "m_cellList") else { return Vec::new() };
    cl.get("m_cells").map(Val::list).unwrap_or_default().iter().filter_map(|c| r.deref(c).map(|b| &b.obj)).collect()
}

pub(super) fn mur(id: i64, r: &Record, altitude: &HashMap<i64, f64>) -> Mur {
    let o = &r.root;
    let p = params(r, o);
    let mut m = Mur {
        id,
        classe: o.class_name().to_string(),
        type_id: id_of(o, "m_WallAttributesId"),
        niveau: id_of(o, "m_assocLevelId"),
        niveau_haut: id_of(o, "m_upToLevelId"),
        hauteur: reel_p(&p, WALL_USER_HEIGHT_PARAM) * FT,
        decalage_bas: reel_p(&p, WALL_BASE_OFFSET) * FT,
        decalage_haut: reel_p(&p, WALL_TOP_OFFSET) * FT,
        trace: r.field_body(o, "m_pCurveDriver").and_then(|d| r.field_body(d, "m_pCrv")).and_then(Trace::lire),
        geometrie: Err(String::new()),
    };
    if let (Some(h), Some(b)) = (altitude.get(&m.niveau_haut), altitude.get(&m.niveau)) {
        m.hauteur = h + m.decalage_haut - b - m.decalage_bas;
    }
    m
}

pub(super) fn niveau(id: i64, r: &Record) -> Option<Niveau> {
    let z = r.field_body(&r.root, "m_pSurface")?.get("m_origin")?.reals()?.get(2).copied()?;
    Some(Niveau { id, nom: r.root.get("m_text")?.str()?.to_string(), altitude: z * FT, type_id: id_of(&r.root, "m_attrId") })
}

pub(super) fn extraire(eid: i64, s: Sorte, r: &Record, altitude: &HashMap<i64, f64>, herite: &dyn Fn(&str, &str) -> bool) -> Extrait {
    let o = &r.root;
    let ou_rien = |x: Option<Extrait>| x.unwrap_or(Extrait::Rien);
    match s {
        Sorte::Niveau => Extrait::Rien,
        Sorte::Type => ou_rien((|| {
            let nom = r.field_body(o, "m_symbolInfo")?.get("m_name")?.str()?.to_string();
            let epaisseur = r.field_body(o, "m_pCompoundStructure").map(|cs| {
                let couches = cs.get("m_layers").map(Val::list).unwrap_or_default();
                couches.iter().filter_map(|c| c.obj()?.get("m_layerWidth")?.real()).sum::<f64>() * FT
            });
            let couches = r.field_body(o, "m_pCompoundStructure").and_then(couches_structure);
            let couches_variables = r.field_body(o, "m_pCompoundStructure").and_then(|cs| cs.get("m_variableLayerIdx")).and_then(Val::int).is_some_and(|i| i >= 0);
            let mut valeurs = HashMap::new();
            if let Some(pp) = r.field_body(o, "m_pParams") {
                for x in pp.get("m_params").map(Val::list).unwrap_or_default() {
                    if let Some((id, v)) = x.obj().and_then(|x| Some((x.get("m_paramId").and_then(Val::element_id)?, x.get("m_value").and_then(Val::real)?))) {
                        valeurs.insert(id, v * FT);
                    }
                }
            }
            let (largeur, hauteur) = (valeurs.get(&FAMILY_WIDTH_PARAM).copied(), valeurs.get(&FAMILY_HEIGHT_PARAM).copied());
            let categorie_texte = o.get("m_lineAndTextAttr").and_then(Val::obj).map(|a| id_of(a, "m_categoryId")).unwrap_or(-1);
            // type de groupe : `m_insertionOrigin` est un repère (`Trf`), son origine `m_or`
            let origine_insertion = r.field_body(o, "m_insertionOrigin").or(o.get("m_insertionOrigin").and_then(Val::obj)).and_then(|t| t.get("m_or")).and_then(vec3).map(|p| p * FT);
            Some(Extrait::Type(eid, TypeElem { nom, classe: o.class_name().to_string(), epaisseur, couches, couches_variables, famille: id_of(o, "m_familyId"), largeur, hauteur, categorie_texte, origine_insertion }))
        })()),
        Sorte::Famille => ou_rien((|| {
            let occurrence = r.field_body(o, "m_familyParams").map(|fp| {
                let liste = fp.get("m_params").map(Val::list).unwrap_or_default();
                liste.iter().filter_map(Val::obj).filter(|x| x.get("m_instance").and_then(Val::int) == Some(1)).filter_map(|x| x.get("m_paramId").and_then(Val::element_id)).collect()
            });
            let formules = r
                .field_body(o, "m_familyParams")
                .map(|fp| {
                    let liste = fp.get("m_params").map(Val::list).unwrap_or_default();
                    liste
                        .iter()
                        .filter_map(Val::obj)
                        .filter_map(|x| {
                            let ex = x.get("m_oExpression")?;
                            Some((x.get("m_paramId").and_then(Val::element_id)?, crate::rvt::formules::lire(r, r.deref(ex).map(|b| &b.obj).or(ex.obj())?)?))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let document = r.field_body(o, "m_oFamDoc").and_then(|d| d.get("m_contentDocGUID")?.obj()?.get("m_guid")).and_then(|g| match g {
                Val::Guid(g) => Some(*g),
                _ => None,
            });
            let f = Famille { nom: o.get("m_name")?.str()?.to_string(), categorie: id_of(o, "m_categoryId"), parent: id_of(o, "m_famId"), occurrence: occurrence.unwrap_or_default(), document, formules };
            Some(Extrait::Famille(eid, f))
        })()),
        Sorte::Materiau => ou_rien((|| {
            let m = r.field_body(o, "m_pMaterial")?;
            // COLORREF : rouge dans l'octet de poids faible
            let c = m.get("m_color").and_then(Val::int).unwrap_or(0x7F7F7F);
            let couleur = [c as u8, (c >> 8) as u8, (c >> 16) as u8];
            let motif = |id: &str, c: &str| {
                let c = m.get(c).and_then(Val::int).unwrap_or(0);
                (id_of(m, id), [c as u8, (c >> 8) as u8, (c >> 16) as u8])
            };
            let motifs = [
                motif("m_cutPatternId", "m_cutPatternColor"),
                motif("m_cutBackgroundPatternId", "m_cutBackgroundPatternColor"),
                motif("m_surfacePatternId", "m_surfacePatternColor"),
                motif("m_surfaceBackgroundPatternId", "m_surfaceBackgroundPatternColor"),
            ];
            Some(Extrait::Materiau(eid, Materiau { nom: m.get("m_name")?.str()?.to_string(), couleur, apparence: id_of(m, "m_appearanceAssetId"), transparence: m.get("m_transparency").and_then(Val::real).unwrap_or(0.0), motifs }))
        })()),
        Sorte::Mur => Extrait::Mur(mur(eid, r, altitude)),
        Sorte::Dalle => {
            let (champ_type, k) = if o.class_name() == "Ceiling" { ("m_ceilingAttributesId", CEILING_HEIGHTABOVELEVEL_PARAM) } else { ("m_floorAttributesId", FLOOR_HEIGHTABOVELEVEL_PARAM) };
            let p = params(r, o);
            Extrait::Dalle(Dalle { id: eid, classe: o.class_name().to_string(), type_id: id_of(o, champ_type), niveau: id_of(o, "m_assocLevelId"), decalage: reel_p(&p, k) * FT, geometrie: Err(String::new()) })
        }
        Sorte::Toposolide => {
            let p = params(r, o);
            Extrait::Toposolide(Dalle { id: eid, classe: o.class_name().to_string(), type_id: id_of(o, "m_TypeId"), niveau: id_of(o, "m_assocLevelId"), decalage: reel_p(&p, TOPOSOLID_HEIGHTABOVELEVEL_PARAM) * FT, geometrie: Err(String::new()) })
        }
        Sorte::SurfaceTopo => {
            // points saisis : sommets fixes du premier état de la triangulation
            // (`m_drivers[0].m_pMementos[0]`) ; l'état suivant d'une surface hôte ajoute
            // les intersections avec le contour de ses sous-régions
            let points = (|| {
                let d = r.deref(o.get("m_drivers")?.list().first()?)?;
                let m = r.deref(d.obj.get("m_pMementos")?.list().first()?)?;
                Some(m.obj.get("m_fixedVerts")?.list().len())
            })()
            .unwrap_or(0);
            Extrait::SurfaceTopo(SurfaceTopo { id: eid, hote: id_of(o, "m_hostId"), points, geometrie: Err(String::new()) })
        }
        Sorte::Toit => {
            let p = params(r, o);
            Extrait::Toit(Toit { id: eid, classe: o.class_name().to_string(), type_id: id_of(o, "m_roofAttributesId"), niveau: id_of(o, "m_assocLevelId"), decalage: reel_p(&p, ROOF_LEVEL_OFFSET_PARAM) * FT, geometrie: Err(String::new()) })
        }
        Sorte::Escalier => Extrait::Escalier(escalier(eid, r, altitude)),
        Sorte::Volee => Extrait::Volee(eid, id_of(o, "m_stairsId")),
        Sorte::GardeCorps => Extrait::GardeCorps(GardeCorps { id: eid, type_id: id_of(o, "m_stairsRailingAttrId"), niveau: id_of(o, "m_assocLevelId"), geometrie: Err(String::new()) }),
        Sorte::Grille => ou_rien(grille(eid, r).map(Extrait::Grille)),
        Sorte::Instance if id_of(o, "m_ownerDBViewId") != -1 => ou_rien((|| {
            let t = r.field_body(o, "m_pInstanceInfo")?.get("m_Trf")?.obj()?;
            let l = t.get("m_3x3")?.list();
            let ligne = |i: usize| l.get(i).and_then(vec3);
            // la colonne j de m_3x3 est l'axe j (geometrie::Trf)
            let axes = DMat3::from_cols(ligne(0)?, ligne(1)?, ligne(2)?).transpose();
            Some(Extrait::InstanceDeVue(InstanceVue { id: eid, vue: id_of(o, "m_ownerDBViewId"), type_id: id_of(o, "m_masterSymbolId"), origine: t.get("m_or").and_then(vec3)? * FT, axes }))
        })()),
        Sorte::Instance => {
            let (i, h) = instance(eid, r);
            Extrait::Instance(i, h)
        }
        Sorte::Piece => Extrait::Piece(piece(eid, r, altitude)),
        Sorte::Forme => ou_rien(forme(eid, r).map(Extrait::Forme)),
        Sorte::Categorie => ou_rien((|| Some(Extrait::Categorie(eid, r.field_body(o, "m_pCategory")?.get("m_name")?.str()?.to_string())))()),
        Sorte::Vue => ou_rien(vues::vue(eid, r).map(Extrait::Vue)),
        Sorte::Viewer => ou_rien(vues::cadrage(r).map(|(v, o, a, b)| Extrait::Cadrage(v, (o, a, b), vues::cadre(r).map(|c| c.1)))),
        // ligne de zone (limite des zones de surface) : droite `m_pCurveDriver.m_pCrv`
        // (`m_origin`, `m_dirVec`, `m_endParams`, pieds)
        Sorte::Annotation if o.class_name() == "CurveElem" && o.get("m_isAreaMeasurementScheme").and_then(Val::int) == Some(1) => ou_rien((|| {
            let c = r.field_body(r.field_body(o, "m_pCurveDriver")?, "m_pCrv")?;
            let (org, dir) = (c.get("m_origin").and_then(vec3)?, c.get("m_dirVec").and_then(vec3)?);
            let t = c.get("m_endParams").and_then(Val::reals)?;
            let pt = |u: f64| ((org + dir * u) * FT).truncate();
            Some(Extrait::LigneZone(id_of(o, "m_areaSchemeId"), id_of(o, "m_assocLevelId"), pt(*t.first()?), pt(*t.get(1)?)))
        })()),
        Sorte::Annotation | Sorte::Region => ou_rien(vues::element(eid, r).map(Extrait::Element2D)),
        Sorte::Motif => ou_rien(vues::motif(r).map(|x| Extrait::Motif(eid, x))),
        Sorte::TypeRegion => ou_rien(vues::type_region(r).map(|x| Extrait::TypeRegion(eid, x))),
        Sorte::Style => ou_rien((|| {
            // style de projection (type 1) : couleur COLORREF, rouge dans l'octet faible
            if o.get("m_gstyleType").and_then(Val::int) != Some(1) {
                return None;
            }
            let c = r.field_body(o, "m_pGStyle")?.get("m_color")?.int()?;
            Some(Extrait::Style(id_of(o, "m_categoryId"), [c as u8, (c >> 8) as u8, (c >> 16) as u8]))
        })()),
        Sorte::Parametre => ou_rien(parametres::definition(r, herite).map(|d| Extrait::Parametre(eid, d))),
        Sorte::Phase if o.class_name() == "AllProjectPhases" => Extrait::OrdrePhases(o.get("m_phaseIds").map(Val::list).unwrap_or_default().iter().filter_map(Val::element_id).collect()),
        Sorte::Phase if o.class_name() == "PhaseFilterElem" => {
            let mut pres = [0u8; 7];
            for (k, v) in o.get("m_phaseStatusPresentation").map(Val::list).unwrap_or_default().iter().take(7).enumerate() {
                pres[k] = v.int().unwrap_or(0) as u8;
            }
            Extrait::FiltrePhase(eid, o.get("m_name").and_then(Val::str).unwrap_or_default().to_string(), pres)
        }
        Sorte::Phase => ou_rien(o.get("m_name").and_then(Val::str).map(|n| Extrait::Phase(eid, n.to_string()))),
        Sorte::Coordonnee => ou_rien(coordonnees::lire(eid, r).map(Extrait::Coordonnee)),
        Sorte::Police => ou_rien((|| {
            let f = r.field_body(o, "m_pFont")?;
            Some(Extrait::Police(id_of(o, "m_ownerId"), Police { nom: f.get("m_name")?.str()?.to_string(), taille: f.get("m_size")?.real()? * FT }))
        })()),
        Sorte::TypeFenetre => ou_rien((|| {
            let nom = r.field_body(o, "m_symbolInfo")?.get("m_name")?.str()?.to_string();
            let oui = |k| o.get(k).and_then(Val::int) == Some(1);
            Some(Extrait::TypeFenetre(eid, TypeFenetre { nom, titre: oui("m_showLabel"), famille_titre: id_of(o, "m_familyTagId"), ligne: oui("m_showExtensionLine") }))
        })()),
        Sorte::Revision => {
            let t = |k| o.get(k).and_then(Val::str).unwrap_or_default().to_string();
            let sequence = o.get("m_sequenceNumber").and_then(Val::int).unwrap_or(0);
            Extrait::Revision(eid, Revision { description: t("m_description"), date: t("m_date"), numero: t("m_numberStr"), sequence, remise_a: t("m_issuedTo"), remise_par: t("m_issuedBy") })
        }
        Sorte::InfoProjet => Extrait::InfoProjet(eid),
        Sorte::TypeCote => ou_rien(vues::type_cote(r).map(|t| Extrait::TypeCote(eid, t))),
        Sorte::Topologie => ou_rien(crate::rvt::zones::Topologie::lire(r).map(Extrait::Topologie)),
        Sorte::PlanReference => ou_rien(crate::rvt::plans::plan_reference(eid, r).map(Extrait::PlanReference)),
        Sorte::PlanConstruction => ou_rien(crate::rvt::plans::plan_construction(eid, r).map(Extrait::PlanConstruction)),
        Sorte::Apparence => ou_rien(crate::rvt::plans::apparence(r).map(|a| Extrait::Apparence(eid, a))),
        Sorte::Media => ou_rien(crate::rvt::medias::lire(eid, r).map(Extrait::Media)),
        Sorte::Annexe => ou_rien(crate::rvt::annexes::lire(eid, r).map(Extrait::Annexe)),
        Sorte::Profile => {
            let info = r.field_body(o, "m_wallSweepInfo").or(o.get("m_wallSweepInfo").and_then(Val::obj));
            Extrait::Forme(Forme {
                id: eid,
                classe: o.class_name().to_string(),
                sous_categorie: -1,
                materiau: info.map(|i| id_of(i, "m_matId")).unwrap_or(-1),
                famille: -1,
                niveau: id_of(o, "m_assocLevelId"),
                membres: Vec::new(),
                geometrie: Err(String::new()),
            })
        }
        Sorte::Unites => Extrait::Unites(vues::format_projet(r), parametres::Unites::lire(r).unwrap_or_default()),
        Sorte::Esquisse => {
            let d = r.field_body(o, "m_elemIdsPairSet").and_then(|p| p.get("m_data")).map(Val::list).unwrap_or_default();
            Extrait::Esquisse(d.iter().filter_map(|x| x.obj().map(|x| id_of(x, "m_elementId"))).collect())
        }
    }
}

/// Forme pleine ou combinaison ; None pour un vide (`GenSweep.m_cutting` = 1).
pub(super) fn forme(id: i64, r: &Record) -> Option<Forme> {
    let o = &r.root;
    if o.get("m_cutting").and_then(Val::int) == Some(1) {
        return None;
    }
    let mut membres = Vec::new();
    for x in o.get("m_svMap").map(Val::list).unwrap_or_default() {
        let Some(x) = x.obj() else { continue };
        membres.extend(x.get("first").and_then(Val::element_id));
        let vides = x.get("second").and_then(Val::obj).and_then(|s| s.get("m_set")).map(Val::list).unwrap_or_default();
        membres.extend(vides.iter().filter_map(Val::element_id));
    }
    Some(Forme {
        id,
        classe: o.class_name().to_string(),
        sous_categorie: id_of(o, "m_categoryId"),
        materiau: id_of(o, "m_materialId"),
        famille: id_of(o, "m_famId"),
        niveau: id_of(o, "m_assocLevelId"),
        membres,
        geometrie: Err(String::new()),
    })
}

/// Volées et paliers listés par l'escalier (`m_runAndLandings` de ses cellules,
/// `m_runsAndLandings`) ; `lire` y ajoute les parties dont `m_stairsId` le désigne.
pub(super) fn escalier(id: i64, r: &Record, altitude: &HashMap<i64, f64>) -> Escalier {
    let o = &r.root;
    let mut parties = BTreeSet::new();
    for c in cellules(r, o).into_iter().chain([o]) {
        for k in ["m_runAndLandings", "m_runsAndLandings"] {
            parties.extend(c.get(k).map(Val::list).unwrap_or_default().iter().filter_map(Val::element_id));
        }
    }
    let (bas, haut) = (id_of(o, "m_baseLevelId"), id_of(o, "m_topLevelId"));
    let hauteur = match (altitude.get(&bas), altitude.get(&haut)) {
        (Some(b), Some(h)) => h + reel(o, "m_topLevelOffset").unwrap_or(0.0) * FT - b - reel(o, "m_baseLevelOffset").unwrap_or(0.0) * FT,
        _ => reel(o, "m_stairsHeight").unwrap_or(0.0) * FT,
    };
    Escalier {
        id,
        type_id: id_of(o, "m_typeId"),
        niveau_bas: bas,
        niveau_haut: haut,
        contremarches: o.get("m_actualNumberOfRisers").and_then(Val::int).unwrap_or(0),
        hauteur_contremarche: reel(o, "m_actualRiserHeight").unwrap_or(0.0) * FT,
        giron: reel(o, "m_actualTreadDepth").unwrap_or(0.0) * FT,
        hauteur,
        parties: parties.into_iter().collect(),
        geometrie: Err(String::new()),
    }
}

pub(super) fn grille(id: i64, r: &Record) -> Option<Grille> {
    let o = &r.root;
    let s = r.field_body(o, "m_pSurface")?;
    let env = s.get("m_Envelope")?.obj()?.get("m_corners")?.list();
    let (u0, u1) = (env.first()?.reals()?.first().copied()?, env.get(1)?.reals()?.first().copied()?);
    let debut = geometrie::point(r, s, u0, 0.0).ok()? * FT;
    let fin = geometrie::point(r, s, u1, 0.0).ok()? * FT;
    let arc = if s.class_name() == "CylSurf" { Some((s.get("m_center").and_then(vec3)? * FT, reel(s, "m_radius")? * FT)) } else { None };
    Some(Grille { id, nom: o.get("m_text")?.str()?.to_string(), debut, fin, arc })
}

/// Instance, son point valant `m_instOrigin` ; pour une instance hébergée, aussi
/// (`m_hostParam`, `m_elevation` en mm), le point étant replacé par `lire` sur la ligne
/// du mur hôte.
pub(super) fn instance(id: i64, r: &Record) -> (Instance, Option<(f64, f64)>) {
    let o = &r.root;
    let p = params(r, o);
    let mut repere = texte_p(&p, ALL_MODEL_MARK);
    if repere.is_empty() {
        // les portes rangent leur repère dans FamInstSpec.m_doorNumber
        repere = r.field_body(o, "m_oFamInstSpec").and_then(|s| s.get("m_doorNumber")?.str()).unwrap_or_default().to_string();
    }
    let i = Instance {
        id,
        type_id: id_of(o, "m_masterSymbolId"),
        hote: id_of(o, "m_hostId"),
        niveau: id_of(o, "m_assocLevelId"),
        point: o.get("m_instOrigin").and_then(vec3).unwrap_or_default() * FT,
        repere,
        axes: (|| {
            let l = r.field_body(o, "m_pInstanceInfo")?.get("m_Trf")?.obj()?.get("m_3x3")?.list();
            let ligne = |i: usize| l.get(i).and_then(vec3);
            Some(DMat3::from_cols(ligne(0)?, ligne(1)?, ligne(2)?).transpose())
        })(),
        pieces_inversees: o.get("m_flippedFromToRoom").and_then(Val::int).unwrap_or(0) != 0,
        main_retournee: o.get("m_flippedX").and_then(Val::int).unwrap_or(0) != 0,
        geometrie: Err(String::new()),
    };
    (i, reel(o, "m_hostParam").map(|h| (h, reel(o, "m_elevation").unwrap_or(0.0) * FT)))
}

pub(super) fn piece(id: i64, r: &Record, altitude: &HashMap<i64, f64>) -> Piece {
    let o = &r.root;
    let p = params(r, o);
    let niveau = id_of(o, "m_assocLevelId");
    // m_point : (x, y) en pieds, à l'altitude du niveau
    let point = match o.get("m_point").and_then(Val::reals).as_deref() {
        Some(&[x, y]) => Some(DVec3::new(x * FT, y * FT, altitude.get(&niveau).copied().unwrap_or(0.0))),
        _ => None,
    };
    let genre = if id_of(o, "m_areaSchemeId") != -1 {
        "surface"
    } else if id_of(o, "m_zoneSchemeId") != -1 {
        "zone"
    } else {
        "piece"
    };
    let place = o.get("m_bIsLocationless").and_then(Val::int) != Some(1);
    Piece {
        id,
        genre,
        nom: texte_p(&p, ROOM_NAME),
        numero: texte_p(&p, ROOM_NUMBER),
        service: texte_p(&p, ROOM_DEPARTMENT),
        phase: id_of(o, "m_phaseId"),
        schema: id_of(o, "m_areaSchemeId"),
        circuit: o.get("m_cachedCircuitId").and_then(Val::obj).and_then(|c| c.get("m_id")).and_then(Val::int).unwrap_or(-1),
        niveau,
        point: point.filter(|_| place),
        geometrie: Err(String::new()),
    }
}
