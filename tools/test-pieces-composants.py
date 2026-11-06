"""Pièces et composants joués à la souris dans une instance isolée : pièce créée au clic
dans quatre murs (refus hors contour), étiquette déplacée, contour recalculé quand un
mur bouge ; composant placé avec rotation par Espace, transformé, copié, supprimé ;
export IFC (IfcSpace, IfcFurniture) et aplat de la pièce dans la capture du plan.

python tools/test-pieces-composants.py --exe target/verif/release/bimedit.exe
"""
import argparse
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time
import zlib


def pixels_png(chemin):
    """Décodeur PNG minimal (8 bits, RGB/RGBA, non entrelacé) : (largeur, hauteur, f(x, y))."""
    d = Path(chemin).read_bytes()
    i, idat, ihdr = 8, b"", None
    while i < len(d):
        n, t = struct.unpack(">I4s", d[i:i + 8])
        c = d[i + 8:i + 8 + n]
        if t == b"IHDR": ihdr = struct.unpack(">IIBBBBB", c)
        elif t == b"IDAT": idat += c
        i += 12 + n
    w, h, prof, couleur = ihdr[:4]
    assert prof == 8 and couleur in (2, 6), ihdr
    bpp = 3 if couleur == 2 else 4
    brut = zlib.decompress(idat)
    lignes, prec, k = [], bytearray(w * bpp), 0
    for _ in range(h):
        f = brut[k]; l = bytearray(brut[k + 1:k + 1 + w * bpp]); k += 1 + w * bpp
        for x in range(len(l)):
            a = l[x - bpp] if x >= bpp else 0; b = prec[x]; c = prec[x - bpp] if x >= bpp else 0
            if f == 1: l[x] = (l[x] + a) & 255
            elif f == 2: l[x] = (l[x] + b) & 255
            elif f == 3: l[x] = (l[x] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c; pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                l[x] = (l[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        lignes.append(l); prec = l
    return w, h, lambda x, y: tuple(lignes[int(y)][int(x) * bpp:int(x) * bpp + 3])


def run(exe, captures=None):
    with tempfile.TemporaryDirectory(prefix="bimedit-pieces-") as dossier:
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

        def ecran(x, y):
            return commande("ecran", f"{x} {y}")["ecran"]

        def clic(q):
            saisie([{"action": "pointer", "x": q[0], "y": q[1]}])
            saisie([{"action": "clic", "x": q[0], "y": q[1]}])

        projet = lambda: commande("projet_natif")["projet"]
        etat = lambda: commande("etat")

        try:
            p = subprocess.Popen([str(exe.resolve())], env=env, stdout=log, stderr=log)
            def pret():
                try: return commande("etat")["ok"]
                except OSError: return False
            attendre(pret, 45)
            commande("nouveau", {"chemin": str(root / "Pieces.modele.json")})
            pts = [(0, 0), (5000, 0), (5000, 4000), (0, 4000)]
            murs = [commande("mur", f"{a[0]} {a[1]} {b[0]} {b[1]} 2800 200")["mur"] for a, b in zip(pts, pts[1:] + pts[:1])]
            commande("camera", "cadrer")

            # 1. Pièce au clic : contour sur les faces intérieures, numéro 1
            commande("outil", "piece")
            assert etat()["outil"] == "Pièce", etat()["outil"]
            clic(ecran(2500, 2000))
            pieces = projet().get("pieces", [])
            assert len(pieces) == 1, pieces
            piece = pieces[0]
            assert piece["numero"] == "1" and len(piece["contour"]) == 4, piece
            xs = sorted(c[0] for c in piece["contour"]); ys = sorted(c[1] for c in piece["contour"])
            # faces intérieures des murs (selon l'implantation de la commande mur : 0 ou 100 mm)
            assert 0 <= xs[0] <= 100 and 4900 <= xs[-1] <= 5000 and 0 <= ys[0] <= 100 and 3900 <= ys[-1] <= 4000, piece
            x0 = xs[0]
            # seconde pièce : numéro 2 ; clic hors des murs : refus, rien de créé
            clic(ecran(3500, 1000))
            assert [x["numero"] for x in projet()["pieces"]] == ["1", "2"]
            commande("annuler")
            # clic dans l'épaisseur d'un mur (côté extérieur de la ligne tracée) : refus
            exterieur = -100 if x0 < 50 else 50
            clic(ecran(2500, exterieur))
            assert len(projet()["pieces"]) == 1
            assert etat().get("erreur_chargement"), etat()
            commande("erreur_effacer")
            commande("outil", "aucun")

            # 2. Aplat de la pièce dans le plan (capture de la vue)
            image = root / "plan.png"
            commande("capture", f"vue {image}")
            attendre(lambda: image.exists() and image.stat().st_size > 0)
            rv = etat()["rectangle_vue"]
            w, h, px = pixels_png(image)
            q = ecran(1200, 3000)
            couleur = px((q[0] - rv[0]) * w / rv[2], (q[1] - rv[1]) * h / rv[3])
            assert abs(couleur[0] - 226) <= 12 and abs(couleur[1] - 236) <= 12 and abs(couleur[2] - 248) <= 12, couleur

            # 3. Étiquette déplacée (Déplacer), annulée ; contour recalculé quand les murs bougent
            pid = piece["id"]
            commande("editer", {"action": "deplacer", "ids": [pid], "delta": [500, 300, 0]})
            assert projet()["pieces"][0]["point"] == [3000, 2300], projet()["pieces"][0]
            commande("annuler")
            assert projet()["pieces"][0]["point"] == [2500, 2000]
            commande("editer", {"action": "deplacer", "ids": murs, "delta": [1000, 0, 0]})
            xs = sorted(c[0] for c in projet()["pieces"][0]["contour"])
            assert abs(xs[0] - x0 - 1000) < 1e-6, projet()["pieces"][0]
            commande("annuler")
            # propriétés : nom et numéro modifiables
            commande("editer", {"action": "piece", "piece": dict(projet()["pieces"][0], nom="Séjour", numero="101")})
            x = projet()["pieces"][0]
            assert (x["nom"], x["numero"]) == ("Séjour", "101"), x

            # sélection au clic sur l'étiquette, suppression, annulation
            commande("selection_native", [])
            clic(ecran(2500, 2000))
            assert etat()["selection_native"] == [pid], etat()["selection_native"]
            commande("outil", "supprimer")
            assert not projet().get("pieces"), projet().get("pieces")
            commande("annuler")
            assert len(projet()["pieces"]) == 1
            commande("selection_native", [])

            # 4. Composant : type par défaut, Espace tourne de 90°, clic place
            commande("outil", "composant")
            r = commande("composant_outil", {"rotation": 0})
            assert r["types"], r
            q = ecran(1500, 1500)
            saisie([{"action": "pointer", "x": q[0], "y": q[1]}])
            saisie([{"action": "touche", "touche": "Space"}])
            assert commande("composant_outil")["composant"]["rotation"] == 90
            clic(q)
            composants = projet().get("composants", [])
            assert len(composants) == 1, composants
            c = composants[0]
            assert abs(c["rotation"] - 90) < 1e-9 and abs(c["position"][0] - 1500) < 60 and abs(c["position"][1] - 1500) < 60, c
            assert projet()["types_composants"][0]["id"] == c["type_composant"]
            commande("outil", "aucun")

            if captures:
                captures.mkdir(parents=True, exist_ok=True)
                commande("capture", f"vue {captures / 'plan-composant.png'}")

            # sélection du composant au clic dans le plan
            commande("selection_native", [])
            clic(ecran(c["position"][0], c["position"][1] - 500))
            assert etat()["selection_native"] == [c["id"]], etat()["selection_native"]
            commande("selection_native", [])

            # 5. Transformations : tourner, symétrie, copier, supprimer, annuler
            cid = c["id"]
            commande("editer", {"action": "tourner", "ids": [cid], "centre": c["position"], "angle": 90})
            assert abs(projet()["composants"][0]["rotation"] - 180) < 1e-6
            commande("editer", {"action": "copier", "ids": [cid], "delta": [2000, 0, 0]})
            assert len(projet()["composants"]) == 2
            commande("selection_native", [projet()["composants"][1]["id"]])
            commande("outil", "supprimer")
            assert len(projet()["composants"]) == 1
            commande("annuler")
            assert len(projet()["composants"]) == 2
            commande("annuler"); commande("annuler")
            assert abs(projet()["composants"][0]["rotation"] - 90) < 1e-6

            # 6. Export IFC : espace et mobilier
            ifc = root / "export.ifc"
            commande("exporter_ifc", str(ifc))
            texte = ifc.read_text(encoding="utf-8")
            assert "IFCSPACE(" in texte and "IFCFURNITURE(" in texte, texte[-2000:]

            commande("quitter", "abandonner"); p.wait(timeout=15)
            print(f"Pièces et composants : {nombre} requêtes validées ; pièce au clic, refus, aplat, recalcul, composant, Espace, transformations, IFC OK")
        except Exception:
            log.flush(); print((root / "instance.log").read_text(encoding="utf-8")[-2500:]); raise
        finally:
            if p is not None and p.poll() is None: p.terminate(); p.wait(timeout=10)
            log.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path, default=Path("target/verif/release/bimedit.exe"))
    parser.add_argument("--captures", type=Path, help="dossier où garder les captures du plan et de la 3D")
    a = parser.parse_args()
    run(a.exe, a.captures)
