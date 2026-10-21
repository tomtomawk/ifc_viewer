#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod camera;
mod csg;
mod geom;
mod model;
mod render;
mod rvt;
mod scene;
mod step;

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

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
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("IFC Viewer")
            .with_inner_size([1500.0, 920.0])
            .with_drag_and_drop(true),
        depth_buffer: 24,
        multisampling: 4,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        "IFC Viewer",
        options,
        Box::new(move |cc| Ok(Box::new(app::ViewerApp::new(cc, path)))),
    ) {
        eprintln!("Erreur : {e}");
    }
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
        for g in gs {
            n += 1;
            match g {
                Ok(g) => {
                    ok += 1;
                    vol += g.controle().volume_m3;
                }
                // cause sans les ids d'éléments, pour regrouper
                Err(e) => *causes.entry(e.split(|c: char| c.is_ascii_digit()).collect::<Vec<_>>().join("#")).or_default() += 1,
            }
        }
        println!("{nom:<13}: {n} ({ok} maillés, {vol:.6} m3)");
        for (c, k) in &causes {
            println!("    non maillés : {k:>5}  {c}");
        }
    };
    bilan("Sols/plafonds", &mut m.dalles.iter().map(|x| &x.geometrie));
    bilan("Toits", &mut m.toits.iter().map(|x| &x.geometrie));
    bilan("Escaliers", &mut m.escaliers.iter().map(|x| &x.geometrie));
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
    println!("Instances de vue (ignorées) : {}", m.instances_de_vue);
    println!("Grilles      : {}", m.grilles.len());
    for g in &m.grilles {
        println!("  {:<34} ({:.1} ; {:.1}) → ({:.1} ; {:.1}){}", g.nom, g.debut.x, g.debut.y, g.fin.x, g.fin.y, g.arc.map(|(_, r)| format!(", rayon {r:.1}")).unwrap_or_default());
    }
    println!("Matériaux    : {}", m.materiaux.len());
    if !m.illisibles.is_empty() {
        println!("Objets illisibles : {}", m.illisibles.len());
    }
    println!("Durée géom.  : {} ms", t0.elapsed().as_millis());
    0
}
