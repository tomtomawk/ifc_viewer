//! Mode console `--stats` pour un fichier IFC.

use std::path::PathBuf;

pub(crate) fn stats(path: PathBuf) -> i32 {
    let handle = match std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || formats::ifc::assemblage::load(&path, &|s| eprintln!("{s}")))
    {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Erreur : impossible de lancer la lecture ({e})");
            return 1;
        }
    };
    let Ok(resultat) = handle.join() else {
        eprintln!("Erreur : la lecture a échoué (panique)");
        return 1;
    };
    match resultat {
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
                println!("  {:<32} {n}", ifc_base::semantique::pretty_type(&c));
            }
            if s.objects.len() <= 50 {
                println!("Volumes (m3) :");
                for o in &s.objects {
                    let mut v = 0.0f64;
                    for r in [o.opaque, o.transparent] {
                        for t in s.vertices[r.start as usize..(r.start + r.count) as usize].as_chunks::<3>().0 {
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
                    println!("  {:<32} {n}", ifc_base::semantique::pretty_type(t));
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
