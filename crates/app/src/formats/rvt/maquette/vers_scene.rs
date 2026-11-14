//! Conversion de la maquette en scène affichable : produits, régions, navigateur.
use super::*;

impl Builder {
    pub(super) fn add(&mut self, ty: &str, args: Vec<Value>) -> Id {
        let id = self.entities.len() as Id + 1;
        self.entities.insert(id, Entity { ty: Arc::from(ty), args });
        id
    }

    pub(super) fn pset(&mut self, prefix: &str, owner: i64, name: &str, props: Vec<(&str, Value)>) -> Id {
        let refs = props.into_iter().map(|(n, v)| Value::Ref(self.add("IFCPROPERTYSINGLEVALUE", vec![Value::Str(n.into()), Value::Null, v]))).collect();
        self.add("IFCPROPERTYSET", vec![gid(prefix, owner), Value::Null, Value::Str(name.into()), Value::Null, Value::List(refs)])
    }

    /// Un jeu de propriétés par groupe de paramètres de Revit (« Contraintes », « Cotes »…).
    pub(super) fn psets_parametres(&mut self, m: &Maquette, element: i64) -> Vec<Id> {
        let mut out = Vec::new();
        let aff = m.parametres_affiches(element);
        let mut k = 0;
        for groupe in aff.chunk_by(|a, b| a.groupe == b.groupe) {
            let props: Vec<(&str, Value)> = groupe.iter().map(|a| (a.nom.as_str(), Value::Str(a.valeur.clone()))).collect();
            out.push(self.pset(&format!("RVTG{k:02}"), element, groupe[0].groupe, props));
            k += 1;
        }
        out
    }

    /// Rattache des jeux de propriétés à une entité.
    pub(super) fn definir(&mut self, prefix: &str, element: i64, entite: Id, psets: Vec<Id>) {
        for (k, p) in psets.into_iter().enumerate() {
            self.add("IFCRELDEFINESBYPROPERTIES", vec![gid(&format!("{prefix}{k:02}"), element), Value::Null, Value::Null, Value::Null, Value::List(vec![Value::Ref(entite)]), Value::Ref(p)]);
        }
    }
}

/// GlobalId synthétique de 22 caractères, pour les entités qui ne sont pas des éléments
/// Revit (jeux de propriétés, relations…) ou un élément sans IfcGUID connu.
pub(super) fn gid(prefix: &str, id: i64) -> Value {
    Value::Str(format!("{prefix}{:0>width$}", id, width = 22 - prefix.len()))
}

pub(super) fn mm(v: f64) -> Value {
    Value::Str(format!("{} mm", format_real((v * 10.0).round() / 10.0)))
}

pub(super) fn point_mm(p: DVec3) -> Value {
    Value::Str(format!("{} ; {} ; {} mm", format_real((p.x * 10.0).round() / 10.0), format_real((p.y * 10.0).round() / 10.0), format_real((p.z * 10.0).round() / 10.0)))
}

/// Couleur d'un matériau dans la scène (transparence plafonnée pour que les faces
/// « vides » restent visibles).
pub(super) fn couleur(m: &Materiau) -> Rgba {
    let [r, g, b] = m.couleur.map(|c| c as f32 / 255.0);
    [r, g, b, (1.0 - m.transparence as f32).clamp(0.1, 1.0)]
}

/// Tracé d'une région remplie : arrière-plan uni s'il y en a un, puis premier plan uni
/// (face colorée) ou hachures (traits de 0,13 mm sur le papier, découpés sur la face) ;
/// région sans type connu : face grise translucide.
pub(super) fn region(m: &Maquette, type_id: i64, g: &Maillage, v: &Vue, element: Option<Id>) -> Vec<Primitive> {
    let face: Vec<[DVec3; 3]> = g.faces.iter().flat_map(|f| f.triangles.iter().map(|t| t.map(|i| g.sommets[i]))).collect();
    let en_m = |tris: &[[DVec3; 3]]| tris.iter().map(|t| t.map(|p| p / 1000.0)).collect::<Vec<_>>();
    let plein = |couleur: [u8; 4]| Primitive { forme: trace2d::Forme::Triangles(en_m(&face)), style: Style { couleur, epaisseur: 0.0 }, role: Role::Annotation, element };
    let Some(t) = m.types_regions.get(&type_id) else { return vec![plein([115, 115, 115, 153])] };
    let avant = m.couleurs_categories.get(&t.categorie).copied().unwrap_or([0, 0, 0]);
    let avant = [avant[0], avant[1], avant[2], 255];
    let normale = v.droite.cross(v.haut).normalize_or_zero();
    let mut prims = Vec::new();
    match m.motifs.get(&t.arriere) {
        Some(x) if x.grilles.is_empty() => prims.push(plein([t.couleur_arriere[0], t.couleur_arriere[1], t.couleur_arriere[2], 255])),
        _ => prims.push(plein([217, 217, 217, 64])),
    }
    match m.motifs.get(&t.avant) {
        Some(x) if x.grilles.is_empty() => prims.push(plein(avant)),
        Some(x) => {
            // traits légèrement décollés de la face, vers l'observateur
            let ecart = normale * (0.05 * v.echelle);
            let s: Vec<[DVec3; 2]> = vues::hachures(&face, x, v.echelle, v.droite, v.haut, 20_000).into_iter().map(|s| s.map(|p| (p + ecart) / 1000.0)).collect();
            if !s.is_empty() {
                prims.push(Primitive { forme: trace2d::Forme::Segments(s), style: Style { couleur: avant, epaisseur: 0.13 }, role: Role::Hachure, element });
            }
        }
        None => {}
    }
    prims
}

/// Couleur d'une partie (0..1) en octets.
pub(super) fn octets(c: Rgba) -> [u8; 4] {
    c.map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Primitives déjà triangulées d'un élément (glyphes, traits des cotes et des cartouches).
pub(super) fn primitives_parts(parts: Vec<Part>, element: Option<Id>) -> Vec<Primitive> {
    parts.into_iter().map(|p| Primitive { forme: trace2d::Forme::Triangles(p.tris), style: Style { couleur: octets(p.color), epaisseur: 0.0 }, role: Role::Annotation, element }).collect()
}

/// Parties à afficher (triangles par couleur) des primitives d'une vue de normale `normale`
/// à l'échelle `echelle`.
pub(super) fn parts_primitives(prims: &[Primitive], normale: DVec3, echelle: f64) -> Vec<Part> {
    let mut parts: Vec<Part> = Vec::new();
    for p in prims {
        let m = p.mailler(normale, echelle);
        // traits fins : rubans de 0,1 mm (le tampon des traits d'un pixel n'est pas
        // celui des objets 2D)
        let mut tris = m.triangles;
        if !m.traits.is_empty() {
            let fin = Primitive { forme: trace2d::Forme::Segments(m.traits), style: Style { couleur: p.style.couleur, epaisseur: 0.1 }, ..p.clone() };
            tris.extend(fin.mailler(normale, echelle).triangles);
        }
        if tris.is_empty() {
            continue;
        }
        let color = p.style.couleur.map(|c| c as f32 / 255.0);
        match parts.last_mut() {
            Some(q) if q.color == color => q.tris.extend(tris),
            _ => parts.push(Part { color, tris }),
        }
    }
    parts
}

/// Propriétés « Coordonnées » du projet, comme la boîte « Emplacement » et les points de
/// base de Revit : emplacement actif, position partagée du point de base du projet et
/// angle par rapport au nord géographique, point topographique, lieu, autres
/// emplacements.
pub(super) fn parametres_du_projet(m: &Maquette) -> (Vec<(String, String)>, Vec<(String, String)>) {
    let mut globaux: Vec<(String, String)> = m
        .definitions
        .iter()
        .filter(|(_, d)| d.genre == "global")
        .map(|(&id, d)| {
            let mut v = d.valeur_globale.as_ref().and_then(|b| parametres::texte(b, id, d.stockage, &d.spec, "", &m.unites, &|e| m.nom_element(id, e))).unwrap_or_default();
            if d.formule.is_some() {
                v += " (formule)";
            }
            (d.nom.clone(), v)
        })
        .collect();
    globaux.sort();
    let mut liaisons: Vec<(String, String)> = m
        .liaisons
        .iter()
        .filter_map(|(id, (cats, occ))| {
            let d = m.definitions.get(id)?;
            let mut noms: Vec<String> = cats.iter().map(|c| m.nom_categorie(*c)).collect();
            noms.sort();
            let genre = if d.guid.is_some() { "partagé" } else { "projet" };
            Some((d.nom.clone(), format!("{genre}, {} ; {} catégories : {}", if *occ { "occurrence" } else { "type" }, cats.len(), noms.join(", "))))
        })
        .collect();
    liaisons.sort();
    (globaux, liaisons)
}

pub(super) fn coordonnees_affichees(m: &Maquette) -> Vec<(String, String)> {
    let c = &m.coordonnees;
    let Some(e) = c.actif() else { return Vec::new() };
    let metres = |v: f64| format!("{} m", format_real(v.round() / 1000.0));
    let degres = |a: f64| format!("{}°", format_real((a.to_degrees() * 100.0).round() / 100.0));
    let mut out = vec![("Emplacement actif".to_string(), e.nom.clone())];
    let base = c.point_base.unwrap_or_default();
    let s = e.vers_partage(base);
    out.push(("Point de base du projet : E/O".into(), metres(s.x)));
    out.push(("Point de base du projet : N/S".into(), metres(s.y)));
    out.push(("Point de base du projet : Élév.".into(), metres(s.z)));
    // paramètre « Angle par rapport au nord géographique » du point de base
    out.push(("Angle par rapport au nord géographique".into(), degres((-e.angle()).rem_euclid(std::f64::consts::TAU))));
    if base.length() > 1e-6 {
        out.push(("Point de base du projet (repère interne)".into(), format!("{} ; {} ; {} mm", format_real(base.x.round()), format_real(base.y.round()), format_real(base.z.round()))));
    }
    if let Some(t) = c.point_topo {
        out.push(("Point topographique (repère interne)".into(), format!("{} ; {} ; {} mm", format_real(t.x.round()), format_real(t.y.round()), format_real(t.z.round()))));
    }
    if let Some(l) = c.lieux.get(&e.lieu) {
        if !l.nom.is_empty() {
            out.push(("Lieu".into(), l.nom.clone()));
        }
        out.push(("Latitude".into(), format!("{:.6}°", l.latitude)));
        out.push(("Longitude".into(), format!("{:.6}°", l.longitude)));
        if !l.systeme.is_empty() {
            let code = l.epsg.map(|c| format!(" (EPSG {c})")).unwrap_or_default();
            out.push(("Système de coordonnées".into(), format!("{}{code}", l.systeme)));
        }
        out.push(("Fuseau horaire".into(), format!("UTC{}{}", if l.fuseau < 0.0 { "−" } else { "+" }, format_real(l.fuseau.abs()))));
        if !l.station.is_empty() {
            out.push(("Station météo".into(), l.station.clone()));
        }
    }
    for o in c.nommes().filter(|o| o.id != e.id) {
        let p = o.vers_partage(DVec3::ZERO);
        out.push((format!("Emplacement « {} »", o.nom), format!("E/O {}, N/S {}, Élév. {}, {}", metres(p.x), metres(p.y), metres(p.z), degres(o.angle()))));
    }
    out
}

/// Lit le fichier et construit la scène du viewer. Erreur si aucun élément n'a pu être
/// maillé.
pub fn load_scene(path: &Path, titre: &str, format: &str, progress: &dyn Fn(String)) -> Result<Scene, String> {
    let t0 = std::time::Instant::now();
    let m = lire(path, progress)?;
    let ko = |v: &mut dyn Iterator<Item = &Geometrie>| v.filter(|g| g.is_err()).count();
    let murs_ko = ko(&mut m.murs.iter().map(|x| &x.geometrie));
    let mailles = m.murs.len() - murs_ko
        + m.dalles.iter().filter(|x| x.geometrie.is_ok()).count()
        + m.toposolides.iter().filter(|x| x.geometrie.is_ok()).count()
        + m.surfaces_topo.iter().filter(|x| x.geometrie.is_ok()).count()
        + m.toits.iter().filter(|x| x.geometrie.is_ok()).count()
        + m.escaliers.iter().filter(|x| x.geometrie.is_ok()).count()
        + m.instances.iter().filter(|x| x.geometrie.is_ok()).count()
        + m.formes.iter().filter(|x| x.geometrie.is_ok()).count()
        + m.elements_2d.iter().filter(|x| !matches!(x.contenu, Contenu::FenetreDeVue { .. })).count();
    if mailles == 0 {
        return Err(match m.murs.len() {
            0 => "aucun mur, sol, toit, escalier, famille, forme ni élément 2D dans le fichier".into(),
            n => format!("aucun des {n} murs n'a pu être maillé"),
        });
    }
    progress("Assemblage des tampons…".into());
    let file_name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut s = scene(&m, titre, file_name, format);
    let pieces: Vec<&Piece> = m.pieces.iter().filter(|p| p.genre == "piece" && p.point.is_some()).collect();
    let toutes = m.murs.iter().map(|x| &x.geometrie).chain(m.dalles.iter().map(|x| &x.geometrie)).chain(m.toits.iter().map(|x| &x.geometrie));
    let toutes = toutes.chain(m.toposolides.iter().map(|x| &x.geometrie)).chain(m.surfaces_topo.iter().map(|x| &x.geometrie));
    let toutes = toutes.chain(m.escaliers.iter().map(|x| &x.geometrie)).chain(m.instances.iter().map(|x| &x.geometrie)).chain(m.garde_corps.iter().map(|x| &x.geometrie));
    let toutes = toutes.chain(m.formes.iter().map(|x| &x.geometrie));
    let solides_ko: usize = toutes.filter_map(|g| g.as_ref().ok()).map(|g| g.echecs.len()).sum();
    for (nom, n) in [
        ("Murs Revit non maillés", murs_ko),
        ("Sols et plafonds Revit non maillés", ko(&mut m.dalles.iter().map(|x| &x.geometrie))),
        ("Toposolides Revit non maillés", ko(&mut m.toposolides.iter().map(|x| &x.geometrie))),
        ("Surfaces topographiques Revit non maillées", ko(&mut m.surfaces_topo.iter().map(|x| &x.geometrie))),
        ("Toits Revit non maillés", ko(&mut m.toits.iter().map(|x| &x.geometrie))),
        ("Escaliers Revit non maillés", ko(&mut m.escaliers.iter().map(|x| &x.geometrie))),
        ("Garde-corps Revit non maillés", ko(&mut m.garde_corps.iter().map(|x| &x.geometrie))),
        ("Pièces Revit sans volume", ko(&mut pieces.iter().map(|x| &x.geometrie))),
        ("Familles Revit sans géométrie 3D", ko(&mut m.instances.iter().map(|x| &x.geometrie))),
        ("Formes de famille sans solide (entièrement évidées)", ko(&mut m.formes.iter().map(|x| &x.geometrie))),
        ("Solides Revit non décodés dans des éléments affichés", solides_ko),
        ("Objets Revit illisibles", m.illisibles.len()),
    ] {
        if n > 0 {
            s.unsupported.push((nom.into(), n));
        }
    }
    s.load_ms = t0.elapsed().as_millis();
    Ok(s)
}

/// Produits de la scène : murs, sols, plafonds, toits, escaliers, garde-corps, instances
/// de familles, formes d'un document de famille, pièces placées et grilles.
pub(super) fn produits(m: &Maquette) -> Vec<Produit<'_>> {
    let nom_niveau: HashMap<i64, &str> = m.niveaux.iter().map(|l| (l.id, l.nom.as_str())).collect();
    let niveau = |l: i64| nom_niveau.get(&l).map(|s| s.to_string()).unwrap_or_else(|| l.to_string());
    let nom_type = |t: i64| m.types.get(&t).map(|t| t.nom.clone()).unwrap_or_else(|| format!("Type {t}"));
    let mut out = Vec::new();
    for x in &m.murs {
        let nt = nom_type(x.type_id);
        out.push(Produit {
            id: x.id,
            classe: "IFCWALL",
            nom: format!("{nt} [{}]", x.id),
            type_id: x.type_id,
            nom_type: nt.clone(),
            niveau: x.niveau,
            props: vec![
                ("Classe Revit", Value::Str(x.classe.clone())),
                ("Type", Value::Str(format!("{nt} [{}]", x.type_id))),
                ("Contrainte inférieure", Value::Str(niveau(x.niveau))),
                ("Contrainte supérieure", Value::Str(if x.niveau_haut == -1 { "Non contraint".into() } else { niveau(x.niveau_haut) })),
                ("Hauteur", mm(x.hauteur)),
                ("Décalage inférieur", mm(x.decalage_bas)),
                ("Décalage supérieur", mm(x.decalage_haut)),
            ],
            geometrie: &x.geometrie,
        });
    }
    for x in &m.toposolides {
        let nt = nom_type(x.type_id);
        let mut props = vec![("Classe Revit", Value::Str(x.classe.clone())), ("Type", Value::Str(format!("{nt} [{}]", x.type_id)))];
        if x.niveau != -1 {
            props.push(("Niveau", Value::Str(niveau(x.niveau))));
            props.push(("Hauteur par rapport au niveau", mm(x.decalage)));
        }
        out.push(Produit { id: x.id, classe: "IFCGEOGRAPHICELEMENT", nom: format!("{nt} [{}]", x.id), type_id: x.type_id, nom_type: nt.clone(), niveau: x.niveau, props, geometrie: &x.geometrie });
    }
    for x in &m.surfaces_topo {
        let (nom, mut props) = if x.hote == -1 { ("Surface topographique", vec![]) } else { ("Sous-région", vec![("Surface hôte", Value::Str(x.hote.to_string()))]) };
        props.insert(0, ("Classe Revit", Value::Str("SiteSurface".into())));
        props.push(("Points", Value::Str(x.points.to_string())));
        out.push(Produit { id: x.id, classe: "IFCGEOGRAPHICELEMENT", nom: format!("{nom} [{}]", x.id), type_id: -1, nom_type: nom.into(), niveau: -1, props, geometrie: &x.geometrie });
    }
    for x in &m.dalles {
        let nt = nom_type(x.type_id);
        out.push(Produit {
            id: x.id,
            classe: if x.classe == "Ceiling" { "IFCCOVERING" } else { "IFCSLAB" },
            nom: format!("{nt} [{}]", x.id),
            type_id: x.type_id,
            nom_type: nt.clone(),
            niveau: x.niveau,
            props: vec![
                ("Classe Revit", Value::Str(x.classe.clone())),
                ("Type", Value::Str(format!("{nt} [{}]", x.type_id))),
                ("Niveau", Value::Str(niveau(x.niveau))),
                ("Hauteur par rapport au niveau", mm(x.decalage)),
            ],
            geometrie: &x.geometrie,
        });
    }
    for x in &m.toits {
        let nt = nom_type(x.type_id);
        out.push(Produit {
            id: x.id,
            classe: "IFCROOF",
            nom: format!("{nt} [{}]", x.id),
            type_id: x.type_id,
            nom_type: nt.clone(),
            niveau: x.niveau,
            props: vec![
                ("Classe Revit", Value::Str(x.classe.clone())),
                ("Type", Value::Str(format!("{nt} [{}]", x.type_id))),
                ("Niveau de base", Value::Str(niveau(x.niveau))),
                ("Décalage par rapport au niveau", mm(x.decalage)),
            ],
            geometrie: &x.geometrie,
        });
    }
    for x in &m.escaliers {
        let nt = nom_type(x.type_id);
        out.push(Produit {
            id: x.id,
            classe: "IFCSTAIR",
            nom: format!("{nt} [{}]", x.id),
            type_id: x.type_id,
            nom_type: nt.clone(),
            niveau: x.niveau_bas,
            props: vec![
                ("Type", Value::Str(format!("{nt} [{}]", x.type_id))),
                ("Niveau de base", Value::Str(niveau(x.niveau_bas))),
                ("Niveau supérieur", Value::Str(niveau(x.niveau_haut))),
                ("Hauteur", mm(x.hauteur)),
                ("Nombre de contremarches", Value::Str(x.contremarches.to_string())),
                ("Hauteur de contremarche", mm(x.hauteur_contremarche)),
                ("Giron", mm(x.giron)),
                ("Volées et paliers (ElementId)", Value::Str(x.parties.iter().map(i64::to_string).collect::<Vec<_>>().join(", "))),
            ],
            geometrie: &x.geometrie,
        });
    }
    for x in &m.garde_corps {
        let nt = nom_type(x.type_id);
        out.push(Produit {
            id: x.id,
            classe: "IFCRAILING",
            nom: format!("{nt} [{}]", x.id),
            type_id: x.type_id,
            nom_type: nt.clone(),
            niveau: x.niveau,
            props: vec![("Type", Value::Str(format!("{nt} [{}]", x.type_id))), ("Niveau", Value::Str(niveau(x.niveau)))],
            geometrie: &x.geometrie,
        });
    }
    for x in &m.instances {
        let (cat, classe) = m.categorie(x);
        let nt = m.nom_type_instance(x);
        let mut props = vec![
            ("Catégorie", Value::Str(cat)),
            ("Famille et type", Value::Str(format!("{nt} [{}]", x.type_id))),
            ("Niveau", Value::Str(niveau(x.niveau))),
            ("Point d'insertion", point_mm(x.point)),
        ];
        if let Some(t) = m.types.get(&x.type_id) {
            // cotes du panneau (paramètres de type, ou d'occurrence si la famille les
            // déclare ainsi), pas l'encombrement de la famille
            let occ = m.familles.get(&t.famille).map(|f| &f.occurrence);
            let cote = |p: i64, du_type: Option<f64>| {
                if !occ.is_some_and(|o| o.contains(&p)) {
                    return du_type;
                }
                let b = m.parametres.get(&x.id)?.iter().find(|(q, _)| *q == p)?.1.selon(parametres::Stockage::Reel);
                if let Brut::Reel(v) = b { Some(v * FT) } else { None }
            };
            if let Some(l) = cote(FAMILY_WIDTH_PARAM, t.largeur).filter(|&l| l > 0.0) {
                props.push(("Largeur (panneau)", mm(l)));
            }
            if let Some(h) = cote(FAMILY_HEIGHT_PARAM, t.hauteur).filter(|&h| h > 0.0) {
                props.push(("Hauteur (panneau)", mm(h)));
            }
        }
        if x.hote != -1 {
            props.push(("Hôte (ElementId)", Value::Str(x.hote.to_string())));
        }
        if !x.repere.is_empty() {
            props.push(("Repère", Value::Str(x.repere.clone())));
        }
        out.push(Produit { id: x.id, classe, nom: format!("{nt} [{}]", x.id), type_id: x.type_id, nom_type: nt, niveau: x.niveau, props, geometrie: &x.geometrie });
    }
    for x in &m.formes {
        let (cat, classe) = m.categorie_famille(x.famille);
        let mut props = vec![("Classe Revit", Value::Str(x.classe.clone())), ("Catégorie", Value::Str(cat))];
        if x.sous_categorie != -1 {
            props.push(("Sous-catégorie", Value::Str(m.nom_categorie(x.sous_categorie))));
        }
        if x.materiau != -1 {
            let nom = m.materiaux.get(&x.materiau).map(|x| x.nom.clone()).unwrap_or_else(|| x.materiau.to_string());
            props.push(("Matériau", Value::Str(nom)));
        }
        if !x.membres.is_empty() {
            props.push(("Formes combinées (ElementId)", Value::Str(x.membres.iter().map(i64::to_string).collect::<Vec<_>>().join(", "))));
        }
        out.push(Produit { id: x.id, classe, nom: format!("{} [{}]", nom_forme(&x.classe), x.id), type_id: -1, nom_type: String::new(), niveau: x.niveau, props, geometrie: &x.geometrie });
    }
    for x in m.pieces.iter().filter(|p| p.genre == "piece" && p.point.is_some()) {
        let mut props = vec![("Nom", Value::Str(x.nom.clone())), ("Numéro", Value::Str(x.numero.clone())), ("Niveau", Value::Str(niveau(x.niveau)))];
        if !x.service.is_empty() {
            props.push(("Service", Value::Str(x.service.clone())));
        }
        out.push(Produit { id: x.id, classe: "IFCSPACE", nom: format!("{} {}", x.numero, x.nom), type_id: -1, nom_type: String::new(), niveau: x.niveau, props, geometrie: &x.geometrie });
    }
    out
}

/// Scène du viewer : un modèle IFC synthétique (projet, niveaux, éléments, types,
/// propriétés) pour l'arborescence et les propriétés, et les maillages colorés par
/// matériau.
/// Côté des couches d'un mur droit à plusieurs couches, vérifié par les matériaux
/// réellement présents sur ses deux grandes faces : normale gauche du tracé, signe
/// (+1 : couche extérieure du côté du minimum le long de la normale), étendue (mm) le
/// long de la normale, couches.
pub(super) fn cote_couches<'a>(m: &'a Maquette, mur: &Mur) -> Option<(DVec3, f64, f64, f64, &'a [CoucheType])> {
    let t = m.types.get(&mur.type_id)?;
    if t.couches_variables { return None; }
    let couches = t.couches.as_ref().filter(|c| c.len() > 1)?;
    let Trace::Ligne { direction, .. } = mur.trace? else { return None };
    let normale = DVec3::new(-direction.y, direction.x, 0.0).normalize_or_zero();
    if normale == DVec3::ZERO { return None; }
    let g = mur.geometrie.as_ref().ok()?;
    let (min, max) = g.sommets.iter().map(|p| p.dot(normale)).fold((f64::INFINITY, f64::NEG_INFINITY), |(a,b), x| (a.min(x),b.max(x)));
    let total: f64 = couches.iter().map(|c| c.epaisseur).sum();
    if total <= 0.0 || (max-min-total).abs() > 0.5 { return None; }
    // La courbe porteuse seule ne dit pas si le mur est retourné. Valider le côté
    // extérieur avec les matériaux réellement présents sur ses deux grandes faces.
    let cote = |materiau: i64| -> Option<f64> {
        if materiau == -1 { return None; }
        let mut cotes = [false; 2];
        for f in g.faces.iter().filter(|f| f.materiau == materiau) {
            for tri in &f.triangles {
                let p = tri.map(|i| g.sommets[i]);
                let n = (p[1]-p[0]).cross(p[2]-p[0]).normalize_or_zero();
                if n.dot(normale).abs() < 0.95 { continue; }
                for (k, z) in [min,max].iter().enumerate() {
                    if p.iter().all(|p| (p.dot(normale)-z).abs() < 0.1) { cotes[k] = true; }
                }
            }
        }
        match cotes { [true,false] => Some(1.0), [false,true] => Some(-1.0), _ => None }
    };
    let exterieur = cote(couches.first()?.materiau);
    let interieur = cote(couches.last()?.materiau).map(|s| -s);
    if exterieur.zip(interieur).is_some_and(|(a,b)| a != b) { return None; }
    let signe = exterieur.or(interieur)?;
    Some((normale, signe, min, max, couches))
}

/// Direction (plan, unitaire) vers l'extérieur d'un mur droit à plusieurs couches,
/// si les matériaux de ses faces permettent de la vérifier.
pub fn direction_exterieure(m: &Maquette, mur: &Mur) -> Option<glam::DVec2> {
    let (normale, signe, ..) = cote_couches(m, mur)?;
    // signe +1 : extérieur du côté du minimum le long de la normale
    Some((-normale * signe).truncate())
}

pub(super) fn bandes_couches_mur(m: &Maquette, mur: &Mur, origine: DVec3) -> Option<Vec<scene::CoucheCoupe>> {
    let (normale, signe, min, max, couches) = cote_couches(m, mur)?;
    let normale = normale * signe;
    let mut position = (if signe > 0.0 { min } else { -max }) / 1000.0 - normale.dot(origine);
    let mut bandes = Vec::new();
    for c in couches {
        let fin = position + c.epaisseur / 1000.0;
        if c.epaisseur > 0.0 {
            let mat = m.materiaux.get(&c.materiau);
            let couleur = mat.map_or([195,195,190], |m| m.couleur);
            let motif = mat.and_then(|m| m.motifs.first()).and_then(|(id,_)| m.motifs.get(id)).cloned().map(Arc::new);
            let encre = mat.map_or([70,70,70], |m| m.motifs[0].1);
            bandes.push(scene::CoucheCoupe { plans: vec![normale.as_vec3().extend(-position as f32), (-normale).as_vec3().extend(fin as f32)], couleur: [couleur[0],couleur[1],couleur[2],255], fond: [255,255,255,255], motif_fond: None, motif, couleur_motif: [encre[0],encre[1],encre[2],255], hachure_defaut: mat.is_none(), isolation: false, centre_isolation: None, trait_mm: noyau::aretes::TRAIT_FORT_MM });
        }
        position = fin;
    }
    Some(bandes)
}

pub fn scene(m: &Maquette, titre: &str, file_name: String, format: &str) -> Scene {
    let mut b = Builder { entities: IdMap::default() };
    let mut element_de: HashMap<Id, i64> = HashMap::new();
    let n = Value::Null;
    // GlobalId d'un élément Revit : son IfcGUID, comme à l'export IFC de Revit
    let guid = |prefix: &str, id: i64| m.guid_ifc(id).map(Value::Str).unwrap_or_else(|| gid(prefix, id));
    let projet = b.add("IFCPROJECT", vec![gid("RVTPROJET", 0), n.clone(), Value::Str(titre.into())]);
    let coord = coordonnees_affichees(m);
    if !coord.is_empty() {
        let ps = b.pset("RVTCOORD", 0, "Coordonnées", coord.iter().map(|(k, v)| (k.as_str(), Value::Str(v.clone()))).collect());
        b.definir("RVTDEFC", 0, projet, vec![ps]);
    }
    // paramètres globaux (valeur, formule) et paramètres de projet et partagés (liaisons)
    let (globaux, liaisons) = parametres_du_projet(m);
    if !globaux.is_empty() {
        let ps = b.pset("RVTGLOB", 0, "Paramètres globaux", globaux.iter().map(|(k, v)| (k.as_str(), Value::Str(v.clone()))).collect());
        b.definir("RVTDEFG", 0, projet, vec![ps]);
    }
    if !liaisons.is_empty() {
        let ps = b.pset("RVTLIAI", 0, "Paramètres de projet et partagés", liaisons.iter().map(|(k, v)| (k.as_str(), Value::Str(v.clone()))).collect());
        b.definir("RVTDEFL", 0, projet, vec![ps]);
    }
    // document de famille : paramètres de la famille (valeurs du type courant)
    if let Some(f) = m.famille_document {
        let ps = b.psets_parametres(m, f);
        b.definir("RVTDEFF", f, projet, ps);
    }
    let mut etage: HashMap<i64, Id> = HashMap::new();
    for l in &m.niveaux {
        let args = vec![guid("RVTNIV", l.id), n.clone(), Value::Str(l.nom.clone()), n.clone(), n.clone(), n.clone(), n.clone(), n.clone(), Value::Enum("ELEMENT".into()), Value::Real(l.altitude / 1000.0)];
        let e = b.add("IFCBUILDINGSTOREY", args);
        let ps = b.psets_parametres(m, l.id);
        b.definir("RVTDEFN", l.id, e, ps);
        etage.insert(l.id, e);
    }
    let storeys: Vec<Value> = m.niveaux.iter().map(|l| Value::Ref(etage[&l.id])).collect();
    b.add("IFCRELAGGREGATES", vec![gid("RVTAGG", 0), n.clone(), n.clone(), n.clone(), Value::Ref(projet), Value::List(storeys)]);
    let shape = b.add("IFCPRODUCTDEFINITIONSHAPE", vec![n.clone(), n.clone(), Value::List(vec![])]);

    let mut types: HashMap<(i64, &str), Id> = HashMap::new();
    let mut compositions: HashMap<Id, Id> = HashMap::new();
    let mut materiaux_composition: HashMap<i64, Id> = HashMap::new();
    let mut contenus: BTreeMap<Id, Vec<Value>> = BTreeMap::new();
    let mut geoms = Vec::new();
    // éléments qui portent chaque matériau (navigateur : onglet Matériaux)
    let mut usages: HashMap<i64, Vec<Id>> = HashMap::new();
    // traits des motifs de surface : plafond (2 sommets de 24 octets par trait)
    let (mut traits_motifs, mut motifs_omis, mut motifs_dessin) = (0usize, 0usize, 0usize);
    for p in produits(m) {
        let id = b.add(p.classe, vec![guid("RVT", p.id), n.clone(), Value::Str(p.nom.clone()), n.clone(), Value::Str(p.nom_type.clone()), n.clone(), Value::Ref(shape), Value::Str(p.id.to_string())]);
        element_de.insert(id, p.id);
        let mut props = vec![("ElementId", Value::Str(p.id.to_string()))];
        if let Some(u) = m.unique_id(p.id) {
            props.push(("UniqueId", Value::Str(u)));
        }
        // variante : jeu, option, principale ou secondaire
        if let Some((jeu, option, principale)) = m.variante(p.id) {
            props.push(("Variante", Value::Str(format!("{jeu} : {option} ({})", if principale { "principale" } else { "secondaire" }))));
        }
        // membre d'un groupe : nom du type de groupe et ElementId du groupe
        if let Some(g) = m.groupe_de.get(&p.id) {
            let nom = m.groupes.iter().find(|x| x.id == *g).and_then(|x| m.types.get(&x.type_id)).map(|t| t.nom.clone()).unwrap_or_default();
            props.push(("Groupe", Value::Str(format!("{nom} ({g})"))));
        }
        props.extend(p.props);
        match p.geometrie {
            Ok(g) => {
                let mut mats: Vec<&Materiau> = g.faces.iter().filter_map(|f| m.materiaux.get(&f.materiau)).collect();
                mats.sort_unstable_by(|a, b| a.nom.cmp(&b.nom));
                mats.dedup_by(|a, b| a.nom == b.nom);
                props.push(("Matériaux des faces", Value::Str(mats.iter().map(|x| x.nom.as_str()).collect::<Vec<_>>().join(", "))));
                // apparences de rendu des matériaux : nom, couleur diffuse, texture
                let apparences: Vec<String> = mats
                    .iter()
                    .filter_map(|x| {
                        let a = m.apparences.get(&x.apparence)?;
                        let mut t = format!("{} : {}", x.nom, a.nom);
                        if let Some(c) = a.diffuse {
                            t += &format!(" (couleur {} {} {})", (c[0] * 255.0).round(), (c[1] * 255.0).round(), (c[2] * 255.0).round());
                        }
                        if let Some(tx) = &a.texture {
                            t += &format!(" (texture {})", tx.split('|').next().unwrap_or(tx).rsplit(['/', '\\']).next().unwrap_or(tx));
                        }
                        Some(t)
                    })
                    .collect();
                if !apparences.is_empty() {
                    props.push(("Apparences de rendu", Value::Str(apparences.join(" ; "))));
                }
                // motifs de coupe et de surface (premier plan) de chaque matériau
                let nom_motif = |id: i64| m.motifs.get(&id).map(|x| x.nom.as_str()).unwrap_or("aucun");
                let motifs: Vec<String> = mats.iter().filter(|x| x.motifs[0].0 != -1 || x.motifs[2].0 != -1).map(|x| format!("{} : coupe {}, surface {}", x.nom, nom_motif(x.motifs[0].0), nom_motif(x.motifs[2].0))).collect();
                if !motifs.is_empty() {
                    props.push(("Motifs des matériaux", Value::Str(motifs.join(" ; "))));
                }
            }
            Err(e) => props.push(("Géométrie", Value::Str(e.clone()))),
        }
        let pset = b.pset("RVTPSET", p.id, "Revit", props);
        b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEF", p.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(pset)]);
        let ps = b.psets_parametres(m, p.id);
        b.definir("RVTDEFP", p.id, id, ps);
        if let Ok(g) = p.geometrie {
            let c = g.controle();
            let qv = b.add("IFCQUANTITYVOLUME", vec![Value::Str("Volume".into()), n.clone(), n.clone(), Value::Real(c.volume_m3)]);
            let qa = b.add("IFCQUANTITYAREA", vec![Value::Str("Aire des faces".into()), n.clone(), n.clone(), Value::Real(c.aire_m2)]);
            let qto = b.add("IFCELEMENTQUANTITY", vec![gid("RVTQTO", p.id), n.clone(), Value::Str("Géométrie Revit (GElement)".into()), n.clone(), n.clone(), Value::List(vec![Value::Ref(qv), Value::Ref(qa)])]);
            b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEFQ", p.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(qto)]);
            // une pièce garde la couleur des espaces ; les autres prennent celle de leurs matériaux
            let defaut = class_color(p.classe);
            let mut par_mat: BTreeMap<i64, Vec<[DVec3; 3]>> = BTreeMap::new();
            for f in &g.faces {
                par_mat.entry(f.materiau).or_default().extend(f.triangles.iter().map(|t| t.map(|i| g.sommets[i] / 1000.0)));
            }
            for &k in par_mat.keys() {
                usages.entry(k).or_default().push(id);
            }
            // couleur de rendu : diffuse de l'apparence du matériau, sauf si elle est
            // donnée par une texture (sa valeur n'est alors qu'un repli)
            let rendu_mat = |k: i64| -> Option<Rgba> {
                let x = m.materiaux.get(&k)?;
                let a = m.apparences.get(&x.apparence)?;
                let d = a.diffuse.filter(|_| a.texture.is_none())?;
                Some([d[0] as f32, d[1] as f32, d[2] as f32, 1.0 - x.transparence as f32])
            };
            let rendu: Vec<Option<Rgba>> = par_mat.keys().map(|&k| if p.classe == "IFCSPACE" { None } else { rendu_mat(k) }).collect();
            let parts = par_mat
                .into_iter()
                .map(|(k, tris)| Part { color: if p.classe == "IFCSPACE" { defaut } else { m.materiaux.get(&k).map(couleur).unwrap_or(defaut) }, tris })
                .collect();
            geoms.push(ProductGeom { id, parts, lignes: Vec::new(), rendu, motifs: Vec::new() });
            // motifs de surface des matériaux (motifs de modèle), calés face par face
            let mut par_couleur: BTreeMap<[u8; 3], Vec<[DVec3; 2]>> = BTreeMap::new();
            for f in &g.faces {
                let Some(cal) = f.motif else { continue };
                match m.motifs.get(&cal.motif) {
                    Some(x) if x.modele && !x.grilles.is_empty() => {
                        if traits_motifs >= MAX_TRAITS_MOTIFS {
                            motifs_omis += 1;
                            continue;
                        }
                        let tris: Vec<[DVec3; 3]> = f.triangles.iter().map(|t| t.map(|i| g.sommets[i])).collect();
                        let s = vues::motif_sur_face(&tris, &cal, x, 1.0, MAX_TRAITS_MOTIFS - traits_motifs);
                        traits_motifs += s.len();
                        par_couleur.entry(cal.couleur).or_default().extend(s);
                    }
                    Some(x) if !x.modele && !x.grilles.is_empty() => motifs_dessin += 1,
                    _ => {}
                }
            }
            if !par_couleur.is_empty() {
                let f = b.add("IFCSURFACEFEATURE", vec![gid("RVTMOTIF", p.id), n.clone(), Value::Str(format!("Motifs de surface [{}]", p.id)), n.clone(), n.clone(), n.clone(), Value::Ref(shape)]);
                b.add("IFCRELAGGREGATES", vec![gid("RVTAGGM", p.id), n.clone(), n.clone(), n.clone(), Value::Ref(id), Value::List(vec![Value::Ref(f)])]);
                let lignes = par_couleur.into_iter().map(|(c, segs)| Ligne { color: [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0], segs }).collect();
                geoms.push(ProductGeom { id: f, parts: Vec::new(), lignes, rendu: Vec::new(), motifs: Vec::new() });
            }
        }
        if p.type_id != -1 {
            let classe_type: &str = p.classe;
            let t = match types.get(&(p.type_id, classe_type)) {
                Some(&t) => t,
                None => {
                    let mut tp = vec![("ElementId", Value::Str(p.type_id.to_string()))];
                    if let Some(e) = m.types.get(&p.type_id).and_then(|t| t.epaisseur) {
                        tp.push(("Épaisseur", mm(e)));
                    }
                    if let Some(u) = m.unique_id(p.type_id) {
                        tp.push(("UniqueId", Value::Str(u)));
                    }
                    let pset = b.pset("RVTTPSET", p.type_id, "Revit", tp);
                    let mut psets = vec![Value::Ref(pset)];
                    psets.extend(b.psets_parametres(m, p.type_id).into_iter().map(Value::Ref));
                    let couches = m.types.get(&p.type_id).and_then(|t| t.couches.as_ref());
                    if let Some(couches) = couches {
                        let props: Vec<(String, Value)> = couches.iter().enumerate().map(|(k, c)| {
                            let materiau = if c.materiau == -1 { "Par catégorie".into() } else { m.materiaux.get(&c.materiau).map(|m| m.nom.clone()).unwrap_or_else(|| format!("Matériau {}", c.materiau)) };
                            (format!("Couche {}", k+1), Value::Str(format!("{} mm — {materiau}", noyau::nombres::format_real(c.epaisseur))))
                        }).collect();
                        let ps = b.pset("RVTCOUCHES", p.type_id, "Composition", props.iter().map(|(k,v)| (k.as_str(),v.clone())).collect());
                        psets.push(Value::Ref(ps));
                    }
                    let t = b.add(&format!("{}TYPE", p.classe), vec![guid("RVTTYPE", p.type_id), n.clone(), Value::Str(p.nom_type.clone()), n.clone(), n.clone(), Value::List(psets)]);
                    if let Some(couches) = couches {
                        let refs = couches.iter().map(|c| {
                            let mat = if c.materiau == -1 { Value::Null } else {
                                Value::Ref(*materiaux_composition.entry(c.materiau).or_insert_with(|| {
                                    let nom = m.materiaux.get(&c.materiau).map(|m| m.nom.clone()).unwrap_or_else(|| format!("Matériau {}", c.materiau));
                                    b.add("IFCMATERIAL", vec![Value::Str(nom)])
                                }))
                            };
                            Value::Ref(b.add("IFCMATERIALLAYER", vec![mat, Value::Real(c.epaisseur / 1000.0), Value::Null]))
                        }).collect();
                        let jeu = b.add("IFCMATERIALLAYERSET", vec![Value::List(refs), Value::Str(p.nom_type.clone())]);
                        compositions.insert(t, jeu);
                    }
                    types.insert((p.type_id, classe_type), t);
                    t
                }
            };
            b.add("IFCRELDEFINESBYTYPE", vec![gid("RVTRELT", p.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(t)]);
            if let Some(&jeu) = compositions.get(&t) {
                b.add("IFCRELASSOCIATESMATERIAL", vec![gid("RVTCMAT", p.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(jeu)]);
            }
        }
        contenus.entry(etage.get(&p.niveau).copied().unwrap_or(projet)).or_default().push(Value::Ref(id));
    }
    for g in &m.grilles {
        let mut props = vec![("ElementId", Value::Str(g.id.to_string())), ("Début", point_mm(g.debut)), ("Fin", point_mm(g.fin))];
        match g.arc {
            Some((c, r)) => {
                props.push(("Centre", point_mm(c)));
                props.push(("Rayon", mm(r)));
            }
            None => props.push(("Longueur", mm(g.debut.distance(g.fin)))),
        }
        let id = b.add("IFCGRID", vec![guid("RVT", g.id), n.clone(), Value::Str(g.nom.clone()), n.clone(), n.clone(), n.clone(), Value::Ref(shape)]);
        let pset = b.pset("RVTPSET", g.id, "Revit", props);
        b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEF", g.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(pset)]);
        contenus.entry(projet).or_default().push(Value::Ref(id));
    }
    // plans de référence et plans de construction du modèle
    let vecteur = |v: DVec3| Value::Str(format!("{:.4} ; {:.4} ; {:.4}", v.x, v.y, v.z));
    for p in m.plans_reference.iter().filter(|p| p.vue == -1) {
        let props = vec![
            ("ElementId", Value::Str(p.id.to_string())),
            ("Extrémité à bulle", point_mm(p.bulle)),
            ("Extrémité libre", point_mm(p.libre)),
            ("Direction", vecteur(p.direction)),
            ("Normale", vecteur(p.normale)),
            ("Est une référence", Value::Str(texte_reference(p.reference).into())),
            ("Définit l'origine", Value::Str((if p.definit_origine { "Oui" } else { "Non" }).into())),
        ];
        let id = b.add("IFCANNOTATION", vec![guid("RVT", p.id), n.clone(), Value::Str(p.nom.clone()), n.clone(), Value::Str("Plan de référence".into()), Value::Ref(shape), n.clone()]);
        let pset = b.pset("RVTPSET", p.id, "Revit", props);
        b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEF", p.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(pset)]);
        contenus.entry(projet).or_default().push(Value::Ref(id));
    }
    for p in &m.plans_construction {
        let props = vec![("ElementId", Value::Str(p.id.to_string())), ("Origine", point_mm(p.origine)), ("Normale", vecteur(p.axes.z_axis)), ("Axe X", vecteur(p.axes.x_axis))];
        let id = b.add("IFCANNOTATION", vec![guid("RVT", p.id), n.clone(), Value::Str(m.nom_plan_construction(p)), n.clone(), Value::Str("Plan de construction".into()), Value::Ref(shape), n.clone()]);
        let pset = b.pset("RVTPSET", p.id, "Revit", props);
        b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEF", p.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(pset)]);
        contenus.entry(projet).or_default().push(Value::Ref(id));
    }
    // images placées, imports CAO, liens Revit
    let nom_vue_de = |v: i64| if v == -1 { "Modèle".to_string() } else { m.vues.iter().find(|x| x.id == v).map(|x| x.nom.clone()).unwrap_or_else(|| v.to_string()) };
    for i in &m.images {
        let t = m.types_images.iter().find(|t| t.id == i.type_id);
        let mut props = vec![("ElementId", Value::Str(i.id.to_string())), ("Vue", Value::Str(nom_vue_de(i.vue))), ("Centre", point_mm(i.centre))];
        if let Some(t) = t {
            props.push(("Largeur", mm(t.largeur * i.echelle.0)));
            props.push(("Hauteur", mm(t.hauteur * i.echelle.1)));
            props.push(("Pixels", Value::Str(format!("{} × {}", t.largeur_px, t.hauteur_px))));
            props.push(("Résolution", Value::Str(format!("{} ppp", format_real(t.dpi)))));
            if let Some(c) = &t.chemin {
                props.push(("Fichier d'origine", Value::Str(c.clone())));
            }
            if let Some(r) = m.rasters.get(&t.donnees) {
                props.push(("Format", Value::Str(format!("{} ({} octets)", r.format, r.donnees.len()))));
            }
        }
        let nom = t.map(|t| t.nom.clone()).unwrap_or_default();
        let id = b.add("IFCANNOTATION", vec![guid("RVT", i.id), n.clone(), Value::Str(nom), n.clone(), Value::Str("Image".into()), Value::Ref(shape), n.clone()]);
        let pset = b.pset("RVTPSET", i.id, "Revit", props);
        b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEF", i.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(pset)]);
        contenus.entry(projet).or_default().push(Value::Ref(id));
    }
    for (genre, i) in m.imports.iter().map(|i| ("Import CAO", i)).chain(m.liens.iter().map(|i| ("Lien Revit", i))) {
        let mut props = vec![("ElementId", Value::Str(i.id.to_string())), ("Vue", Value::Str(nom_vue_de(i.vue))), ("Origine", point_mm(i.origine)), ("Axe X", vecteur(i.axes.x_axis))];
        let nom = if let Some(t) = m.types_imports.iter().find(|t| t.id == i.type_id) {
            props.push(("Taille du fichier", Value::Str(format!("{} octets", t.taille))));
            props.push(("Catégorie", Value::Str(m.nom_categorie(t.categorie))));
            props.push(("Lien", Value::Str((if t.lie { "Lié" } else { "Importé" }).into())));
            if !t.chemin.is_empty() {
                props.push(("Chemin", Value::Str(t.chemin.clone())));
            }
            props.push(("Unité", Value::Str(t.unite.rsplit(':').next().unwrap_or("").split('-').next().unwrap_or("").into())));
            t.nom.clone()
        } else if let Some(t) = m.types_liens.iter().find(|t| t.id == i.type_id) {
            props.push(("Chemin", Value::Str(t.chemin.clone())));
            props.push(("Chemin relatif", Value::Str((if t.relatif { "Oui" } else { "Non" }).into())));
            if i.nom.is_empty() { t.nom.clone() } else { format!("{} : {}", t.nom, i.nom) }
        } else {
            i.nom.clone()
        };
        let id = b.add("IFCANNOTATION", vec![guid("RVT", i.id), n.clone(), Value::Str(nom), n.clone(), Value::Str(genre.into()), Value::Ref(shape), n.clone()]);
        let pset = b.pset("RVTPSET", i.id, "Revit", props);
        b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEF", i.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(pset)]);
        contenus.entry(projet).or_default().push(Value::Ref(id));
    }
    // objets annexes : groupes, composants de légende, motifs de ligne, épaisseurs,
    // tables de clés, objets nommés, contraintes d'alignement (résumé)
    let mut annexe = |b: &mut Builder, eid: i64, genre: &str, nom: String, props: Vec<(&str, Value)>| {
        let id = b.add("IFCANNOTATION", vec![guid("RVT", eid), n.clone(), Value::Str(nom), n.clone(), Value::Str(genre.into()), Value::Ref(shape), n.clone()]);
        let pset = b.pset("RVTPSET", eid, "Revit", props);
        b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEF", eid), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(pset)]);
        contenus.entry(projet).or_default().push(Value::Ref(id));
    };
    let ids_texte = |l: &[i64]| {
        let mut t = l.iter().take(40).map(i64::to_string).collect::<Vec<_>>().join(", ");
        if l.len() > 40 {
            t += &format!(" … ({} en tout)", l.len());
        }
        t
    };
    for g in &m.groupes {
        let nom = m.types.get(&g.type_id).map(|t| t.nom.clone()).unwrap_or_default();
        let niveau = m.niveaux.iter().find(|l| l.id == g.niveau).map(|l| l.nom.clone()).unwrap_or_default();
        let props = vec![
            ("ElementId", Value::Str(g.id.to_string())),
            ("Point", point_mm(m.point_groupe(g))),
            ("Niveau", Value::Str(niveau)),
            ("Décalage", mm(g.decalage)),
            ("Vue", Value::Str(nom_vue_de(g.vue))),
            ("Membres", Value::Str(g.membres.len().to_string())),
            ("Membres (ElementId)", Value::Str(ids_texte(&g.membres))),
        ];
        annexe(&mut b, g.id, "Groupe", nom, props);
    }
    for l in m.legendes.iter().filter(|l| !l.apercu_type) {
        let props = vec![
            ("ElementId", Value::Str(l.id.to_string())),
            ("Vue", Value::Str(nom_vue_de(l.vue))),
            ("Vue du composant", Value::Str(l.vue_composant.to_string())),
            ("Longueur", mm(l.longueur)),
            ("Niveau de détail", Value::Str(l.detail.to_string())),
        ];
        annexe(&mut b, l.id, "Composant de légende", m.nom_element(-1_002_052, l.type_id).unwrap_or_default(), props);
    }
    let genre_segment = |g: i64| match g {
        0 => "trait",
        1 => "espace",
        2 => "point",
        _ => "?",
    };
    for x in m.motifs_ligne.values() {
        let segs = x.segments.iter().map(|(g, l)| format!("{} {} mm", genre_segment(*g), format_real((l * 1000.0).round() / 1000.0))).collect::<Vec<_>>().join(", ");
        annexe(&mut b, x.id, "Motif de ligne", x.nom.clone(), vec![("ElementId", Value::Str(x.id.to_string())), ("Segments", Value::Str(segs))]);
    }
    for x in &m.epaisseurs {
        let liste = |v: &[f64]| v.iter().map(|e| format_real((e * 1000.0).round() / 1000.0)).collect::<Vec<_>>().join(" ; ");
        let mut props = vec![("ElementId", Value::Str(x.id.to_string())), ("Perspective (mm)", Value::Str(liste(&x.perspective))), ("Annotation (mm)", Value::Str(liste(&x.annotation)))];
        let echelles: Vec<String> = x.modele.iter().map(|(e, v)| format!("1:{e} : {}", liste(v))).collect();
        props.push(("Modèle (mm, par échelle)", Value::Str(echelles.join(" | "))));
        annexe(&mut b, x.id, "Épaisseurs de ligne", String::new(), props);
    }
    for x in &m.tables_cles {
        let apercu = x.entrees.iter().take(20).map(|(k, _, t)| format!("{k} {t}")).collect::<Vec<_>>().join(" ; ");
        annexe(&mut b, x.id, x.genre, x.nom.clone(), vec![("ElementId", Value::Str(x.id.to_string())), ("Entrées", Value::Str(x.entrees.len().to_string())), ("Premières entrées", Value::Str(apercu))]);
    }
    for x in &m.objets_nommes {
        let mut props = vec![("ElementId", Value::Str(x.id.to_string()))];
        props.extend(x.props.iter().map(|(k, v)| (*k, Value::Str(v.clone()))));
        annexe(&mut b, x.id, x.genre, x.nom.clone(), props);
    }
    if !m.contraintes.is_empty() {
        let liees: std::collections::HashSet<i64> = m.contraintes.iter().flat_map(|c| c.references.iter().copied()).filter(|&e| e != -1).collect();
        let vues: std::collections::HashSet<i64> = m.contraintes.iter().map(|c| c.vue).collect();
        annexe(
            &mut b,
            -1,
            "Contraintes d'alignement",
            format!("{} contraintes", m.contraintes.len()),
            vec![("Éléments contraints", Value::Str(liees.len().to_string())), ("Vues", Value::Str(vues.len().to_string())), ("Premières (ElementId)", Value::Str(ids_texte(&m.contraintes.iter().map(|c| c.id).collect::<Vec<_>>())))],
        );
    }
    // Vues : un nœud par vue (dans le niveau d'un plan, sinon dans le projet), ses
    // éléments 2D en enfants, tracés dans le plan de la vue (masqués par défaut).
    // éléments 2D par vue, sauf ceux que Revit n'y montre pas (masqués, cotes dont un
    // élément coté n'est pas visible dans la vue)
    let boites = feuilles::boites(m);
    let par_id: HashMap<i64, &Vue> = m.vues.iter().map(|v| (v.id, v)).collect();
    let mut par_vue: BTreeMap<i64, Vec<&Element2D>> = BTreeMap::new();
    for e in m.elements_2d.iter().filter(|e| par_id.get(&e.vue).is_some_and(|v| feuilles::visible_dans_vue(m, e, v, &boites))) {
        par_vue.entry(e.vue).or_default().push(e);
    }
    let nom_vue: HashMap<i64, &str> = m.vues.iter().map(|v| (v.id, v.nom.as_str())).collect();
    let mut entite_vue: HashMap<i64, Id> = HashMap::new();
    // éléments 2D de chaque vue en primitives (export)
    let mut primitives_vues: HashMap<i64, Vec<Primitive>> = HashMap::new();
    for v in m.vues.iter().filter(|v| !v.gabarit && v.genre != "Arborescence") {
        let mut props = vec![
            ("ElementId", Value::Str(v.id.to_string())),
            ("Classe Revit", Value::Str(v.classe.clone())),
            ("Genre", Value::Str(v.genre.into())),
            ("Échelle", Value::Str(format!("1:{}", v.echelle))),
            ("Orientation", Value::Str((if v.nord_geographique { "Nord géographique" } else { "Nord du projet" }).into())),
            ("Plage de vue", Value::Str(texte_plage(m, v))),
            ("Origine", point_mm(v.origine)),
            ("Direction de vue", Value::Str(format!("{:.4} ; {:.4} ; {:.4}", v.direction.x, v.direction.y, v.direction.z))),
            ("Droite, haut", Value::Str(format!("{:.4} ; {:.4} ; {:.4} / {:.4} ; {:.4} ; {:.4}", v.droite.x, v.droite.y, v.droite.z, v.haut.x, v.haut.y, v.haut.z))),
        ];
        if let Some(niv) = m.niveaux.iter().find(|l| l.id == v.niveau) {
            props.push(("Niveau", Value::Str(niv.nom.clone())));
        }
        if let Some((_, _, l)) = v.cadrage {
            props.push(("Cadrage", Value::Str(format!("{} × {} × {} mm", format_real(l[0].round()), format_real(l[1].round()), format_real(l[2].round())))));
        }
        let elems = par_vue.get(&v.id).map(Vec::as_slice).unwrap_or_default();
        props.push(("Éléments 2D", Value::Str(elems.len().to_string())));
        let nom = if v.nom.is_empty() { format!("{} [{}]", v.genre, v.id) } else { format!("{} : {}", v.genre, v.nom) };
        let id_vue = b.add("IFCANNOTATION", vec![guid("RVTVUE", v.id), n.clone(), Value::Str(nom), n.clone(), Value::Str(v.genre.into()), n.clone(), Value::Ref(shape)]);
        entite_vue.insert(v.id, id_vue);
        let pset = b.pset("RVTPSETV", v.id, "Revit", props);
        b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEFV", v.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id_vue)]), Value::Ref(pset)]);
        let ps = b.psets_parametres(m, v.id);
        b.definir("RVTDEFW", v.id, id_vue, ps);
        contenus.entry(etage.get(&v.niveau).copied().unwrap_or(projet)).or_default().push(Value::Ref(id_vue));
        let normale = if v.direction.length() > 0.5 { v.direction.normalize() } else { DVec3::Z };
        let mut enfants = Vec::new();
        for e in elems {
            let (genre, couleur, mut props): (&str, Rgba, Vec<(&str, Value)>) = match &e.contenu {
                Contenu::Trait(p) => ("Trait", [0.12, 0.12, 0.12, 1.0], vec![("Longueur", mm(p.windows(2).map(|w| w[0].distance(w[1])).sum()))]),
                Contenu::Texte { texte, point, .. } => ("Texte", [0.80, 0.35, 0.10, 0.45], vec![("Texte", Value::Str(texte.clone())), ("Point", point_mm(*point))]),
                Contenu::Cote { segments, .. } => {
                    let valeurs = segments.iter().map(|s| format!("{} mm", format_real((s.0 * 10.0).round() / 10.0))).collect::<Vec<_>>().join(" + ");
                    ("Cote", [0.10, 0.35, 0.80, 1.0], vec![("Valeur", Value::Str(valeurs))])
                }
                Contenu::Region { type_id } => {
                    let t = m.types_regions.get(type_id);
                    let nom_motif = |id: i64| m.motifs.get(&id).map(|x| x.nom.clone()).unwrap_or_else(|| "aucun".into());
                    let mut p = vec![("Type", Value::Str(t.map(|t| t.nom.clone()).unwrap_or_else(|| type_id.to_string())))];
                    if let Some(t) = t {
                        p.push(("Motif de premier plan", Value::Str(nom_motif(t.avant))));
                        p.push(("Motif d'arrière-plan", Value::Str(nom_motif(t.arriere))));
                    }
                    ("Région remplie", [0.45, 0.45, 0.45, 0.6], p)
                }
                Contenu::Etiquette { params, hauteur, .. } => {
                    let exemple = params.iter().map(|p| format!("{}{}{}", p.prefixe, p.exemple, p.suffixe)).collect::<Vec<_>>().join(" ");
                    ("Étiquette", [0.80, 0.35, 0.10, 0.45], vec![("Texte d'exemple", Value::Str(exemple)), ("Hauteur du cadre", mm(*hauteur))])
                }
                Contenu::FenetreDeVue { vue, echelle, decalage, .. } => {
                    let nv = nom_vue.get(vue).map(|s| s.to_string()).unwrap_or_else(|| vue.to_string());
                    ("Fenêtre de vue", [0.5, 0.5, 0.5, 1.0], vec![("Vue placée", Value::Str(nv)), ("Échelle", Value::Str(format!("1:{}", (1.0 / echelle).round()))), ("Décalage", point_mm(*decalage))])
                }
            };
            props.splice(0..0, [("ElementId", Value::Str(e.id.to_string())), ("Classe Revit", Value::Str(e.classe.clone())), ("Vue", Value::Str(v.nom.clone()))]);
            let titre = match &e.contenu {
                Contenu::Texte { texte, .. } => format!("{genre} « {} » [{}]", texte.lines().next().unwrap_or_default(), e.id),
                _ => format!("{genre} [{}]", e.id),
            };
            let id = b.add("IFCANNOTATION", vec![guid("RVT2D", e.id), n.clone(), Value::Str(titre), n.clone(), Value::Str(genre.into()), n.clone(), Value::Ref(shape)]);
            let pset = b.pset("RVTPSET2D", e.id, "Revit", props);
            b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEF2D", e.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(pset)]);
            let ps = b.psets_parametres(m, e.id);
            b.definir("RVTDEF2P", e.id, id, ps);
            let mut prims = Vec::new();
            // note de texte : ses glyphes (le cadre seul si le texte est vide) ; cote :
            // ligne, lignes d'attache, marques et valeurs comme dans Revit
            if let Some((color, t)) = feuilles::note(m, e, v.echelle) {
                prims.extend(primitives_parts(vec![Part { color, tris: t.into_iter().map(|t| t.map(|p| p / 1000.0)).collect() }], Some(id)));
            } else if let Some(p) = feuilles::cote(m, e, normale, v.droite, v.echelle) {
                prims.extend(primitives_parts(p, Some(id)));
            } else {
                prims.extend(vues::primitives(e, normale, v.echelle, octets(couleur), Some(id)));
            }
            if let (Contenu::Region { type_id }, Ok(g)) = (&e.contenu, &e.geometrie) {
                prims.extend(region(m, *type_id, g, v, Some(id)));
            }
            let parts = parts_primitives(&prims, normale, v.echelle);
            if !parts.is_empty() {
                geoms.push(ProductGeom { id, parts, lignes: Vec::new(), rendu: Vec::new(), motifs: Vec::new() });
            }
            primitives_vues.entry(v.id).or_default().extend(prims);
            enfants.push(Value::Ref(id));
        }
        // instances propres à la vue (cartouches, symboles, composants de détail) et titres
        // des fenêtres de vue d'une feuille
        for o in feuilles::objets_vue(m, v) {
            let id = b.add("IFCANNOTATION", vec![guid("RVT2D", o.id), n.clone(), Value::Str(o.titre), n.clone(), Value::Str(o.genre.into()), n.clone(), Value::Ref(shape)]);
            let classe = if o.genre == "Titre de vue" { "Viewport" } else { "FamilyInstance" };
            let props = vec![("ElementId", Value::Str(o.id.to_string())), ("Classe Revit", Value::Str(classe.into())), ("Genre", Value::Str(o.genre.into())), ("Vue", Value::Str(v.nom.clone()))];
            let pset = b.pset("RVTPSET2I", o.id, "Revit", props);
            b.add("IFCRELDEFINESBYPROPERTIES", vec![gid("RVTDEF2I", o.id), n.clone(), n.clone(), n.clone(), Value::List(vec![Value::Ref(id)]), Value::Ref(pset)]);
            let ps = b.psets_parametres(m, o.id);
            b.definir("RVTDEF2J", o.id, id, ps);
            primitives_vues.entry(v.id).or_default().extend(primitives_parts(o.parts.clone(), Some(id)));
            geoms.push(ProductGeom { id, parts: o.parts, lignes: Vec::new(), rendu: Vec::new(), motifs: Vec::new() });
            enfants.push(Value::Ref(id));
        }
        if !enfants.is_empty() {
            b.add("IFCRELAGGREGATES", vec![gid("RVTAGGV", v.id), n.clone(), n.clone(), n.clone(), Value::Ref(id_vue), Value::List(enfants)]);
        }
    }
    for (k, (s, items)) in contenus.into_iter().enumerate() {
        b.add("IFCRELCONTAINEDINSPATIALSTRUCTURE", vec![gid("RVTCONT", k as i64), n.clone(), n.clone(), n.clone(), Value::List(items), Value::Ref(s)]);
    }
    let entite_type: HashMap<i64, Id> = types.iter().map(|(&(t, _), &e)| (t, e)).collect();
    let mut nav = navigateur(m, &entite_vue, &entite_type, &etage);
    nav.materiaux = m
        .materiaux
        .iter()
        .map(|(k, x)| scene::NavMateriau { nom: x.nom.clone(), couleur: x.couleur, transparence: x.transparence, elements: usages.remove(k).unwrap_or_default() })
        .collect();
    nav.materiaux.sort_by_key(|a| a.nom.to_lowercase());
    for v in &mut nav.vues {
        v.primitives = primitives_vues.remove(&v.revit_id).unwrap_or_default();
    }
    let model = IfcModel::new(StepFile { schema: format!("REVIT {format}"), entities: b.entities });
    let mut s = crate::formats::ifc::scene::assemble(model, geoms, file_name);
    for (&ent, &el) in &element_de {
        if let Some(mur) = m.murs.iter().find(|w| w.id == el) {
            if let Some(bandes) = bandes_couches_mur(m, mur, s.origin) { s.couches_coupe.insert(ent, bandes); }
        }
    }
    let mut nav = nav;
    for (&ent, &el) in &element_de {
        let ph = |k| id_parametre(m, el, k);
        if let Some(c) = ph(-1_012_100) {
            nav.phases_objets.insert(ent, (c, ph(-1_012_101).unwrap_or(-1)));
        }
        if m.variante(el).is_some_and(|v| !v.2) {
            nav.variantes_secondaires.insert(ent);
        }
    }
    s.navigateur = Some(nav);
    // coordonnées partagées du point cliqué (document placé)
    if let Some(e) = m.coordonnees.actif().filter(|_| m.coordonnees.place()) {
        let col = |j: usize| DVec3::new(e.lignes[0][j], e.lignes[1][j], e.lignes[2][j]);
        s.partage = Some(scene::ReperePartage { nom: e.nom.clone(), origine: e.origine / 1000.0, colonnes: [col(0), col(1), col(2)] });
    }
    for (nom, k) in [
        ("Faces à motif de surface de dessin (taille d'écran dans Revit), non tracé en 3D", motifs_dessin),
        ("Faces à motif de surface non tracé (limite de traits atteinte)", motifs_omis),
    ] {
        if k > 0 {
            s.unsupported.push((nom.into(), k));
        }
    }
    s
}

/// Famille système (catégorie, famille) d'une classe de type, comme dans le navigateur
/// de Revit ; None pour les types qu'il ne montre pas sous « Familles ».
pub(super) fn famille_systeme(classe: &str) -> Option<(&'static str, &'static str)> {
    Some(match classe {
        "BasicWallType" => ("Murs", "Mur de base"),
        "NewCurtainWallType" | "CurtainWallType" => ("Murs", "Mur-rideau"),
        "StackedWallType" => ("Murs", "Mur empilé"),
        "FloorAttributes" => ("Sols", "Sol"),
        "CeilingAttributes" => ("Plafonds", "Plafond composé"),
        "RoofAttributes" => ("Toits", "Toit de base"),
        "StairsType" => ("Escaliers", "Escalier assemblé"),
        "StairsRunType" => ("Escaliers", "Volée"),
        "StairsLandingType" => ("Escaliers", "Palier"),
        "BaseRailingSym" | "StairsRailingAttr" => ("Garde-corps", "Garde-corps"),
        "ToposolidType" => ("Toposolides", "Toposolide"),
        "GridAttributes" => ("Quadrillages", "Quadrillage"),
        "LevelAttributes" => ("Niveaux", "Niveau"),
        "TextNoteAttributes" => ("Annotations", "Note textuelle"),
        "FilledRegionAttributes" => ("Annotations", "Zone remplie"),
        _ => return None,
    })
}

/// Arborescence du projet : vues (hors gabarits), feuilles et vues qu'elles portent,
/// familles chargées par catégorie puis familles système, groupes.
/// Plage de vue d'un plan en clair : « Haut : Niveau 1 + 2300 mm ; Plan de coupe : … ».
pub(super) fn texte_plage(m: &Maquette, v: &Vue) -> String {
    let Some(p) = v.plage_vue else { return String::new() };
    let niveau = |n: i64| match n {
        -3 => "Niveau associé".to_string(),
        -4 => "Niveau au-dessus".to_string(),
        -2 => "Niveau en dessous".to_string(),
        -1 => "Illimité".to_string(),
        n => m.niveaux.iter().find(|x| x.id == n).map(|x| x.nom.clone()).unwrap_or_else(|| format!("#{n}")),
    };
    ["Haut", "Plan de coupe", "Bas", "Profondeur de vue"]
        .iter()
        .zip(p)
        .map(|(nom, (n, d))| if n == -1 { format!("{nom} : Illimité") } else { format!("{nom} : {} {:+} mm", niveau(n), format_real((d * 10.0).round() / 10.0).parse::<f64>().unwrap_or(d)) })
        .collect::<Vec<_>>()
        .join(" ; ")
}

/// Valeur ElementId d'un paramètre d'un élément (jeux de paramètres).
pub(super) fn id_parametre(m: &Maquette, element: i64, param: i64) -> Option<i64> {
    m.parametres.get(&element)?.iter().find(|(p, _)| *p == param).and_then(|(_, b)| if let Brut::Id(i) = b.selon(parametres::Stockage::Id) { Some(i) } else { None })
}

/// Valeur entière d'un paramètre d'un élément (jeux de paramètres).
pub(super) fn entier_parametre(m: &Maquette, element: i64, param: i64) -> Option<i64> {
    m.parametres.get(&element)?.iter().find(|(p, _)| *p == param).and_then(|(_, b)| if let Brut::Entier(i) = b.selon(parametres::Stockage::Entier) { Some(i) } else { None })
}

pub(super) fn navigateur(m: &Maquette, entite_vue: &HashMap<i64, Id>, entite_type: &HashMap<i64, Id>, etage: &HashMap<i64, Id>) -> Navigateur {
    let mut placees: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut fenetres: HashMap<i64, Vec<scene::NavFenetre>> = HashMap::new();
    let par_id: HashMap<i64, &Vue> = m.vues.iter().map(|v| (v.id, v)).collect();
    for e in &m.elements_2d {
        if let Contenu::FenetreDeVue { vue, decalage, echelle, .. } = &e.contenu {
            placees.entry(e.vue).or_default().push(*vue);
            if let Some(v) = par_id.get(vue) {
                let mut f = feuilles::fenetre(*decalage, *echelle, v);
                f.masques.extend(feuilles::masques_par_categorie(m, v));
                fenetres.entry(e.vue).or_default().push(f);
            }
        }
    }
    let nom_type = |t: i64| m.types.get(&t).map(|x| x.nom.clone()).unwrap_or_default();
    let vues = m
        .vues
        .iter()
        .filter(|v| !v.gabarit && v.genre != "Arborescence")
        .map(|v| NavVue {
            plan_coupe: None,
            limite_basse: None,
            cadrage: Vec::new(),
            revit_id: v.id,
            phase: id_parametre(m, v.id, -1_012_102).unwrap_or(0),
            filtre_phase: id_parametre(m, v.id, -1_012_103).unwrap_or(-1),
            coupe: v.plage_vue.map(|p| {
                // plan de coupe ramené au niveau de la vue
                let alt = |n: i64| m.niveaux.iter().find(|x| x.id == n).map(|x| x.altitude);
                let (n, d) = p[1];
                let ecart = if n == -3 || n == v.niveau { 0.0 } else { alt(n).zip(alt(v.niveau)).map_or(0.0, |(a, b)| a - b) };
                (ecart + d) / 1000.0
            }),
            entite: entite_vue.get(&v.id).copied(),
            genre: v.genre.to_string(),
            type_vue: nom_type(v.type_vue),
            nom: v.nom.clone(),
            echelle: v.echelle,
            detail: entier_parametre(m, v.id, -1_011_002).unwrap_or(0),
            numero: v.numero.clone(),
            haut: v.haut,
            direction: v.direction,
            etage: etage.get(&v.niveau).copied(),
            placees: placees.remove(&v.id).unwrap_or_default(),
            fenetres: fenetres.remove(&v.id).unwrap_or_default(),
            primitives: Vec::new(),
            tableau: v.nomenclature.as_ref().map(|d| {
                let t = crate::formats::rvt::nomenclatures::calculer(m, d);
                (t.titres, t.lignes, t.non_calculees, crate::formats::rvt::nomenclatures::texte_tris(d))
            }),
        })
        .collect();
    // familles chargées (catégorie, famille, types), puis familles système
    let mut par_famille: BTreeMap<(String, String), Vec<(String, Option<Id>)>> = BTreeMap::new();
    for (&id, f) in m.familles.iter().filter(|(_, f)| f.parent == -1) {
        let cle = (m.nom_categorie(f.categorie), f.nom.clone());
        let mut ts: Vec<(String, Option<Id>)> = m.types.iter().filter(|(_, t)| t.famille == id).map(|(&tid, t)| (t.nom.clone(), entite_type.get(&tid).copied())).collect();
        ts.sort();
        par_famille.entry(cle).or_default().extend(ts);
    }
    for (&tid, t) in &m.types {
        if let Some((cat, fam)) = famille_systeme(&t.classe) {
            par_famille.entry((cat.to_string(), fam.to_string())).or_default().push((t.nom.clone(), entite_type.get(&tid).copied()));
        }
    }
    let familles = par_famille
        .into_iter()
        .map(|((categorie, nom), mut types)| {
            types.sort();
            types.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
            NavFamille { categorie, nom, types }
        })
        .collect();
    let mut groupes: Vec<String> = m.types.values().filter(|t| t.classe == "ElementGroupType").map(|t| t.nom.clone()).collect();
    groupes.sort();
    let filtres_phase = m.filtres_phase.iter().map(|(&id, (_, p))| (id, *p)).collect();
    Navigateur { vues, familles, groupes, materiaux: Vec::new(), phases: m.ordre_phases.clone(), filtres_phase, phases_objets: Default::default(), variantes_secondaires: Default::default() }
}
