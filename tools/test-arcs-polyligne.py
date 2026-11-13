"""Polyligne à segments d'arc tracée par le pilote : Alt+clic puis point de passage, Alt+clic
puis rayon tapé, sur une instance et un dossier temporaires isolés.

python tools/test-arcs-polyligne.py --exe target/debug/bimedit.exe [--capture dossier]
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


def run(exe, capture):
    with tempfile.TemporaryDirectory(prefix="bimedit-arcs-", ignore_cleanup_errors=True) as dossier:
        root = Path(dossier)
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0))
            port = s.getsockname()[1]
        env = dict(os.environ, BIMEDIT_PORT=str(port), BIMEDIT_ARRIERE_PLAN=os.environ.get("BIMEDIT_ARRIERE_PLAN", "1"), BIMEDIT_CACHE_COLLABORATION=str(root / "cache"))
        # Ne lit ni n'écrit les préférences de l'utilisateur.
        env["APPDATA"] = str(root / "configuration")
        log = (root / "instance.log").open("w", encoding="utf-8")
        p = subprocess.Popen([str(exe.resolve())], env=env, stdout=log, stderr=log)

        def commande(nom, argument=None, ok=True):
            if argument is not None and not isinstance(argument, str):
                argument = json.dumps(argument, ensure_ascii=False, separators=(",", ":"))
            ligne = nom + (" " + argument if argument is not None else "")
            with socket.create_connection(("127.0.0.1", port), timeout=90) as s:
                s.sendall((ligne + "\n").encode())
                resultat = json.loads(s.makefile("r", encoding="utf-8").readline())
            assert resultat.get("ok") == ok, (nom, resultat)
            return resultat

        def attendre(f, timeout=45):
            fin = time.monotonic() + timeout
            while time.monotonic() < fin:
                v = f()
                if v:
                    return v
                time.sleep(0.1)
            raise AssertionError("Délai dépassé")

        def saisie(entrees):
            commande("saisie", entrees)
            attendre(lambda: commande("etat")["saisies_en_attente"] == 0)

        def polylignes():
            annotations = commande("projet_natif")["projet"]["annotations"]
            return [a for a in annotations if a["forme"] == "polyligne"]

        def rayon(a, b, k):
            return math.dist(a, b) * (1 + k * k) / (4 * abs(k))

        try:
            def pret():
                try:
                    return commande("etat")["ok"]
                except OSError:
                    assert p.poll() is None, (root / "instance.log").read_text(encoding="utf-8")
                    return False
            attendre(pret)
            commande("nouveau", {"chemin": str(root / "Arcs.modele.json"), "nom": "Arcs"})
            niveau = commande("projet_natif")["projet"]["niveaux"][0]["id"]
            commande("vue_plan", niveau)
            # l'outil Arc n'existe plus : l'arc est un segment de la polyligne
            commande("outil", "arc", ok=False)
            commande("outil", "polyligne")
            x, y, w, h = commande("etat")["rectangle_vue"]
            assert w > 300 and h > 300
            def ecran(u, v):
                return {"x": x + w * u, "y": y + h * v}

            # 1. deux points, Alt+clic sur la fin de l'arc, clic du point de passage, un
            #    point de plus, Entrée
            saisie([{"action": "pointer", **ecran(.25, .7)}, {"action": "clic", **ecran(.25, .7)}])
            saisie([{"action": "pointer", **ecran(.4, .7)}, {"action": "clic", **ecran(.4, .7)}])
            saisie([{"action": "pointer", **ecran(.6, .7)}, {"action": "clic", **ecran(.6, .7), "alt": True}])
            if capture:
                saisie([{"action": "pointer", **ecran(.5, .5)}])
                commande("capture", f"fenetre {capture / 'arc-en-attente.png'}")
            saisie([{"action": "pointer", **ecran(.5, .55)}, {"action": "clic", **ecran(.5, .55)}])
            saisie([{"action": "pointer", **ecran(.75, .7)}, {"action": "clic", **ecran(.75, .7)}])
            saisie([{"action": "touche", "touche": "Enter"}])
            formes = polylignes()
            assert len(formes) == 1, formes
            f = formes[0]
            assert len(f["points"]) == 4 and len(f["courbures"]) == 3, f
            assert f["courbures"][0] == 0 and f["courbures"][2] == 0 and abs(f["courbures"][1]) > 0.05, f

            # 2. Alt+clic, puis rayon tapé (au moins la demi-corde, dans l'unité d'affichage)
            commande("outil", "polyligne")
            saisie([{"action": "pointer", **ecran(.3, .35)}, {"action": "clic", **ecran(.3, .35)}])
            saisie([{"action": "pointer", **ecran(.6, .35)}, {"action": "clic", **ecran(.6, .35), "alt": True}])
            saisie([{"action": "pointer", **ecran(.45, .2)}])
            corde = math.dist(f["points"][1], f["points"][2]) * 1.5
            valeur = round(corde)
            saisie([{"action": "texte", "texte": str(valeur)}])
            if capture:
                commande("capture", f"fenetre {capture / 'arc-rayon-tape.png'}")
            saisie([{"action": "touche", "touche": "Enter"}])
            saisie([{"action": "touche", "touche": "Enter"}])
            formes = polylignes()
            assert len(formes) == 2, formes
            g = formes[1]
            assert len(g["points"]) == 2 and len(g["courbures"]) == 1 and g["courbures"][0] != 0, g
            r = rayon(g["points"][0], g["points"][1], g["courbures"][0])
            assert any(abs(r / valeur - facteur) < 1e-3 * facteur for facteur in (1, 10, 1000)), (r, valeur)

            # 3. relecture : les courbures sont enregistrées et relues à l'identique
            commande("enregistrer")
            relu = json.loads((root / "Arcs.modele.json").read_text(encoding="utf-8"))
            assert [a for a in relu["annotations"] if a["forme"] == "polyligne"] == formes
            # 4. Modifier tracé puis valider : les arcs restent sur leurs segments
            commande("outil", "aucun")
            saisie([{"action": "touche", "touche": "Escape"}])
            commande("modifier_trace", f["id"])
            if capture:
                saisie([{"action": "pointer", **ecran(.5, .5)}])
                commande("capture", f"fenetre {capture / 'modifier-trace.png'}")
            saisie([{"action": "touche", "touche": "Enter"}])
            assert polylignes() == formes, polylignes()
            if capture:
                commande("outil", "aucun")
                saisie([{"action": "touche", "touche": "Escape"}])
                commande("capture", f"fenetre {capture / 'polylignes-a-arcs.png'}")
            print("ok : polylignes à arcs", [f["courbures"], g["courbures"]], "rayon", round(r))
        finally:
            try:
                commande("quitter")
            except Exception:
                pass
            try:
                p.wait(timeout=10)
            except subprocess.TimeoutExpired:
                p.kill()
                p.wait()
            log.close()


if __name__ == "__main__":
    a = argparse.ArgumentParser()
    a.add_argument("--exe", type=Path, default=Path("target/debug/bimedit.exe"))
    a.add_argument("--capture", type=Path)
    args = a.parse_args()
    if args.capture:
        args.capture.mkdir(parents=True, exist_ok=True)
        args.capture = args.capture.resolve()
    run(args.exe, args.capture)
