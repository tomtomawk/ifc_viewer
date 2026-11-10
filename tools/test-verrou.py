"""Verrouiller / déverrouiller au ruban et au cadenas de la vue, dans une instance isolée.

python tools/test-verrou.py --exe target/debug/bimedit.exe
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
    with tempfile.TemporaryDirectory(prefix="bimedit-verrou-") as dossier:
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
            with socket.create_connection(("127.0.0.1", port), timeout=30) as s:
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

        def verrous():
            return commande("projet_natif")["projet"].get("verrous", {})

        try:
            p = subprocess.Popen([str(exe.resolve())], env=env, stdout=log, stderr=log)
            def pret():
                try: return commande("etat")["ok"]
                except OSError: return False
            attendre(pret, 45)
            commande("nouveau", {"chemin": str(root / "Verrou.modele.json")})
            a = commande("mur", "0 0 4000 0 2800 200")["mur"]
            b = commande("mur", "0 3000 4000 3000 2800 200")["mur"]

            # 1. Verrouiller par le ruban
            commande("outil", "aucun"); commande("selection_native", [a])
            commande("outil", "verrouiller")
            assert verrous() == {a: True}, verrous()

            # 2. Cadenas visible dans la vue, sur l'élément
            cad = attendre(lambda: [c for c in commande("etat")["cadenas"] if c["id"] == a])
            vue = commande("etat")["rectangle_vue"]
            x, y = cad[0]["ecran"]
            assert vue[0] <= x <= vue[0] + vue[2] and vue[1] <= y <= vue[1] + vue[3], (cad, vue)

            # 3. Supprimer et déplacer refusés, sélection mixte : seul le libre bouge
            avant = commande("projet_natif")["empreinte"]
            commande("selection_native", [a]); commande("outil", "supprimer", ok=False)
            assert commande("projet_natif")["empreinte"] == avant
            assert any(m["id"] == a for m in commande("projet_natif")["projet"]["murs"])
            murs0 = {m["id"]: m for m in commande("projet_natif")["projet"]["murs"]}
            commande("editer", {"action": "deplacer", "ids": [a, b], "delta": [0, 500, 0]})
            murs = {m["id"]: m for m in commande("projet_natif")["projet"]["murs"]}
            assert murs[a] == murs0[a], "mur verrouillé déplacé"
            assert murs[b] != murs0[b], "mur libre non déplacé"
            commande("annuler")

            # 4. Clic graphique sur le cadenas : déverrouille, une étape d'annulation
            commande("outil", "aucun")
            # position relue après les modifications, stable sur deux images successives
            def position():
                p1 = [c["ecran"] for c in commande("etat")["cadenas"] if c["id"] == a]
                time.sleep(.1)
                p2 = [c["ecran"] for c in commande("etat")["cadenas"] if c["id"] == a]
                return p1 and p1 == p2 and p1[0]
            x, y = attendre(position)
            saisie([{"action": "clic", "x": x, "y": y}])
            attendre(lambda: verrous() == {})
            assert attendre(lambda: commande("etat")["cadenas"] == [])
            commande("annuler")
            assert verrous() == {a: True}

            # 5. Déverrouiller par le ruban, puis suppression permise
            commande("selection_native", [a]); commande("outil", "deverrouiller")
            assert verrous() == {}
            commande("selection_native", [a]); commande("outil", "supprimer")
            assert all(m["id"] != a for m in commande("projet_natif")["projet"]["murs"])

            commande("quitter", "abandonner"); p.wait(timeout=15)
            print(f"Verrou : {nombre} requêtes validées ; ruban, cadenas cliquable, refus et annulation OK")
        except Exception:
            log.flush(); print((root / "instance.log").read_text(encoding="utf-8")[-2500:]); raise
        finally:
            if p is not None and p.poll() is None: p.terminate(); p.wait(timeout=10)
            log.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path, default=Path("target/debug/bimedit.exe"))
    run(parser.parse_args().exe)
