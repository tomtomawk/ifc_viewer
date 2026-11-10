"""Déplacer, Copier, Tourner et Aligner joués à la souris (socle commun des
transformations, docs/MODIFICATION.md), dans une instance isolée.

python tools/test-transformer.py --exe target/debug/bimedit.exe
"""
import argparse
import json
import math
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time


def run(exe):
    with tempfile.TemporaryDirectory(prefix="bimedit-transformer-") as dossier:
        root = Path(dossier)
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0)); port = s.getsockname()[1]
        env = dict(os.environ, BIMEDIT_PORT=str(port), BIMEDIT_ARRIERE_PLAN=os.environ.get("BIMEDIT_ARRIERE_PLAN", "1"), APPDATA=str(root / "configuration"), BIMEDIT_CACHE_COLLABORATION=str(root / "cache"))
        log = (root / "instance.log").open("w", encoding="utf-8")
        p = None
        nombre = 0

        def commande(nom, argument=None, ok=True):
            nonlocal nombre
            if argument is not None and not isinstance(argument, str):
                argument = json.dumps(argument, ensure_ascii=False, separators=(",", ":"))
            ligne = nom + (" " + argument if argument is not None else "")
            with socket.create_connection(("127.0.0.1", port), timeout=60) as s:
                s.sendall((ligne + "\n").encode()); r = json.loads(s.makefile("r", encoding="utf-8").readline())
            assert r.get("ok") == ok, (nom, r)
            nombre += 1
            return r

        def attendre(f, timeout=15):
            fin = time.monotonic() + timeout
            while time.monotonic() < fin:
                r = f()
                if r: return r
                time.sleep(.05)
            raise AssertionError("Délai dépassé")

        def saisie(entrees):
            commande("saisie", entrees)
            attendre(lambda: commande("etat")["saisies_en_attente"] == 0)

        def clic(q, maj=False):
            # survol puis clic : la cible est surlignée à l'image précédant le clic
            saisie([{"action": "pointer", "x": q[0], "y": q[1]}])
            saisie([{"action": "clic", "x": q[0], "y": q[1], "maj": maj}])

        def mur(id):
            return next(m for m in commande("projet_natif")["projet"]["murs"] if m["id"] == id)

        def axe(id):
            commande("selection_native", [id])
            return commande("axe_selection")

        def transfo():
            return commande("etat")["transformation"]

        def milieu(a):
            return [(a["debut_ecran"][i] + a["fin_ecran"][i]) / 2 for i in range(2)]

        proche = lambda u, v, tol=1.0: all(abs(x - y) <= tol for x, y in zip(u, v))

        try:
            p = subprocess.Popen([str(exe.resolve())], env=env, stdout=log, stderr=log)
            def pret():
                try: return commande("etat")["ok"]
                except OSError: return False
            attendre(pret, 45)
            commande("nouveau", {"chemin": str(root / "Transformer.modele.json")})
            a = commande("mur", "0 0 4000 0 2800 200")["mur"]
            b = commande("mur", "6000 1500 9000 1500 2800 200")["mur"]
            commande("camera", "cadrer")
            commande("outil", "aucun")

            # 1. Déplacer : base cliquée, direction au curseur, distance tapée (cm), Entrée
            ax = axe(a)
            commande("outil", "deplacer")
            assert "point de base" in transfo()["consigne"]
            clic(ax["debut_ecran"])
            assert len(transfo()["points"]) == 1 and "destination" in transfo()["consigne"]
            saisie([{"action": "pointer", "x": ax["fin_ecran"][0], "y": ax["fin_ecran"][1]}])
            saisie([{"action": "texte", "texte": "100"}])
            assert transfo()["saisie"] == "100"
            saisie([{"action": "touche", "touche": "Enter"}])
            apres = axe(a)
            assert proche(apres["debut"], [ax["debut"][0] + 1000, ax["debut"][1]]), apres
            commande("annuler")
            assert proche(axe(a)["debut"], ax["debut"])

            # 2. Copier : deux clics, l'original reste, une copie décalée
            commande("selection_native", [a]); commande("outil", "copier")
            clic(ax["debut_ecran"]); clic(ax["fin_ecran"])
            murs = commande("projet_natif")["projet"]["murs"]
            assert len(murs) == 3, len(murs)
            assert proche(axe(a)["debut"], ax["debut"])
            commande("annuler")
            assert len(commande("projet_natif")["projet"]["murs"]) == 2

            # 3. Tourner à la souris : centre = milieu de la sélection, direction, puis 90°
            commande("outil", "aucun"); commande("selection_native", [a]); commande("outil", "tourner")
            t = transfo()
            assert abs(t["centre"][0] - 2000) < 1 and abs(t["centre"][1]) <= 200, t  # axe médian du mur
            c = t["centre_ecran"]; f = ax["fin_ecran"]
            clic(f)
            assert len(transfo()["points"]) == 1
            v = [f[0] - c[0], f[1] - c[1]]
            clic([c[0] - v[1], c[1] + v[0]], maj=True)
            r = axe(a)
            assert abs(r["debut"][0] - r["fin"][0]) < 1.0 and abs(abs(r["debut"][1] - r["fin"][1]) - 4000) < 1.0, r
            commande("annuler")
            assert proche(axe(a)["debut"], ax["debut"])

            # 4. Tourner au clavier : angle tapé, Entrée
            commande("outil", "aucun"); commande("selection_native", [a]); commande("outil", "tourner")
            saisie([{"action": "pointer", "x": f[0], "y": f[1]}])
            saisie([{"action": "texte", "texte": "90"}, {"action": "touche", "touche": "Enter"}])
            r = axe(a)
            assert abs(r["debut"][0] - r["fin"][0]) < 1.0, r
            commande("annuler")

            # 5. Échap revient d'une étape, puis quitte l'outil
            commande("outil", "aucun"); commande("selection_native", [a]); commande("outil", "tourner")
            clic(f)
            assert len(transfo()["points"]) == 1
            saisie([{"action": "touche", "touche": "Escape"}])
            assert transfo()["points"] == [] and commande("etat")["outil"] == "Tourner"
            saisie([{"action": "touche", "touche": "Escape"}])
            assert commande("etat")["outil"] is None

            # 6. Tourner : clic sur le centre pour le déplacer
            commande("selection_native", [a]); commande("outil", "tourner")
            clic(transfo()["centre_ecran"])
            assert transfo()["placer_centre"]
            clic(ax["debut_ecran"])
            assert proche(transfo()["centre"], [0, 0], 60), transfo()
            saisie([{"action": "touche", "touche": "Escape"}, {"action": "touche", "touche": "Escape"}])

            # 7. Aligner : référence cliquée, puis élément cliqué
            commande("outil", "aucun"); commande("selection_native", []); commande("outil", "aligner")
            assert "référence" in transfo()["consigne"]
            clic(milieu(axe(a)))
            commande("outil", "aligner") if commande("etat")["outil"] != "Aligner" else None
            attendre(lambda: transfo() and transfo()["reference"] == a)
            bx = axe(b); commande("selection_native", [])
            clic(milieu(bx))
            r = axe(b)
            assert abs(r["debut"][1]) < 1.0 and abs(r["fin"][1]) < 1.0, r
            commande("annuler")
            assert proche(axe(b)["debut"], bx["debut"])
            nb = lambda: len(commande("projet_natif")["projet"]["murs"])

            # 8. Symétrie : axe = deux extrémités du mur B (y = 1500), copie par défaut
            commande("outil", "aucun"); commande("selection_native", [a]); commande("outil", "symetrie")
            assert transfo()["copie"] is True
            clic(bx["debut_ecran"]); clic(bx["fin_ecran"])
            assert nb() == 3
            nouveau = next(m for m in commande("projet_natif")["projet"]["murs"] if m["id"] not in (a, b))
            n = axe(nouveau["id"])
            assert all(2600 < y < 3400 for y in (n["debut"][1], n["fin"][1])), n  # image de y≈0 par l'axe y≈1500
            assert abs(abs(n["debut"][0] - n["fin"][0]) - 4000) < 1.0, n

            commande("annuler"); assert nb() == 2

            # 9. Réseau linéaire de 3 : base puis 2ᵉ élément (pas = longueur du mur)
            commande("outil", "aucun"); commande("selection_native", [a]); commande("outil", "reseau")
            commande("transformation", {"nombre": 3, "polaire": False})
            assert transfo()["nombre"] == 3
            clic(ax["debut_ecran"]); clic(ax["fin_ecran"])
            assert nb() == 4, nb()
            commande("annuler"); assert nb() == 2

            # 10. Échelle refusée sur un mur (épaisseur de type), projet inchangé
            avant = commande("projet_natif")["empreinte"]
            commande("outil", "aucun"); commande("selection_native", [a]); commande("outil", "echelle")
            clic(ax["debut_ecran"]); clic(milieu(ax)); clic(ax["fin_ecran"])
            assert commande("projet_natif")["empreinte"] == avant
            commande("outil", "aucun")

            # 11. Décaler : clic sur le mur, côté au curseur (vers B), distance tapée 50 cm
            commande("selection_native", []); commande("outil", "decaler")
            clic(milieu(ax))
            assert transfo()["element"] == a, transfo()
            saisie([{"action": "pointer", "x": bx["debut_ecran"][0], "y": bx["debut_ecran"][1]}])
            saisie([{"action": "texte", "texte": "50"}, {"action": "touche", "touche": "Enter"}])
            assert nb() == 3, nb()
            assert proche(axe(a)["debut"], ax["debut"]), "l'original reste (Copier coché)"
            commande("annuler"); assert nb() == 2

            # 12. Scinder : clic au milieu du mur A → deux murs
            commande("outil", "aucun"); commande("selection_native", []); commande("outil", "scinder")
            clic(milieu(ax))
            assert nb() == 3, nb()
            r = axe(a)
            assert abs(r["fin"][0] - 2000) < 60, r
            commande("annuler"); assert nb() == 2

            commande("quitter", "abandonner"); p.wait(timeout=15)
            print(f"Transformations : {nombre} requêtes validées ; déplacer, copier, tourner, symétrie, réseau, échelle, aligner, décaler, scinder à la souris, valeurs tapées et Échap OK")
        except Exception:
            log.flush(); print((root / "instance.log").read_text(encoding="utf-8")[-2500:]); raise
        finally:
            if p is not None and p.poll() is None: p.terminate(); p.wait(timeout=10)
            log.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path, default=Path("target/debug/bimedit.exe"))
    run(parser.parse_args().exe)
