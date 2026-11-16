//! Mode console `--stats` pour un fichier Revit.

pub(crate) fn rvt_stats(path: &std::path::Path) -> i32 {
    let info = match formats::rvt::load(path) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Erreur : {e}");
            return 1;
        }
    };
    println!("Fichier      : {}", info.file_name);
    println!("Type         : {} Revit {}", info.kind, info.format().unwrap_or("?"));
    println!("Taille       : {:.1} Mo", info.file_size as f64 / 1e6);
    for (k, v) in &info.basic {
        println!("  {:<34} {v}", formats::rvt::basic_label(k));
    }
    if let Some(t) = &info.title {
        println!("Titre        : {t}");
    }
    if let Some(c) = &info.category {
        println!("Catégorie    : {c}");
    }
    for g in info.groups.iter().chain(&info.family_types) {
        println!("[{}]", g.title);
        for p in &g.params {
            println!("  {:<34} {}{}", p.name, p.value, p.units.as_deref().map(|u| format!(" {u}")).unwrap_or_default());
        }
    }
    if !info.external_refs.is_empty() {
        println!("Références externes :");
        for r in &info.external_refs {
            println!("  {:<24} {}  ({})", r.kind, r.path, r.load_state);
        }
    }
    let preview = match info.preview_png.as_deref() {
        None => "absente".to_string(),
        Some(p) => match formats::rvt::decode_png_rgba(p) {
            Some((w, h, _)) => format!("PNG {w} x {h} px ({} octets)", p.len()),
            None => format!("PNG illisible ({} octets)", p.len()),
        },
    };
    println!("Vignette     : {preview}");
    let n = info.streams.iter().filter(|s| !s.1).count();
    println!("Flux OLE     : {n}");
    for w in &info.warnings {
        println!("Avertissement : {w}");
    }
    let t0 = std::time::Instant::now();
    let m = match formats::rvt::maquette::lire(path, &|s| eprintln!("{s}")) {
        Ok(m) => m,
        Err(e) => {
            println!("Géométrie    : illisible ({e})");
            return 0;
        }
    };
    println!("Niveaux      : {}", m.niveaux.len());
    for n in &m.niveaux {
        println!("  {:<34} {} mm", n.nom, noyau::nombres::format_real((n.altitude * 10.0).round() / 10.0));
    }
    let mut genres = std::collections::BTreeMap::<&str, usize>::new();
    for d in m.definitions.values() {
        *genres.entry(d.genre.as_str()).or_default() += 1;
    }
    let partages = m.definitions.values().filter(|d| d.guid.is_some()).count();
    let occurrence = m.definitions.values().filter(|d| d.occurrence == Some(true)).count();
    let valeurs: usize = m.parametres.values().map(Vec::len).sum();
    println!(
        "Paramètres   : {} définitions ({}), dont {partages} avec GUID et {occurrence} de famille d'occurrence ; {valeurs} valeurs sur {} éléments",
        m.definitions.len(),
        genres.iter().map(|(g, n)| format!("{g} {n}")).collect::<Vec<_>>().join(", "),
        m.parametres.len()
    );
    println!("Identifiants : {} épisodes, {} éléments avec UniqueId", m.episodes.len(), m.episode_de.len());
    if let Ok(i) = formats::rvt::maquette::infos_document(path) {
        println!(
            "Document : logiciel {}, {} versions de Revit ({}), sous-projets {:?}, {} enregistrements (dernier par {}), {} documents de famille embarqués",
            i.logiciel,
            i.versions.len(),
            i.versions.last().map(String::as_str).unwrap_or(""),
            i.sous_projets,
            i.enregistrements.len(),
            i.enregistrements.last().map(|e| e.0.as_str()).unwrap_or(""),
            i.documents.len()
        );
    }
    println!(
        "Images et liens : {} types d'image du projet et {} des familles ({} fichiers, {} octets), {} images placées, {} vignettes d'apparence, {} imports CAO ({} types), {} liens Revit ({})",
        m.types_images.iter().filter(|t| t.famille == -1).count(),
        m.types_images.iter().filter(|t| t.famille != -1).count(),
        m.rasters.len(),
        m.rasters.values().map(|r| r.donnees.len()).sum::<usize>(),
        m.images.len(),
        m.apparences.values().filter(|a| a.vignette.is_some()).count(),
        m.imports.len(),
        m.types_imports.len(),
        m.liens.len(),
        m.types_liens.iter().map(|t| t.chemin.as_str()).collect::<Vec<_>>().join(", ")
    );
    println!(
        "Annexes : {} groupes ({} membres lus), {} jeux de variantes, {} options, {} éléments en variante, {} composants de légende, {} contraintes d'alignement, {} motifs de ligne, {} objets nommés",
        m.groupes.len(),
        m.groupe_de.len(),
        m.variantes.values().filter(|v| v.jeu).count(),
        m.variantes.values().filter(|v| !v.jeu).count(),
        m.variante_de.len(),
        m.legendes.iter().filter(|l| !l.apercu_type).count(),
        m.contraintes.len(),
        m.motifs_ligne.len(),
        m.objets_nommes.len()
    );
    let formules: usize = m.familles.values().map(|f| f.formules.len()).sum();
    let champs_calcules = m.vues.iter().filter_map(|v| v.nomenclature.as_ref()).flat_map(|d| &d.champs).filter(|c| c.formule.is_some()).count();
    println!("Formules : {formules} dans les familles, {champs_calcules} champs calculés de nomenclatures");
    let c = &m.coordonnees;
    if let Some(e) = c.actif() {
        let b = c.partage(c.point_base.unwrap_or_default()).unwrap_or_default() / 1000.0;
        let lieu = c.lieux.get(&e.lieu).map(|l| format!(", lieu « {} » {:.6}° {:.6}° (altitude {})", l.nom, l.latitude, l.longitude, l.altitude)).unwrap_or_default();
        println!(
            "Coordonnées : emplacement actif « {} » ({} emplacements), point de base E/O {:.3} N/S {:.3} Élév. {:.3} m, nord géographique {:.3}°{lieu}",
            e.nom,
            c.nommes().count(),
            b.x,
            b.y,
            b.z,
            e.angle().to_degrees()
        );
    }
    let lus: Vec<_> = m.murs.iter().filter_map(|w| Some((w, w.geometrie.as_ref().ok()?.controle()))).collect();
    let fermes = lus.iter().filter(|(_, c)| c.non_appariees == 0).count();
    println!("Murs         : {} ({} maillés, dont {fermes} fermés)", m.murs.len(), lus.len());
    if !lus.is_empty() {
        let (mut mn, mut mx) = (glam::DVec3::splat(f64::MAX), glam::DVec3::splat(f64::MIN));
        for (_, c) in &lus {
            mn = mn.min(c.min);
            mx = mx.max(c.max);
        }
        let size = (mx - mn) / 1000.0;
        println!("Triangles    : {}", lus.iter().map(|(_, c)| c.triangles).sum::<usize>());
        println!("Volume murs  : {:.6} m3", lus.iter().map(|(_, c)| c.volume_m3).sum::<f64>());
        println!("Emprise      : {:.2} x {:.2} x {:.2} m", size.x, size.y, size.z);
    }
    if m.murs.len() <= 100 {
        for w in &m.murs {
            let ty = m.types.get(&w.type_id).map(|t| t.nom.as_str()).unwrap_or("?");
            match &w.geometrie {
                Ok(g) => {
                    let c = g.controle();
                    println!("  {:>10} {:<8} {:>12.6} m3 {:>5} tri.  {ty}", w.id, w.classe, c.volume_m3, c.triangles);
                }
                Err(e) => println!("  {:>10} {:<8} non maillé : {e}", w.id, w.classe),
            }
        }
    } else {
        for w in m.murs.iter().filter(|w| w.geometrie.is_err()) {
            println!("  {:>10} {:<8} non maillé : {}", w.id, w.classe, w.geometrie.as_ref().err().unwrap());
        }
    }
    let bilan = |nom: &str, gs: &mut dyn Iterator<Item = &formats::rvt::maquette::Geometrie>| {
        let (mut n, mut ok, mut vol) = (0, 0, 0.0);
        let mut causes: std::collections::BTreeMap<String, usize> = Default::default();
        let mut partiels: std::collections::BTreeMap<String, usize> = Default::default();
        // cause sans les ids d'éléments, pour regrouper
        let cle = |e: &str| e.split(|c: char| c.is_ascii_digit()).collect::<Vec<_>>().join("#");
        for g in gs {
            n += 1;
            match g {
                Ok(g) => {
                    ok += 1;
                    vol += g.controle().volume_m3;
                    for e in &g.echecs {
                        *partiels.entry(cle(e)).or_default() += 1;
                    }
                }
                Err(e) => *causes.entry(cle(e)).or_default() += 1,
            }
        }
        println!("{nom:<13}: {n} ({ok} maillés, {vol:.6} m3)");
        for (c, k) in &causes {
            println!("    non maillés : {k:>5}  {c}");
        }
        for (c, k) in &partiels {
            println!("    solides écartés : {k:>5}  {c}");
        }
    };
    bilan("Sols/plafonds", &mut m.dalles.iter().map(|x| &x.geometrie));
    bilan("Toposolides", &mut m.toposolides.iter().map(|x| &x.geometrie));
    for t in &m.toposolides {
        if let Ok(g) = &t.geometrie {
            let c = g.controle();
            let nom = m.types.get(&t.type_id).map(|x| x.nom.as_str()).unwrap_or("?");
            println!("  {:>10} {:<22} niveau {:>9} décalage {:>9.1} mm  {:.6} m3  {:.4} m2  {} faces", t.id, nom, t.niveau, t.decalage, c.volume_m3, c.aire_m2, g.faces.len());
        }
    }
    bilan("Surfaces topographiques", &mut m.surfaces_topo.iter().map(|x| &x.geometrie));
    for s in &m.surfaces_topo {
        if let Ok(g) = &s.geometrie {
            let c = g.controle();
            println!("  {:>10} hôte {:>9}  {} points  {} sommets  {} triangles  {:.6} m2  boîte {:.3?} – {:.3?} mm", s.id, s.hote, s.points, g.sommets.len(), c.triangles, c.aire_m2, c.min, c.max);
        }
    }
    bilan("Toits", &mut m.toits.iter().map(|x| &x.geometrie));
    bilan("Escaliers", &mut m.escaliers.iter().map(|x| &x.geometrie));
    bilan("Garde-corps", &mut m.garde_corps.iter().map(|x| &x.geometrie));
    bilan("Pièces", &mut m.pieces.iter().filter(|p| p.genre == "piece" && p.point.is_some()).map(|x| &x.geometrie));
    for genre in ["piece", "surface", "zone"] {
        let n = m.pieces.iter().filter(|p| p.genre == genre).count();
        let places = m.pieces.iter().filter(|p| p.genre == genre && p.point.is_some()).count();
        println!("  {genre:<34} {n} ({places} placées)");
    }
    bilan("Instances", &mut m.instances.iter().map(|x| &x.geometrie));
    let mut cats: std::collections::BTreeMap<String, usize> = Default::default();
    for i in &m.instances {
        *cats.entry(m.categorie(i).0).or_default() += 1;
    }
    for (c, n) in &cats {
        println!("  {c:<34} {n}");
    }
    println!("Instances de vue (cartouches, symboles, composants de détail) : {}", m.instances_de_vue);
    bilan("Formes", &mut m.formes.iter().map(|x| &x.geometrie));
    for f in &m.formes {
        let g = f.geometrie.as_ref().map(|g| format!("{:.6} m3", g.controle().volume_m3)).unwrap_or_else(|e| e.clone());
        let sc = if f.sous_categorie == -1 { String::new() } else { m.nom_categorie(f.sous_categorie) };
        println!("  {:>10} {:<22} {:<16} {g}", f.id, f.classe, sc);
    }
    println!("Vues         : {}", m.vues.len());
    for v in &m.vues {
        let n2d = m.elements_2d.iter().filter(|e| e.vue == v.id).count();
        println!("  {:>10} {:<16} 1:{:<5} {:>4} él. 2D  {}{}", v.id, v.genre, v.echelle, n2d, v.nom, if v.gabarit { " (gabarit)" } else { "" });
    }
    let mut c2d: std::collections::BTreeMap<&str, usize> = Default::default();
    for e in &m.elements_2d {
        *c2d.entry(e.classe.as_str()).or_default() += 1;
    }
    println!("Éléments 2D  : {} {:?}", m.elements_2d.len(), c2d);
    // régions remplies : unies, hachurées (nombre de traits découpés), sans type connu
    let (mut unies, mut hachurees, mut traits, mut sans_type) = (0, 0, 0, 0);
    for e in &m.elements_2d {
        let formats::rvt::vues::Contenu::Region { type_id } = &e.contenu else { continue };
        let (Some(t), Some(v), Ok(g)) = (m.types_regions.get(type_id), m.vues.iter().find(|v| v.id == e.vue), &e.geometrie) else {
            sans_type += 1;
            continue;
        };
        match m.motifs.get(&t.avant) {
            Some(x) if !x.grilles.is_empty() => {
                let face: Vec<[glam::DVec3; 3]> = g.faces.iter().flat_map(|f| f.triangles.iter().map(|t| t.map(|i| g.sommets[i]))).collect();
                traits += formats::rvt::vues::hachures(&face, x, v.echelle, v.droite, v.haut, 20_000).len();
                hachurees += 1;
            }
            _ => unies += 1,
        }
    }
    println!("Motifs       : {} ; types de régions : {} ; régions unies {unies}, hachurées {hachurees} ({traits} traits), sans type {sans_type}", m.motifs.len(), m.types_regions.len());
    println!("Grilles      : {}", m.grilles.len());
    for g in &m.grilles {
        println!("  {:<34} ({:.1} ; {:.1}) → ({:.1} ; {:.1}){}", g.nom, g.debut.x, g.debut.y, g.fin.x, g.fin.y, g.arc.map(|(_, r)| format!(", rayon {r:.1}")).unwrap_or_default());
    }
    println!("Matériaux    : {}", m.materiaux.len());
    if !m.illisibles.is_empty() {
        println!("Objets illisibles : {}", m.illisibles.len());
    }
    println!("Durée géom.  : {} ms", t0.elapsed().as_millis());
    // scène du viewer : objets, triangles, traits (motifs de surface)
    let t1 = std::time::Instant::now();
    let s = formats::rvt::maquette::scene(&m, "", String::new(), "");
    let traits: u32 = s.objects.iter().map(|o| o.lines.count / 2).sum();
    let motifs = s.objects.iter().filter(|o| &*o.class == "IFCSURFACEFEATURE").count();
    println!("Scène        : {} objets, {} triangles, {traits} traits ({motifs} objets de motifs), {} ms", s.objects.len(), s.triangles, t1.elapsed().as_millis());
    for (nom, n) in &s.unsupported {
        println!("  {nom} : {n}");
    }
    0
}
