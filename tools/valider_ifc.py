"""Validation indépendante du schéma IFC et de chaque représentation exportée.

Dépendances de développement : ifcopenshell, pytest (règles EXPRESS).
Usage : python tools/valider_ifc.py fichier.ifc
"""
import sys
import ifcopenshell
import ifcopenshell.geom
import ifcopenshell.validate


def valider(chemin):
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
    for erreur in erreurs:
        print(erreur, file=sys.stderr)
    print(f"{modele.schema} : {geometries} géométries, {len(erreurs)} erreurs")
    return 1 if erreurs else 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("Usage : python tools/valider_ifc.py fichier.ifc")
    raise SystemExit(valider(sys.argv[1]))
