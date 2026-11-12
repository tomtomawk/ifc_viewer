"""Chargement des catégories de famille dans une instance isolée.

python tools/test-chargement-familles.py --exe target/debug/bimedit.exe
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
    with tempfile.TemporaryDirectory(prefix="bimedit-chargement-familles-") as dossier:
        root = Path(dossier)
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0))
            port = s.getsockname()[1]
        env = dict(os.environ, BIMEDIT_PORT=str(port), BIMEDIT_ARRIERE_PLAN="1", APPDATA=str(root / "configuration"))
        with (root / "instance.log").open("w", encoding="utf-8") as log:
            processus = subprocess.Popen([str(exe.resolve())], env=env, stdout=log, stderr=log)

            def commande(nom, argument=None):
                texte = nom if argument is None else nom + " " + (argument if isinstance(argument, str) else json.dumps(argument, ensure_ascii=False))
                with socket.create_connection(("127.0.0.1", port), timeout=30) as connexion:
                    connexion.sendall((texte + "\n").encode("utf-8"))
                    resultat = json.loads(connexion.makefile(encoding="utf-8").readline())
                assert resultat.get("ok"), (nom, resultat)
                return resultat

            try:
                limite = time.monotonic() + 45
                while True:
                    try:
                        commande("etat")
                        break
                    except OSError:
                        if processus.poll() is not None or time.monotonic() > limite:
                            raise RuntimeError("L'instance de test ne démarre pas")
                        time.sleep(0.2)
                commande("nouveau", {"chemin": str(root / "Projet.modele.json")})
                composant = commande("famille_nouvelle", "composant")["fichier"]
                profondeur_initiale = composant["profondeur"]
                composant["profondeur"] = 750.0
                commande("famille_modifier", composant)
                commande("annuler")
                assert commande("famille_document")["fichier"]["profondeur"] == profondeur_initiale
                commande("retablir")
                assert commande("famille_document")["fichier"]["profondeur"] == 750.0
                commande("famille_fermer", "abandonner")
                fenetre = commande("famille_nouvelle", "fenetre")["fichier"]
                commande("famille_fermer", "abandonner")
                gabarit = json.loads(Path("gabarits/defaut.gabarit.json").read_text(encoding="utf-8"))
                profil = gabarit["profils"][0]["document_famille"]
                categories = [("composant", composant, "types_composants"), ("fenetre", fenetre, "types_ouvertures"),
                              ("profil", profil, "profils"), ("cartouche", profil, "familles_cartouches"),
                              ("etiquette_piece", profil, "familles_etiquettes")]
                avant = commande("projet_natif")["projet"]
                for categorie, base, _ in categories:
                    fichier = json.loads(json.dumps(base))
                    fichier["nom"] = "Import " + categorie
                    fichier["famille"]["categorie"] = categorie
                    chemin = root / (categorie + ".famille.json")
                    chemin.write_text(json.dumps(fichier, ensure_ascii=False), encoding="utf-8")
                    commande("famille_charger", str(chemin))
                apres = commande("projet_natif")["projet"]
                for _, _, registre in categories:
                    assert len(apres.get(registre, [])) == len(avant.get(registre, [])) + 1, registre
                ajoute = next(x for x in apres["types_composants"] if x["nom"] == "Import composant")
                assert ajoute["profondeur"] == 750.0
                print("Chargement des cinq catégories et profondeur du composant : OK")
            finally:
                processus.terminate()
                processus.wait(timeout=15)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=Path, default=Path("target/debug/bimedit.exe"))
    run(parser.parse_args().exe)
