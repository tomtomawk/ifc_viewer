"""Motif de hachures dessiné dans sa vue de dessin : accrochage au cadre, trait de bord, ruban Modifier,
Terminer et Annuler."""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time


def run(exe):
    with tempfile.TemporaryDirectory(prefix="bimedit-motif-") as dossier:
        root = Path(dossier)
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0))
            port = s.getsockname()[1]
        env = dict(os.environ, BIMEDIT_PORT=str(port), BIMEDIT_ARRIERE_PLAN="1", APPDATA=str(root / "configuration"))
        with (root / "instance.log").open("w", encoding="utf-8") as log:
            processus = subprocess.Popen([str(exe.resolve())], env=env, stdout=log, stderr=log)

            def commande(nom, argument=None):
                ligne = nom if argument is None else nom + " " + (argument if isinstance(argument, str) else json.dumps(argument, ensure_ascii=False))
                with socket.create_connection(("127.0.0.1", port), timeout=30) as connexion:
                    connexion.sendall((ligne + "\n").encode("utf-8"))
                    resultat = json.loads(connexion.makefile(encoding="utf-8").readline())
                assert resultat.get("ok"), (nom, resultat)
                return resultat

            def saisie(evenements):
                commande("saisie", evenements)
                limite = time.monotonic() + 15
                while commande("etat")["saisies_en_attente"]:
                    if time.monotonic() > limite:
                        raise TimeoutError("Saisie graphique non terminée")
                    time.sleep(0.05)
                time.sleep(0.15)

            def clic(x, y):
                # survol d'abord : l'élément visé est celui sous le pointeur
                saisie([{"action": "pointer", "x": x, "y": y}])
                saisie([{"action": "clic", "x": x, "y": y}])

            def traits(vue):
                return [a for a in commande("projet_natif")["projet"].get("annotations", []) if a["vue"] == vue]

            try:
                limite = time.monotonic() + 45
                while True:
                    try:
                        commande("etat")
                        break
                    except OSError:
                        if processus.poll() is not None or time.monotonic() > limite:
                            raise RuntimeError("Instance de test indisponible")
                        time.sleep(0.2)
                commande("nouveau", {"chemin": str(root / "Projet.modele.json")})
                nom = commande("editeur_hachure")["nom"]
                projet = commande("projet_natif")["projet"]
                vue = [v["id"] for v in projet["vues"] if v["genre"] == "dessin"][-1]
                assert commande("etat")["vue"]["nom"] == f"Motif — {nom}"
                # polyligne accrochée au cadre (coins) et à l'origine (centre du carré)
                def polyligne(*pts):
                    commande("outil", "polyligne")
                    for x, y in pts:
                        clic(*commande("ecran", f"{x} {y}")["ecran"])
                    saisie([{"action": "touche", "touche": "Enter"}, {"action": "touche", "touche": "Escape"}, {"action": "touche", "touche": "Escape"}])
                polyligne((-195, -192), (6, 4), (193, -195))
                trace = traits(vue)
                assert trace[0]["points"] == [[-200.0, -200.0], [0.0, 0.0], [200.0, -200.0]], trace
                # déplacer par le ruban Modifier : de l'origine au milieu du bord haut
                clic(*commande("ecran", "-100 -100")["ecran"])
                assert commande("etat")["selection_native"] == [trace[0]["id"]]
                commande("outil", "deplacer")
                clic(*commande("ecran", "3 2")["ecran"])
                clic(*commande("ecran", "4 195")["ecran"])
                assert traits(vue)[0]["points"] == [[-200.0, 0.0], [0.0, 200.0], [200.0, 0.0]], traits(vue)
                saisie([{"action": "touche", "touche": "Escape"}, {"action": "touche", "touche": "Escape"}])
                # un trait sur le bord bas et un sur le bord haut : le même trait répété
                polyligne((-196, -197), (197, -196))
                polyligne((-196, 197), (197, 196))
                assert len(traits(vue)) == 3
                commande("terminer_motif")
                projet = commande("projet_natif")["projet"]
                assert not any(v["id"] == vue for v in projet["vues"])
                motif = [m for m in projet["motifs"] if m["nom"] == nom][0]
                segments = sorted(map(sorted, motif["tuile"]["segments"]))
                assert segments == [[[0, 0], [240, 0]], [[0, 120], [120, 240]], [[120, 0], [240, -120]]], motif["tuile"]
                assert len(motif["lignes"]) == 3
                # le motif est rangé dans l'arborescence : propriétés par le pilote
                commande("motifs_hachures", nom)
                # rouvrir puis annuler : rien ne change
                commande("editeur_hachure", nom)
                assert len(traits([v["id"] for v in commande("projet_natif")["projet"]["vues"] if v["genre"] == "dessin"][-1])) == 3
                commande("annuler_motif")
                apres = commande("projet_natif")["projet"]
                assert not any(v["genre"] == "dessin" for v in apres["vues"]) and apres["motifs"] == projet["motifs"]
                print("Motif dessiné en vue de dessin : OK")
            finally:
                processus.terminate()
                processus.wait(timeout=15)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path, default=Path("target/debug/bimedit.exe"))
    run(parser.parse_args().exe)
