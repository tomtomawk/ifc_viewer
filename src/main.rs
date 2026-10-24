#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod camera;
mod csg;
mod dessin;
mod geom;
mod model;
mod modele;
mod pilote;
mod render;
mod rvt;
mod scene;
mod step;
mod texte;
mod trace2d;

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Mode console : ifc_viewer --decodage fichier.rvt|.rfa|.rte (part décodée, JSON)
    if args.first().map(|s| s.as_str()) == Some("--decodage") {
        let Some(path) = args.get(1) else {
            eprintln!("Usage : ifc_viewer --decodage <fichier.rvt|.rfa|.rte>");
            std::process::exit(2);
        };
        std::process::exit(rvt_decodage(&PathBuf::from(path)));
    }
    // Mode console : ifc_viewer --nomenclatures fichier.rvt (contenu calculé, JSON par ligne)
    if args.first().map(|s| s.as_str()) == Some("--nomenclatures") {
        let Some(path) = args.get(1) else {
            eprintln!("Usage : ifc_viewer --nomenclatures <fichier.rvt>");
            std::process::exit(2);
        };
        std::process::exit(rvt_nomenclatures(&PathBuf::from(path)));
    }
    // Mode console : ifc_viewer --parametres fichier.rvt (valeurs brutes, unités internes)
    if args.first().map(|s| s.as_str()) == Some("--parametres") {
        let Some(path) = args.get(1) else {
            eprintln!("Usage : ifc_viewer --parametres <fichier.rvt|.rfa|.rte>");
            std::process::exit(2);
        };
        std::process::exit(rvt_parametres(&PathBuf::from(path)));
    }
    // Mode console : ifc_viewer --modele fichier.rvt|.ifc [sortie.json]
    if args.first().map(|s| s.as_str()) == Some("--modele") {
        let Some(path) = args.get(1) else {
            eprintln!("Usage : ifc_viewer --modele <fichier.rvt|.rfa|.ifc> [sortie.json]");
            std::process::exit(2);
        };
        let entree = PathBuf::from(path);
        let sortie = args.get(2).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("{path}.modele.json")));
        std::process::exit(modele(entree, sortie));
    }
    // Mode console : ifc_viewer --stats fichier.ifc
    if args.first().map(|s| s.as_str()) == Some("--stats") {
        let Some(path) = args.get(1) else {
            eprintln!("Usage : ifc_viewer --stats <fichier.ifc|.rvt|.rfa|.rte>");
            std::process::exit(2);
        };
        let path = PathBuf::from(path);
        let code = if rvt::is_revit_path(&path) { rvt_stats(&path) } else { stats(path) };
        std::process::exit(code);
    }

    let path = args.first().map(PathBuf::from);
    // anticrénelage 4×, sinon sans : pilotes ou serveurs X sans configuration
    // multiéchantillonnée (machines virtuelles, bureau à distance, Xvfb)
    for multisampling in [4, 0] {
        let options = eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_title("IFC Viewer")
                .with_inner_size([1500.0, 920.0])
                .with_drag_and_drop(true),
            depth_buffer: 24,
            multisampling,
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        };
        let path = path.clone();
        match eframe::run_native("IFC Viewer", options, Box::new(move |cc| Ok(Box::new(app::ViewerApp::new(cc, path))))) {
            Ok(()) => return,
            Err(e) => eprintln!("Erreur (anticrénelage {multisampling}×) : {e}"),
        }
    }
}

/// Conversion vers le modèle v0 : projet JSON, et rapport (Markdown) à côté et sur la
/// sortie standard.
fn modele(entree: PathBuf, sortie: PathBuf) -> i32 {
    let t0 = std::time::Instant::now();
    let e = entree.clone();
    let handle = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let progress = |s: String| eprintln!("{s}");
            if rvt::is_revit_path(&e) { modele::depuis_rvt::convertir(&e, &progress) } else { modele::depuis_ifc::convertir(&e, &progress) }
        })
        .unwrap();
    let (projet, mesures) = match handle.join() {
        Ok(Ok(x)) => x,
        Ok(Err(e)) => {
            eprintln!("Erreur : {e}");
            return 1;
        }
        Err(_) => {
            eprintln!("Erreur : la conversion a échoué (panique)");
            return 1;
        }
    };
    let rapport = modele::rapport::markdown(&projet, &mesures, t0.elapsed().as_secs_f64());
    let json = match projet.json() {
        Ok(j) => j,
        Err(e) => {
            eprintln!("Erreur : {e}");
            return 1;
        }
    };
    // aller-retour : le fichier écrit se relit à l'identique
    match modele::Projet::lire_json(&json) {
        Ok(p) if p.murs.len() == projet.murs.len() && p.importes.len() == projet.importes.len() => {}
        Ok(_) => eprintln!("Attention : le projet relu diffère du projet écrit"),
        Err(e) => eprintln!("Attention : projet écrit illisible ({e})"),
    }
    let chemin_rapport = sortie.with_extension("rapport.md");
    if let Err(e) = std::fs::write(&sortie, json).and_then(|_| std::fs::write(&chemin_rapport, &rapport)) {
        eprintln!("Écriture impossible : {e}");
        return 1;
    }
    println!("{rapport}");
    eprintln!("Projet : {}\nRapport : {}", sortie.display(), chemin_rapport.display());
    0
}

fn stats(path: PathBuf) -> i32 {
    let handle = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || scene::load(&path, &|s| eprintln!("{s}")))
        .unwrap();
    match handle.join().unwrap() {
        Ok(s) => {
            println!("Fichier      : {}", s.file_name);
            println!("Schéma       : {}", s.model.file.schema);
            println!("Entités      : {}", s.model.file.entities.len());
            println!("Unité        : 1 unité = {} m", s.model.length_scale);
            println!("Produits     : {}", s.model.products.len());
            println!("Objets 3D    : {}", s.objects.len());
            println!("Triangles    : {}", s.triangles);
            let size = s.max - s.min;
            println!("Emprise      : {:.2} x {:.2} x {:.2} m", size.x, size.y, size.z);
            println!("Origine      : {:.3?}", s.origin);
            println!("Durée        : {} ms", s.load_ms);
            let mut by_class = std::collections::BTreeMap::<String, usize>::new();
            for o in &s.objects {
                *by_class.entry(o.class.to_string()).or_default() += 1;
            }
            println!("Par classe   :");
            for (c, n) in by_class {
                println!("  {:<32} {n}", model::pretty_type(&c));
            }
            if s.objects.len() <= 50 {
                println!("Volumes (m3) :");
                for o in &s.objects {
                    let mut v = 0.0f64;
                    for r in [o.opaque, o.transparent] {
                        for t in s.vertices[r.start as usize..(r.start + r.count) as usize].chunks_exact(3) {
                            let [a, b, c] = [0, 1, 2].map(|i| glam::Vec3::from(t[i].pos).as_dvec3());
                            v += a.dot(b.cross(c)) / 6.0;
                        }
                    }
                    println!("  {:<32} {:.4}", s.model.label(o.id), v);
                }
            }
            let no_geom = s.model.products.len() - s.objects.len();
            println!("Produits sans géométrie : {no_geom}");
            if !s.unsupported.is_empty() {
                println!("Non pris en charge :");
                for (t, n) in &s.unsupported {
                    println!("  {:<32} {n}", model::pretty_type(t));
                }
            }
            0
        }
        Err(e) => {
            eprintln!("Erreur : {e}");
            1
        }
    }
}

/// Valeurs de paramètres décodées, une par ligne : élément, paramètre, stockage, valeur
/// brute (unités internes de Revit), pour comparaison avec l'API.
fn rvt_parametres(path: &std::path::Path) -> i32 {
    use rvt::parametres::{Brut, Stockage, integres};
    let m = match rvt::maquette::lire(path, &|_| {}) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Erreur : {e}");
            return 1;
        }
    };
    let mut ids: Vec<&i64> = m.parametres.keys().collect();
    ids.sort();
    for e in ids {
        for (p, b) in &m.parametres[e] {
            let st = m.definitions.get(p).map(|d| d.stockage).or_else(|| integres().get(p).map(|i| i.stockage)).unwrap_or(Stockage::Aucun);
            let (s, v) = match b.selon(st) {
                Brut::Reel(x) => ("Double", format!("{x:?}")),
                Brut::Entier(i) => ("Integer", i.to_string()),
                Brut::Id(i) => ("ElementId", i.to_string()),
                Brut::Texte(t) => ("String", t.replace('\\', "\\\\").replace('\t', "\\t").replace('\n', "\\n").replace('\r', "\\r")),
                Brut::Nomme { .. } => ("?", String::new()),
            };
            println!("{e}\t{p}\t{s}\t{v}");
        }
    }
    // IfcGUID calculés (paramètres IFC_GUID et IFC_TYPE_GUID de l'API)
    let mut ids: Vec<&i64> = m.episode_de.keys().collect();
    ids.sort();
    for e in ids {
        if let Some(g) = m.guid_ifc(*e) {
            println!("{e}\tifcguid\tString\t{g}");
        }
    }
    0
}

/// Contenu calculé de chaque nomenclature : une ligne JSON {id, nom, titres, lignes,
/// non_calculees}.
fn rvt_decodage(path: &std::path::Path) -> i32 {
    match rvt::maquette::decodage(path) {
        Ok(d) => {
            println!(
                "{{\"flux\":[{},{}],\"non_lus\":[{}],\"elements\":[{},{},{}],\"octets_elements\":[{},{},{}],\"geometries\":[{},{}],\"octets_geometries\":[{},{}]}}",
                d.flux.0,
                d.flux.1,
                d.non_lus.iter().map(|(p, n)| format!("[{},{n}]", pilote::texte(p))).collect::<Vec<_>>().join(","), d.elements.0, d.elements.1, d.elements.2, d.octets_elements.0, d.octets_elements.1, d.octets_elements.2, d.geometries.0, d.geometries.1, d.octets_geometries.0, d.octets_geometries.1
            );
            0
        }
        Err(e) => {
            eprintln!("Erreur : {e}");
            1
        }
    }
}

fn rvt_nomenclatures(path: &std::path::Path) -> i32 {
    let m = match rvt::maquette::lire(path, &|_| {}) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Erreur : {e}");
            return 1;
        }
    };
    let j = |s: &str| pilote::texte(s);
    for v in &m.vues {
        let Some(d) = &v.nomenclature else { continue };
        let t = rvt::nomenclatures::calculer(&m, d);
        let liste = |l: &[String]| format!("[{}]", l.iter().map(|x| j(x)).collect::<Vec<_>>().join(","));
        let lignes = t.lignes.iter().map(|l| liste(l)).collect::<Vec<_>>().join(",");
        let nc = t.non_calculees.iter().map(usize::to_string).collect::<Vec<_>>().join(",");
        // colonnes visibles calculées par une formule
        let formules = d.champs.iter().filter(|c| !c.masque).enumerate().filter(|(_, c)| c.genre == rvt::nomenclatures::GenreChamp::Formule).map(|(k, _)| k.to_string()).collect::<Vec<_>>().join(",");
        println!("{{\"id\":{},\"nom\":{},\"titres\":{},\"lignes\":[{lignes}],\"non_calculees\":[{nc}],\"formules\":[{formules}]}}", v.id, j(&v.nom), liste(&t.titres));
    }
    0
}

fn rvt_stats(path: &std::path::Path) -> i32 {
    let info = match rvt::load(path) {
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
        println!("  {:<34} {v}", rvt::basic_label(k));
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
        Some(p) => match rvt::decode_png_rgba(p) {
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
    let m = match rvt::maquette::lire(path, &|s| eprintln!("{s}")) {
        Ok(m) => m,
        Err(e) => {
            println!("Géométrie    : illisible ({e})");
            return 0;
        }
    };
    println!("Niveaux      : {}", m.niveaux.len());
    for n in &m.niveaux {
        println!("  {:<34} {} mm", n.nom, step::format_real((n.altitude * 10.0).round() / 10.0));
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
    if let Ok(i) = rvt::maquette::infos_document(path) {
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
    let bilan = |nom: &str, gs: &mut dyn Iterator<Item = &rvt::maquette::Geometrie>| {
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
        let rvt::vues::Contenu::Region { type_id } = &e.contenu else { continue };
        let (Some(t), Some(v), Ok(g)) = (m.types_regions.get(type_id), m.vues.iter().find(|v| v.id == e.vue), &e.geometrie) else {
            sans_type += 1;
            continue;
        };
        match m.motifs.get(&t.avant) {
            Some(x) if !x.grilles.is_empty() => {
                let face: Vec<[glam::DVec3; 3]> = g.faces.iter().flat_map(|f| f.triangles.iter().map(|t| t.map(|i| g.sommets[i]))).collect();
                traits += rvt::vues::hachures(&face, x, v.echelle, v.droite, v.haut, 20_000).len();
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
    let s = rvt::maquette::scene(&m, "", String::new(), "");
    let traits: u32 = s.objects.iter().map(|o| o.lines.count / 2).sum();
    let motifs = s.objects.iter().filter(|o| &*o.class == "IFCSURFACEFEATURE").count();
    println!("Scène        : {} objets, {} triangles, {traits} traits ({motifs} objets de motifs), {} ms", s.objects.len(), s.triangles, t1.elapsed().as_millis());
    for (nom, n) in &s.unsupported {
        println!("  {nom} : {n}");
    }
    0
}
