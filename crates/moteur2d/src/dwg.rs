//! Export DWG (format natif d'AutoCAD) d'un dessin ou d'une feuille : le DXF du dessin
//! (`dxf.rs` : mêmes calques, couleurs et coordonnées en mm du papier) relu par
//! `acadrust` et réécrit en DWG binaire, version 2004 (AC1018 : ouverte par les versions
//! d'AutoCAD depuis 2004 et par DWG TrueView).
use acadrust::{DwgWriter, DxfReader};
use noyau::trace2d::Dessin;

/// Version DWG écrite.
pub const VERSION: acadrust::types::DxfVersion = acadrust::types::DxfVersion::AC1018;

/// DWG du dessin `d`.
pub fn dwg(d: &Dessin) -> Result<Vec<u8>, String> {
    depuis_dxf(&crate::dxf::dxf(d))
}

/// DWG d'un DXF ASCII.
pub fn depuis_dxf(texte: &str) -> Result<Vec<u8>, String> {
    let lecteur = DxfReader::from_reader(std::io::Cursor::new(texte.as_bytes().to_vec())).map_err(|e| format!("DWG : lecture du DXF : {e}"))?;
    let mut doc = lecteur.read().map_err(|e| format!("DWG : lecture du DXF : {e}"))?;
    doc.version = VERSION;
    // le DXF R12 n'a pas de table des calques : ceux des entités y sont ajoutés
    let noms: std::collections::BTreeSet<String> = doc.entities().map(|e| e.common().layer.clone()).collect();
    for nom in noms {
        if doc.layers.get(&nom).is_none() {
            doc.layers.add(acadrust::tables::Layer::new(nom)).map_err(|e| format!("DWG : calque : {e}"))?;
        }
    }
    DwgWriter::write_to_vec(&doc).map_err(|e| format!("DWG : écriture : {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un DXF de deux lignes sur deux calques donne un DWG 2004 relu avec ses entités.
    #[test]
    fn dwg_relu() {
        let dxf = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\nCOUPE\n62\n7\n10\n0.0\n20\n0.0\n30\n0.0\n11\n100.0\n21\n50.0\n31\n0.0\n\
0\nLINE\n8\nVU\n62\n8\n10\n0.0\n20\n10.0\n30\n0.0\n11\n80.0\n21\n10.0\n31\n0.0\n0\nENDSEC\n0\nEOF\n";
        let octets = depuis_dxf(dxf).unwrap();
        assert_eq!(&octets[..6], b"AC1018");
        let doc = acadrust::DwgReader::from_stream(std::io::Cursor::new(octets)).read().unwrap();
        let lignes: Vec<_> = doc.entities().filter_map(|e| match e { acadrust::EntityType::Line(l) => Some(l), _ => None }).collect();
        assert_eq!(lignes.len(), 2);
        // calques et couleurs gardés (le DXF R12 n'a pas de table des calques)
        assert!(lignes.iter().any(|l| l.common.layer == "COUPE" && l.common.color == acadrust::Color::Index(7) && (l.end.x - 100.0).abs() < 1e-6));
        assert!(lignes.iter().any(|l| l.common.layer == "VU" && l.common.color == acadrust::Color::Index(8)));
    }
}
