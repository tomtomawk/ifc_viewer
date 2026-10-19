#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod camera;
mod csg;
mod geom;
mod model;
mod render;
mod scene;
mod step;

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Mode console : ifc_viewer --stats fichier.ifc
    if args.first().map(|s| s.as_str()) == Some("--stats") {
        let Some(path) = args.get(1) else {
            eprintln!("Usage : ifc_viewer --stats <fichier.ifc>");
            std::process::exit(2);
        };
        std::process::exit(stats(PathBuf::from(path)));
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
