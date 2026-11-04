"""Régression des options de vue et de l’inversion des murs dans une instance isolée."""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time


def run(exe, captures):
    captures.mkdir(parents=True, exist_ok=True)
    captures=captures.resolve()
    with tempfile.TemporaryDirectory(prefix="bimedit-interface-") as dossier:
        root=Path(dossier)
        with socket.socket() as s:
            s.bind(("127.0.0.1",0));port=s.getsockname()[1]
        env=dict(os.environ,BIMEDIT_PORT=str(port),APPDATA=str(root/"configuration"),BIMEDIT_CACHE_COLLABORATION=str(root/"cache"))
        log=(root/"instance.log").open("w",encoding="utf-8")
        p=None
        nombre=0

        def commande(nom,argument=None,ok=True):
            nonlocal nombre
            if argument is not None and not isinstance(argument,str):
                argument=json.dumps(argument,ensure_ascii=False,separators=(",",":"))
            ligne=nom+(" "+argument if argument is not None else "")
            with socket.create_connection(("127.0.0.1",port),timeout=30) as s:
                s.sendall((ligne+"\n").encode());r=json.loads(s.makefile("r",encoding="utf-8").readline())
            assert r.get("ok")==ok,(nom,r)
            nombre+=1
            return r

        def attendre(f,timeout=15):
            fin=time.monotonic()+timeout
            while time.monotonic()<fin:
                r=f()
                if r:return r
                time.sleep(.05)
            raise AssertionError("Délai dépassé")

        def demarrer():
            nonlocal p
            p=subprocess.Popen([str(exe.resolve())],env=env,stdout=log,stderr=log)
            def pret():
                try:return commande("etat")["ok"]
                except OSError:return False
            attendre(pret,45)

        def saisie(entrees):
            commande("saisie",entrees)
            attendre(lambda:commande("etat")["saisies_en_attente"]==0)

        def capture(nom,zone="fenetre"):
            commande("capture",f"{zone} {captures/nom}")

        try:
            demarrer()
            commande("nouveau",{"chemin":str(root/"Verifications.modele.json")})
            id=commande("mur","0 0 4000 0 2800 300")["mur"]
            commande("outil","aucun");commande("selection_native",[id])
            avant=commande("projet_natif")
            axes=commande("axe_selection")
            saisie([{"action":"touche","touche":"Space"}])
            inverse=commande("axe_selection")
            proche=lambda a,b:sum((x-y)**2 for x,y in zip(a,b))<0.01
            assert proche(axes["debut"],inverse["fin"]) and proche(axes["fin"],inverse["debut"]),(axes,inverse)
            assert commande("projet_natif")["empreinte"]!=avant["empreinte"]
            commande("annuler")
            assert commande("projet_natif")["empreinte"]==avant["empreinte"]
            commande("editer",{"action":"niveau","nom":"Etage","altitude":3000})
            commande("vue_plan","Etage 1200")
            a=commande("projet_natif");vues=a["projet"]["vues"]
            plans=[(i,v) for i,v in enumerate(vues) if v["genre"]=="plan"]
            assert len(plans)>=2
            i,v=plans[-1];source=plans[0][1]["id"]
            etage=next(n["id"] for n in a["projet"]["niveaux"] if n["nom"]=="Etage")
            commande("modifier_projet",{"empreinte":a["empreinte"],"operations":[{"op":"add","path":f"/vues/{i}/fond_plan","value":source},{"op":"add","path":f"/vues/{i}/limite_haute_niveau","value":etage},{"op":"replace","path":f"/vues/{i}/limite_haute_decalage","value":3200}]})
            v=commande("projet_natif")["projet"]["vues"][i]
            assert v["fond_plan"]==source and v["limite_haute_decalage"]==3200
            commande("options",{"theme_interface":"sombre"})
            assert commande("options")["options"]["theme_interface"]=="sombre"
            commande("options",{"theme_interface":"systeme"})
            commande("enregistrer")
            cmdpath=str(root/"Verifications.modele.json")
            commande("fermer")
            commande("ouvrir",cmdpath)
            attendre(lambda:commande("projet_natif")["projet"]["vues"][i].get("fond_plan")==source)
            commande("quitter");p.wait(timeout=15)
            print(f"Sélection, inversion, fond de plan, limite haute et thème : {nombre} requêtes validées")
        except Exception:
            log.flush();print((root/"instance.log").read_text(encoding="utf-8")[-2500:]);raise
        finally:
            if p is not None and p.poll() is None:p.terminate();p.wait(timeout=10)
            log.close()

if __name__=="__main__":
    parser=argparse.ArgumentParser();parser.add_argument("--exe",type=Path,default=Path("target/release/bimedit.exe"))
    args=parser.parse_args();run(args.exe,Path("exports/verification-options"))
