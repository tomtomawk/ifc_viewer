"""Validation indépendante du schéma IFC et de chaque représentation exportée.

Dépendances de développement : ifcopenshell, pytest (règles EXPRESS).
Usage : python tools/valider_ifc.py [--donnees] fichier.ifc

--donnees : vérifie aussi les données attendues d'un export BIMedit (IfcSite, au moins
un IfcRelDefinesByType, Pset_WallCommon et Qto_WallBaseQuantities sur chaque mur).
"""
import sys
import ifcopenshell
import ifcopenshell.geom
import ifcopenshell.validate


def donnees(modele):
    """Erreurs sur les données attendues d'un export BIMedit (liste vide si tout y est)."""
    erreurs = []
    if not modele.by_type("IfcSite"):
        erreurs.append("aucun IfcSite")
    if not modele.by_type("IfcRelDefinesByType"):
        erreurs.append("aucun IfcRelDefinesByType")
    for mur in modele.by_type("IfcWall"):
        jeux = {
            rel.RelatingPropertyDefinition.Name
            for rel in mur.IsDefinedBy
            if rel.is_a("IfcRelDefinesByProperties")
        }
        for attendu in ("Pset_WallCommon", "Qto_WallBaseQuantities"):
            if attendu not in jeux:
                erreurs.append(f"#{mur.id()} IfcWall : {attendu} absent")
    return erreurs


def valider(chemin, verifier_donnees=False):
    modele = ifcopenshell.open(chemin)
    journal = ifcopenshell.validate.json_logger()
    ifcopenshell.validate.validate(modele, journal, express_rules=True)
    erreurs = [e["message"] for e in journal.statements]
    geometries = 0
    reglages = ifcopenshell.geom.settings()
    for element in modele.by_type("IfcProduct"):
        if not element.Representation:
            continue
        try:
            forme = ifcopenshell.geom.create_shape(reglages, element)
            if not forme.geometry.verts:
                raise ValueError("géométrie vide")
            geometries += 1
        except Exception as erreur:
            erreurs.append(f"#{element.id()} {element.is_a()} : {erreur}")
    if verifier_donnees:
        erreurs.extend(donnees(modele))
    for erreur in erreurs:
        print(erreur, file=sys.stderr)
    print(f"{modele.schema} : {geometries} géométries, {len(erreurs)} erreurs")
    return 1 if erreurs else 0


if __name__ == "__main__":
    arguments = sys.argv[1:]
    verifier_donnees = "--donnees" in arguments
    fichiers = [a for a in arguments if a != "--donnees"]
    if len(fichiers) != 1:
        raise SystemExit("Usage : python tools/valider_ifc.py [--donnees] fichier.ifc")
    raise SystemExit(valider(fichiers[0], verifier_donnees))
