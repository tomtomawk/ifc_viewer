"""Accrochage du tracé sur les murs existants (dalle, mur) et trait de construction
perpendiculaire au dernier trait, dans une instance isolée.

python tools/test-accrochage.py --exe target/release/bimedit.exe
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
    with tempfile.TemporaryDirectory(prefix="bimedit-accrochage-") as dossier:
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

        def survol(q):
            # deux positions proches : le tracé lit la position courante à chaque image
            saisie([{"action": "pointer", "x": q[0] + 1, "y": q[1]}])
            saisie([{"action": "pointer", "x": q[0], "y": q[1]}])
            return commande("etat")["accrochage"]["point"]

        def ecrans(id):
            commande("selection_native", [id])
            a = commande("axe_selection")
            commande("selection_native", [])
            return a

        try:
            p = subprocess.Popen([str(exe.resolve())], env=env, stdout=log, stderr=log)
            def pret():
                try: return commande("etat")["ok"]
                except OSError: return False
            attendre(pret, 45)
            commande("nouveau", {"chemin": str(root / "Accrochage.modele.json")})
            diag = commande("mur", "0 0 3000 3000 2800 200")["mur"]
            horiz = commande("mur", "0 -2000 3000 -2000 2800 200")["mur"]
            commande("camera", "cadrer")
            a, b = ecrans(diag), ecrans(horiz)

            # 1. Accrochage aux extrémités et milieux d'un mur simple, dalle et mur
            for outil in ["dalle", "mur"]:
                commande("outil", outil)
                for q, attendu, nom in [(a["debut_ecran"], [0, 0], "Extrémité"), (a["fin_ecran"], [3000, 3000], "Extrémité"), (b["fin_ecran"], [3000, -2000], "Extrémité")]:
                    pt = survol([q[0] + 4, q[1] + 3])
                    # extrémité d'axe ou coin du contour du mur (à moins d'une épaisseur)
                    assert pt["accroche"] == nom and ((pt["position"][0] - attendu[0]) ** 2 + (pt["position"][1] - attendu[1]) ** 2) ** .5 <= 150, (outil, pt)
                m = [(b["debut_ecran"][i] + b["fin_ecran"][i]) / 2 for i in range(2)]
                pt = survol([m[0] + 4, m[1] + 3])
                assert pt["accroche"] == "Milieu" and abs(pt["position"][0] - 1500) < 1 and abs(pt["position"][1] + 2000) <= 150, (outil, pt)
                commande("outil", "aucun")

            # 2. Perpendiculaire au dernier trait : tracé (0,0) → (3000,3000), puis un point
            # proche de la perpendiculaire en (3000,3000), soit la droite x + y = 6000
            # repère écran du plan (vue de dessus) : affine, déduit de trois points connus
            (x0, y0), (x1, y1), (x2, y2) = a["debut_ecran"], a["fin_ecran"], b["debut_ecran"]
            def ecran(x, y):
                # base : (0,0), (3000,3000), (0,-2000)
                dxdx = (x1 - x0 - (x2 - x0) / -2000 * 3000) / 3000
                dxdy = (x2 - x0) / -2000
                dydx = (y1 - y0 - (y2 - y0) / -2000 * 3000) / 3000
                dydy = (y2 - y0) / -2000
                return [x0 + dxdx * x + dxdy * y, y0 + dydx * x + dydy * y]
            commande("outil", "dalle")
            saisie([{"action": "pointer", "x": x0, "y": y0}]); saisie([{"action": "clic", "x": x0, "y": y0}])
            saisie([{"action": "pointer", "x": x1, "y": y1}]); saisie([{"action": "clic", "x": x1, "y": y1}])
            cible = ecran(2000 + 25, 4000 - 5)  # 25 mm à côté de la perpendiculaire
            pt = survol(cible)
            trace = commande("etat")["accrochage"]["trace"]
            assert trace and len(trace) == 2, trace
            (ax, ay), (bx, by) = trace
            x, y = pt["position"]
            # (point − dernier point) · (dernier trait) = 0 : sur la perpendiculaire
            assert abs((x - bx) * (bx - ax) + (y - by) * (by - ay)) / ((bx - ax) ** 2 + (by - ay) ** 2) ** .5 < 1.0, ("perpendiculaire non accrochée", pt, trace)
            saisie([{"action": "touche", "touche": "Escape"}, {"action": "touche", "touche": "Escape"}])

            commande("quitter", "abandonner"); p.wait(timeout=15)
            print(f"Accrochage : {nombre} requêtes validées ; murs existants (dalle, mur) et perpendiculaire au dernier trait OK")
        except Exception:
            log.flush(); print((root / "instance.log").read_text(encoding="utf-8")[-2500:]); raise
        finally:
            if p is not None and p.poll() is None: p.terminate(); p.wait(timeout=10)
            log.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path, default=Path("target/release/bimedit.exe"))
    run(parser.parse_args().exe)
