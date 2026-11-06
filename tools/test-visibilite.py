"""Masquer temporaire, Masquer dans la vue et Afficher élément par élément (clic sur
l'élément magenta), dans une instance isolée.

python tools/test-visibilite.py --exe target/release/bimedit.exe
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
    with tempfile.TemporaryDirectory(prefix="bimedit-visibilite-") as dossier:
        root = Path(dossier)
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0)); port = s.getsockname()[1]
        env = dict(os.environ, BIMEDIT_PORT=str(port), APPDATA=str(root / "configuration"), BIMEDIT_CACHE_COLLABORATION=str(root / "cache"))
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

        def clic(q):
            saisie([{"action": "pointer", "x": q[0], "y": q[1]}])
            saisie([{"action": "clic", "x": q[0], "y": q[1]}])

        def milieu(id):
            commande("selection_native", [id])
            a = commande("axe_selection")
            return [(a["debut_ecran"][i] + a["fin_ecran"][i]) / 2 for i in range(2)]

        etat = lambda: commande("etat")
        vue = lambda: next(v for v in commande("projet_natif")["projet"]["vues"] if v["genre"] == "plan" or v.get("genre", {}) == "plan")

        try:
            p = subprocess.Popen([str(exe.resolve())], env=env, stdout=log, stderr=log)
            def pret():
                try: return commande("etat")["ok"]
                except OSError: return False
            attendre(pret, 45)
            commande("nouveau", {"chemin": str(root / "Visibilite.modele.json")})
            a = commande("mur", "0 0 4000 0 2800 200")["mur"]
            b = commande("mur", "0 3000 4000 3000 2800 200")["mur"]
            commande("camera", "cadrer")
            ma, mb = milieu(a), milieu(b)

            # 1. Masquer temporaire : rien d'enregistré dans la vue
            commande("outil", "aucun"); commande("selection_native", [a]); commande("outil", "masquer")
            projet = commande("projet_natif")["projet"]
            assert all(a not in v.get("elements_masques", []) for v in projet["vues"])

            # 1 bis. Cadre temporaire : la croix en haut à droite réaffiche tout
            assert etat()["visibilite_temporaire"] is True
            croix = attendre(lambda: etat()["croix_temporaire"])
            clic(croix)
            assert attendre(lambda: etat()["visibilite_temporaire"] is False)
            assert etat()["croix_temporaire"] is None or not etat()["visibilite_temporaire"]
            # Isoler, puis la croix
            commande("outil", "aucun"); commande("selection_native", [b]); commande("outil", "isoler")
            assert etat()["visibilite_temporaire"] is True
            clic(attendre(lambda: etat()["croix_temporaire"]))
            assert attendre(lambda: etat()["visibilite_temporaire"] is False)
            # de nouveau masqué temporairement, pour la suite
            commande("outil", "aucun"); commande("selection_native", [a]); commande("outil", "masquer")

            # 2. Afficher : l'élément masqué apparaît (magenta), clic = réaffiché
            commande("outil", "afficher")
            assert etat()["afficher_masques"] is True
            assert attendre(lambda: etat()["objets_masques"] > 0)
            ma = milieu(a); commande("selection_native", [])
            clic(ma)
            assert attendre(lambda: etat()["objets_masques"] == 0)
            saisie([{"action": "touche", "touche": "Escape"}])
            assert etat()["afficher_masques"] is False

            # 3. Masquer dans la vue : enregistré avec la vue
            commande("outil", "aucun"); commande("selection_native", [b]); commande("outil", "masquer_vue")
            masques = lambda: [x for v in commande("projet_natif")["projet"]["vues"] for x in v.get("elements_masques", [])]
            assert b in masques(), masques()

            # 4. Afficher puis clic : retiré des éléments masqués de la vue
            commande("outil", "afficher")
            assert attendre(lambda: etat()["objets_masques"] > 0)
            mb = milieu(b); commande("selection_native", [])  # position relue après la modification
            clic(mb)
            assert attendre(lambda: b not in masques())
            commande("outil", "afficher")  # bascule : sortie du mode
            assert etat()["afficher_masques"] is False

            commande("quitter", "abandonner"); p.wait(timeout=15)
            print(f"Visibilité : {nombre} requêtes validées ; masquer temporaire, croix de sortie, isoler, masquer dans la vue, afficher au clic OK")
        except Exception:
            log.flush(); print((root / "instance.log").read_text(encoding="utf-8")[-2500:]); raise
        finally:
            if p is not None and p.poll() is None: p.terminate(); p.wait(timeout=10)
            log.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path, default=Path("target/release/bimedit.exe"))
    run(parser.parse_args().exe)
