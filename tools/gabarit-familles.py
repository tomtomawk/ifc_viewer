"""Régénère la section types_ouvertures du gabarit par défaut depuis le code
(crates/app/src/modele/famille/modelee/catalogue.rs), en gardant la mise en forme du fichier.

Usage : python tools/gabarit-familles.py   (CARGO_TARGET_DIR respecté)
"""
import os
import subprocess
from pathlib import Path

racine = Path(__file__).resolve().parent.parent
subprocess.run(["cargo", "test", "--release", "--bin", "bimedit", "--", "--ignored", "ecrire_familles_du_gabarit"], cwd=racine, check=True, env=os.environ)
lignes = (racine / "target" / "familles-gabarit.json").read_text(encoding="utf-8").splitlines()
gabarit = racine / "gabarits" / "defaut.gabarit.json"
texte = gabarit.read_bytes().decode("utf-8")
debut = texte.index(' "types_ouvertures": [')
fin = texte.index("\n ]", debut) + len("\n ]")
nl = "\r\n" if "\r\n" in texte else "\n"
section = ' "types_ouvertures": [' + nl + ("," + nl).join("  " + l for l in lignes) + nl + " ]"
gabarit.write_bytes((texte[:debut] + section + texte[fin:]).encode("utf-8"))
print(f"{len(lignes)} familles écrites dans {gabarit.name}")
