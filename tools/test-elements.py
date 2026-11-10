"""Démolir, Reproduire les propriétés, Changer d'hôte, Inverser, menu du clic droit,
répétition de la dernière commande et poignée du milieu, joués à la souris dans une
instance isolée.

python tools/test-elements.py --exe target/release/bimedit.exe
"""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time


def run(exe):
    with tempfile.TemporaryDirectory(prefix="bimedit-elements-") as dossier:
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
            saisie([{"action": "pointer", "x": q[0], "y": q[1]}])
            saisie([{"action": "clic", "x": q[0], "y": q[1], "maj": maj}])

        def axe(id):
            commande("selection_native", [id])
            return commande("axe_selection")

        def milieu(id):
            a = axe(id)
            return [(a["debut_ecran"][i] + a["fin_ecran"][i]) / 2 for i in range(2)]

        projet = lambda: commande("projet_natif")["projet"]
        mur = lambda id: next(m for m in projet()["murs"] if m["id"] == id)
        etat = lambda: commande("etat")

        try:
            p = subprocess.Popen([str(exe.resolve())], env=env, stdout=log, stderr=log)
            def pret():
                try: return commande("etat")["ok"]
                except OSError: return False
            attendre(pret, 45)
            commande("nouveau", {"chemin": str(root / "Elements.modele.json")})
            a = commande("mur", "0 0 4000 0 2800 200")["mur"]
            b = commande("mur", "0 3000 4000 3000 2800 200")["mur"]
            commande("camera", "cadrer")

            # 1. Démolir au clic, puis annuler la démolition au second clic
            ma = milieu(a); commande("selection_native", [])
            commande("outil", "demolir")
            assert "démolir" in etat()["transformation"]["consigne"]
            clic(ma)
            assert projet().get("demolitions", {}).get(a), projet().get("demolitions")
            clic(ma)
            assert not projet().get("demolitions", {}).get(a)
            commande("outil", "aucun")

            # 2. Pinceau : la hauteur de base de A passe sur B
            i = next(k for k, m in enumerate(projet()["murs"]) if m["id"] == a)
            r = commande("projet_natif")
            commande("modifier_projet", {"empreinte": r["empreinte"], "operations": [{"op": "replace", "path": f"/murs/{i}/decalage_bas", "value": 300}]})
            ma, mb = milieu(a), milieu(b); commande("selection_native", [])
            commande("outil", "reproduire")
            clic(ma)
            assert etat()["transformation"]["element"] == a
            clic(mb)
            assert mur(b)["decalage_bas"] == 300, mur(b)
            commande("outil", "aucun")

            # 3. Inverser au clic : le sens du mur B est retourné
            avant = axe(b); mb = milieu(b); commande("selection_native", [])
            commande("outil", "inverser")
            clic(mb)
            apres = axe(b)
            assert abs(apres["debut"][0] - avant["fin"][0]) < 1 and abs(apres["fin"][0] - avant["debut"][0]) < 1, (avant, apres)
            commande("outil", "aucun")

            # 4. Répétition : Entrée relance la dernière commande (Inverser)
            commande("selection_native", [])
            saisie([{"action": "touche", "touche": "Enter"}])
            assert etat()["outil"] == "Inverser", etat()["outil"]
            commande("outil", "aucun")
            commande("menu", "repeter")
            assert etat()["outil"] == "Inverser"
            commande("outil", "aucun")

            # 5. Menu : identiques dans le projet (même type), filtre par catégorie
            commande("selection_native", [a]); commande("menu", "identiques_projet")
            assert sorted(etat()["selection_native"]) == sorted([a, b]), etat()["selection_native"]
            commande("menu", "garder murs")
            assert len(etat()["selection_native"]) == 2
            commande("selection_native", [a]); commande("menu", "verrouiller")
            assert projet().get("verrous", {}).get(a)
            commande("menu", "deverrouiller")
            assert not projet().get("verrous", {}).get(a)

            # 6. Changer d'hôte : ouverture de A placée sur B par deux clics
            commande("editer", {"action": "ouverture", "mur": a, "genre": "porte", "type_ouverture": None, "abscisse": 2000, "largeur": 900, "hauteur": 2100, "allege": 0})
            o = projet()["ouvertures"][0]["id"]
            ma, mb = milieu(a), milieu(b); commande("selection_native", [])
            commande("outil", "changer_hote")
            clic(ma)
            assert etat()["transformation"]["element"] == o, etat()["transformation"]
            clic(mb)
            assert projet()["ouvertures"][0]["mur"] == b, projet()["ouvertures"][0]
            commande("outil", "aucun")

            # 7. Poignée du milieu : glisser déplace le mur, une étape d'annulation
            ax = axe(a); m = milieu(a)
            saisie([{"action": "pointer", "x": m[0], "y": m[1]}])
            saisie([{"action": "bouton", "x": m[0], "y": m[1], "bouton": "gauche", "appuye": True}, {"action": "pointer", "x": m[0] + 5, "y": m[1] - 40}, {"action": "pointer", "x": m[0] + 10, "y": m[1] - 80}])
            saisie([{"action": "bouton", "x": m[0] + 10, "y": m[1] - 80, "bouton": "gauche", "appuye": False}])
            apres = axe(a)
            assert abs(apres["debut"][1] - ax["debut"][1]) > 50, (ax, apres)
            commande("annuler")
            assert abs(axe(a)["debut"][1] - ax["debut"][1]) < 1

            commande("quitter", "abandonner"); p.wait(timeout=15)
            print(f"Éléments : {nombre} requêtes validées ; démolir, pinceau, inverser, répéter, menu, changer d'hôte, poignée du milieu OK")
        except Exception:
            log.flush(); print((root / "instance.log").read_text(encoding="utf-8")[-2500:]); raise
        finally:
            if p is not None and p.poll() is None: p.terminate(); p.wait(timeout=10)
            log.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path, default=Path("target/release/bimedit.exe"))
    run(parser.parse_args().exe)
