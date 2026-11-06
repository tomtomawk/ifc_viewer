"""Régression du tracé de murs courbes, des poignées et de la sélection des traits."""
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
            commande("nouveau",{"chemin":str(root/"Courbes.modele.json")})
            h=commande("mur","-10000 -10000 10000 -10000 2800 300")["mur"]
            v=commande("mur","-10000 -8000 -10000 10000 2800 300")["mur"]
            commande("camera","cadrer")
            commande("selection_native",[h]);a=commande("axe_selection")
            commande("selection_native",[v]);b=commande("axe_selection")
            ux=[(a["fin_ecran"][i]-a["debut_ecran"][i])/(a["fin"][0]-a["debut"][0]) for i in range(2)]
            uy=[(b["fin_ecran"][i]-b["debut_ecran"][i])/(b["fin"][1]-b["debut"][1]) for i in range(2)]
            origine=[a["debut_ecran"][i]-ux[i]*a["debut"][0]-uy[i]*a["debut"][1] for i in range(2)]
            def ecran(x,y):return [origine[i]+ux[i]*x+uy[i]*y for i in range(2)]
            def clic(x,y):
                x,y=ecran(x,y);saisie([{"action":"clic","x":x,"y":y}])
            def murs():return commande("projet_natif")["projet"]["murs"]
            commande("mur_trace","cercle")
            avant=commande("projet_natif")
            for x,y in [(0,0),(4000,0),(4000,0)]:
                clic(x,y)
                assert commande("projet_natif")["empreinte"]==avant["empreinte"],"Mur créé avant la fin du périmètre"
            clic(0,4000)
            nouveaux=[m for m in murs() if m["id"] not in {h,v}]
            assert len(nouveaux)==1 and nouveaux[0]["axe"]["forme"]=="arc",nouveaux
            id=nouveaux[0]["id"]
            commande("outil","aucun");commande("selection_native",[id])
            controle=commande("mur_trace")["controle_arc"]
            assert controle and len(controle["points"])==3 and all(controle["ecran"]),controle
            avant=commande("projet_natif")
            x,y=controle["ecran"][1];tx,ty=ecran(3800,3800)
            saisie([{"action":"bouton","x":x,"y":y,"bouton":"gauche","appuye":True},{"action":"pointer","x":tx,"y":ty},{"action":"bouton","x":tx,"y":ty,"bouton":"gauche","appuye":False}])
            assert commande("projet_natif")["empreinte"]!=avant["empreinte"],"La poignée ne modifie pas l'arc"
            commande("annuler")
            assert commande("projet_natif")["empreinte"]==avant["empreinte"]
            # Sélection d'un trait droit du dessin : l'annotation reste en place.
            projet=commande("projet_natif")["projet"]
            vue=next(v["id"] for v in projet["vues"] if v["genre"]=="plan")
            commande("editer",{"action":"annoter","annotation":{"id":"","vue":vue,"hauteur":2.5,"forme":"ligne","debut":[-6000,-1000],"fin":[-2000,-1000]}})
            commande("mur_trace","selection")
            n=len(murs());ann=commande("projet_natif")["projet"]["annotations"]
            clic(-4000,-1000)
            assert len(murs())==n+1,"Le trait sélectionné n'a pas produit de mur"
            assert commande("projet_natif")["projet"]["annotations"]==ann
            commande("annuler")
            assert len(murs())==n
            # Un arc du dessin garde sa géométrie, sans facettes droites.
            commande("editer",{"action":"annoter","annotation":{"id":"","vue":vue,"hauteur":2.5,"forme":"arc","points":[[-6000,2000],[-4000,4000],[-2000,2000]]}})
            clic(-4000,4000)
            assert len(murs())==n+1 and murs()[-1]["axe"]["forme"]=="arc"
            # Un cercle déjà dessiné conserve son centre et son rayon au passage depuis Annotation.
            commande("outil","ligne")
            commande("editer",{"action":"annoter","annotation":{"id":"","vue":vue,"hauteur":2.5,"forme":"cercle","centre":[-5000,6000],"rayon":1500}})
            commande("mur_trace","selection")
            clic(-3500,6000)
            assert len(commande("mur_trace")["points"])==2,"Le cercle sélectionné a perdu son rayon"
            avant=len(murs())
            clic(-3500,6000);assert len(murs())==avant
            clic(-5000,7500);assert len(murs())==avant+1
            # Un cercle complet nécessite aussi les deux clics de périmètre.
            commande("mur_trace","cercle")
            avant=len(murs())
            for x,y in [(6000,6000),(8500,6000),(6000,8500)]:
                clic(x,y);assert len(murs())==avant
            clic(6000,8500)
            assert len(murs())==avant+2 and all(m["axe"]["forme"]=="arc" for m in murs()[-2:])
            # Arc : début, fin, puis courbure.
            commande("mur_trace","arc")
            avant=len(murs())
            for x,y in [(-6000,-5000),(-2000,-5000),(-4000,-3000)]:clic(x,y)  # début, fin, courbure
            assert len(murs())==avant+1 and murs()[-1]["axe"]["forme"]=="arc"
            commande("outil","aucun")
            commande("quitter","abandonner");p.wait(timeout=15)
            print(f"Murs courbes : {nombre} requêtes validées, quatre clics, contrôle de l'arc et sélection des traits OK")
        except Exception:
            log.flush()
            print((root/"instance.log").read_text(encoding="utf-8")[-3500:])
            raise
        finally:
            if p is not None and p.poll() is None:p.terminate();p.wait(timeout=10)
            log.close()

if __name__=="__main__":
    parser=argparse.ArgumentParser()
    parser.add_argument("--exe",type=Path,default=Path("target/release/bimedit.exe"))
    args=parser.parse_args();run(args.exe,Path("exports/verification-murs-courbes"))
