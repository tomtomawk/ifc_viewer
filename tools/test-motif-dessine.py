"""Motif de hachures dessiné dans sa vue de dessin : nœuds de la grille, ruban Modifier,
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
                # polyligne : points ramenés aux nœuds de la grille (8 divisions, 50 mm)
                commande("outil", "polyligne")
                points = [commande("ecran", f"{x} {y}")["ecran"] for x, y in ((-195, -190), (8, 3), (190, -195))]
                for q in points:
                    clic(*q)
                saisie([{"action": "touche", "touche": "Enter"}, {"action": "touche", "touche": "Escape"}, {"action": "touche", "touche": "Escape"}])
                trace = traits(vue)
                assert trace[0]["points"] == [[-200.0, -200.0], [0.0, 0.0], [200.0, -200.0]], trace
                # déplacer par le ruban Modifier : vecteur d'un nœud à l'autre
                a = commande("ecran", "-100 -100")["ecran"]
                clic(*a)
                assert commande("etat")["selection_native"] == [trace[0]["id"]]
                commande("outil", "deplacer")
                clic(*commande("ecran", "0 0")["ecran"])
                clic(*commande("ecran", "2 52")["ecran"])
                assert traits(vue)[0]["points"] == [[-200.0, -150.0], [0.0, 50.0], [200.0, -150.0]], traits(vue)
                saisie([{"action": "touche", "touche": "Escape"}, {"action": "touche", "touche": "Escape"}])
                commande("terminer_motif")
                projet = commande("projet_natif")["projet"]
                assert not any(v["id"] == vue for v in projet["vues"])
                motif = [m for m in projet["motifs"] if m["nom"] == nom][0]
                assert sorted(map(sorted, motif["tuile"]["segments"])) == [[[0, 1], [4, 5]], [[4, 5], [8, 1]]], motif["tuile"]
                assert len(motif["lignes"]) == 2
                # rouvrir puis annuler : rien ne change
                commande("editeur_hachure", nom)
                assert len(traits([v["id"] for v in commande("projet_natif")["projet"]["vues"] if v["genre"] == "dessin"][-1])) == 2
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
