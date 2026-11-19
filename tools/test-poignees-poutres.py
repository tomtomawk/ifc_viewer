"""Poignées d'une poutre sélectionnée (docs/MODIFICATION.md §1) jouées à la souris dans
une instance isolée : extrémité glissée (allonger), Ctrl+Z, milieu glissé (déplacer).

python tools/test-poignees-poutres.py --exe target/release/bimedit [--capture poutre.png]
"""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time


def run(exe, capture):
    with tempfile.TemporaryDirectory(prefix="bimedit-poignees-poutres-") as dossier:
        root = Path(dossier)
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0)); port = s.getsockname()[1]
        env = dict(os.environ, BIMEDIT_PORT=str(port), APPDATA=str(root / "configuration"), BIMEDIT_CACHE_COLLABORATION=str(root / "cache"))
        log = (root / "instance.log").open("w", encoding="utf-8")

        def commande(nom, argument=None, ok=True):
            if argument is not None and not isinstance(argument, str):
                argument = json.dumps(argument, ensure_ascii=False, separators=(",", ":"))
            ligne = nom + (" " + argument if argument is not None else "")
            with socket.create_connection(("127.0.0.1", port), timeout=60) as s:
                s.sendall((ligne + "\n").encode()); r = json.loads(s.makefile("r", encoding="utf-8").readline())
            assert r.get("ok") == ok, (nom, r)
            return r

        def attendre(f, timeout=15):
            fin = time.monotonic() + timeout
            while time.monotonic() < fin:
                try:
                    r = f()
                except OSError:
                    r = None
                if r: return r
                time.sleep(.1)
            raise AssertionError("Délai dépassé")

        def saisie(entrees):
            commande("saisie", entrees)
            attendre(lambda: commande("etat")["saisies_en_attente"] == 0)
            time.sleep(.15)

        def glisser(a, b):
            saisie([{"action": "pointer", "x": a[0], "y": a[1]}])
            saisie([{"action": "bouton", "x": a[0], "y": a[1], "bouton": "gauche", "appuye": True},
                    {"action": "pointer", "x": (a[0] + b[0]) / 2, "y": (a[1] + b[1]) / 2},
                    {"action": "pointer", "x": b[0], "y": b[1]}])
            saisie([{"action": "bouton", "x": b[0], "y": b[1], "bouton": "gauche", "appuye": False}])

        def poutre(id):
            return next(b for b in commande("projet_natif")["projet"]["barres"] if b["id"] == id)

        p = subprocess.Popen([str(exe.resolve())], env=env, stdout=log, stderr=log)
        try:
            attendre(lambda: commande("etat")["ok"], 60)
            commande("nouveau", {"chemin": str(root / "Poutres.modele.json")})
            niveau = commande("projet_natif")["projet"]["niveaux"][0]["id"]
            r = commande("editer", {"action": "barre", "barre": {"id": "", "nom": "Poutre", "genre": "poutre", "niveau": niveau,
                                                               "debut": [0, 0, 2600], "fin": [4000, 0, 2600], "largeur": 200, "hauteur": 400}})
            id = r["ids"][0] if "ids" in r else poutre_id(commande)
            commande("camera", "cadrer")
            commande("outil", "aucun")
            commande("selection_native", [id])
            ax = commande("axe_selection")
            assert ax["debut"] == [0, 0] or abs(ax["debut"][0]) < 1, ax
            # 1. extrémité de fin glissée vers la droite (Maj : axe) -> allongée, Z gardé
            d, f = ax["debut_ecran"], ax["fin_ecran"]
            cible = [f[0] + (f[0] - d[0]) * 0.5, f[1] + (f[1] - d[1]) * 0.5]
            glisser(f, cible)
            b = poutre(id)
            assert b["debut"] == [0, 0, 2600], b
            assert b["fin"][0] > 5000 and b["fin"][2] == 2600, b
            print("extrémité :", b["debut"], b["fin"])
            if capture:
                commande("selection_native", [id])
                time.sleep(.5)
                commande("capture", "fenetre " + str(Path(capture).resolve()))
            # 2. une seule étape d'annulation
            commande("annuler")
            assert poutre(id)["fin"] == [4000, 0, 2600], poutre(id)
            # 3. milieu glissé -> déplacée
            commande("selection_native", [id])
            ax = commande("axe_selection")
            d, f = ax["debut_ecran"], ax["fin_ecran"]
            m = [(d[0] + f[0]) / 2, (d[1] + f[1]) / 2]
            glisser(m, [m[0], m[1] + 80])
            b = poutre(id)
            assert abs(b["fin"][0] - b["debut"][0] - 4000) < 1 and b["debut"][1] != 0 and b["debut"][2] == 2600, b
            print("milieu :", b["debut"], b["fin"])
            commande("annuler")
            assert poutre(id)["debut"] == [0, 0, 2600]
            # Repartir près d'un about hors grille : le tracé doit reprendre ses XY exacts.
            fin = [1378, 1589, 2800]
            commande("editer", {"action": "barre", "barre": {
                "id": "", "nom": "Référence hors grille", "genre": "poutre", "niveau": niveau,
                "debut": [-3222, 1589, 2800], "fin": fin, "largeur": 200, "hauteur": 400,
                "arase": "superieure"}})
            barres = commande("projet_natif")["projet"]["barres"]
            commande("camera", "cadrer")
            commande("selection_native", [barres[-1]["id"]])
            x, y = commande("axe_selection")["fin_ecran"]
            commande("outil", "poutre")
            saisie([{"action": "clic", "x": x + 2, "y": y + 2}])
            saisie([{"action": "clic", "x": x, "y": y + 70}])
            saisie([{"action": "touche", "touche": "Enter"}])
            nouvelles = commande("projet_natif")["projet"]["barres"]
            assert len(nouvelles) == len(barres) + 1 and nouvelles[-1]["debut"] == fin, nouvelles
            print("OK poignées des poutres")
        finally:
            p.terminate(); p.wait(10)
            log.close()


def poutre_id(commande):
    return next(b["id"] for b in commande("projet_natif")["projet"]["barres"] if b["genre"] == "poutre")


if __name__ == "__main__":
    a = argparse.ArgumentParser()
    a.add_argument("--exe", type=Path, default=Path("target/release/bimedit"))
    a.add_argument("--capture")
    x = a.parse_args()
    run(x.exe, x.capture)
