"""Audit de couverture du service d'assets — ce qui est REELLEMENT servi, extension par extension.

Le VFS porte 255 308 fichiers repartis sur 39 extensions. Deux services les exposent :

  * `nie-site` (:8085) — `/f/<chemin VFS>` rend les octets BRUTS, sans decodage ;
  * `nie-model-serve` (:8790) — decode a la demande : `/tex/...png`, `/audio/...`, `/model/...`

Dire « le serving d'assets marche » n'a aucun sens sans compter. Ce script tire un echantillon
par extension, demande chaque fichier aux deux services, et rend un tableau : combien de
formats sont adressables, combien sont decodables, et ou sont les trous.

Il ne corrige rien. Il mesure, pour qu'on sache quoi corriger.

    uv run scripts/audit-serving-assets.py
    uv run scripts/audit-serving-assets.py --echantillon 10 --json var/audit-assets.json
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass, field

SITE = "http://127.0.0.1:8085"
AMONT = "http://127.0.0.1:8790"
DELAI = 30.0

# Les routes de decodage de `nie-model-serve`, par extension. `None` = aucun decodage n'est
# attendu pour ce format : l'absence de route n'est alors PAS un echec, c'est un choix.
#
# La convention est mesuree, pas devinee : `/tex/<chemin sans .g4tx>.png` — passer
# `.g4tx.png` rend 404, ce qui a deja coute une session entiere.
DECODAGE: dict[str, str | None] = {
    "g4tx": "/tex/{sans_ext}.png",
    "acb": "/audio-info/{chemin}",
    "awb": None,  # l'AWB seul n'a pas de table de cues : c'est l'ACB jumeau qui l'adresse
    "usm": "/video/catalog.json",
    "g4md": None,
    "g4mg": None,
    "g4sk": None,
    "g4mt": None,
    "g4pk": None,
    "g4pkm": None,
    "bin": None,
    "objbin": None,
    "p3lip": None,
}


@dataclass
class Resultat:
    """Ce qu'une extension donne, mesure sur son echantillon."""

    extension: str
    total_vfs: int
    echantillon: int = 0
    brut_ok: int = 0
    brut_echecs: list[tuple[str, str]] = field(default_factory=list)
    decode_ok: int = 0
    decode_echecs: list[tuple[str, str]] = field(default_factory=list)
    decodage_attendu: bool = False
    ms_median_brut: float = 0.0

    @property
    def taux_brut(self) -> float:
        return 100.0 * self.brut_ok / self.echantillon if self.echantillon else 0.0

    @property
    def taux_decode(self) -> float:
        return 100.0 * self.decode_ok / self.echantillon if self.echantillon else 0.0


def histogramme() -> list[tuple[str, int]]:
    """Les extensions du VFS et leur compte, par `niers vfs stats`."""
    sortie = subprocess.run(
        ["niers", "vfs", "stats"], capture_output=True, text=True, timeout=300, check=False
    ).stdout
    paires: list[tuple[str, int]] = []
    for ligne in sortie.splitlines():
        morceaux = ligne.split()
        if len(morceaux) == 2 and morceaux[0].isdigit() and morceaux[1].startswith("."):
            paires.append((morceaux[1][1:], int(morceaux[0])))
    return paires


def echantillonner(ext: str, combien: int) -> list[str]:
    """Des chemins reels de cette extension, tires par `niers vfs find`."""
    r = subprocess.run(
        ["niers", "vfs", "find", "data", "--ext", ext, "-n", str(combien), "--json"],
        capture_output=True,
        text=True,
        timeout=300,
        check=False,
    )
    if r.returncode != 0 or not r.stdout.strip():
        return []
    try:
        charge = json.loads(r.stdout)
    except json.JSONDecodeError:
        return []
    chemins = []
    for e in charge:
        c = e.get("chemin") or e.get("path") if isinstance(e, dict) else e
        if isinstance(c, str):
            chemins.append(c)
    return chemins


def demander(url: str) -> tuple[int, int, float]:
    """`(code, octets, millisecondes)`. Code 0 quand la connexion elle-meme echoue."""
    debut = time.monotonic()
    try:
        with urllib.request.urlopen(url, timeout=DELAI) as r:  # noqa: S310 - hote local fixe
            corps = r.read()
            return r.status, len(corps), (time.monotonic() - debut) * 1000
    except urllib.error.HTTPError as e:
        return e.code, 0, (time.monotonic() - debut) * 1000
    except Exception:  # noqa: BLE001 - toute panne reseau se compte pareil
        return 0, 0, (time.monotonic() - debut) * 1000


def auditer(ext: str, total: int, chemins: list[str]) -> Resultat:
    res = Resultat(extension=ext, total_vfs=total, echantillon=len(chemins))
    gabarit = DECODAGE.get(ext, "")
    res.decodage_attendu = bool(gabarit)
    durees = []

    for chemin in chemins:
        code, octets, ms = demander(f"{SITE}/f/{urllib.parse.quote(chemin)}")
        durees.append(ms)
        # Un 200 a zero octet est un echec : le fichier existe dans l'index et ne sort pas.
        if code == 200 and octets > 0:
            res.brut_ok += 1
        else:
            res.brut_echecs.append((chemin, f"HTTP {code}, {octets} o"))

        if gabarit:
            sans_ext = chemin.rsplit(".", 1)[0]
            url = AMONT + gabarit.format(chemin=urllib.parse.quote(chemin), sans_ext=urllib.parse.quote(sans_ext))
            code, octets, _ = demander(url)
            if code == 200 and octets > 0:
                res.decode_ok += 1
            else:
                res.decode_echecs.append((chemin, f"HTTP {code}, {octets} o"))

    if durees:
        durees.sort()
        res.ms_median_brut = durees[len(durees) // 2]
    return res


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--echantillon", type=int, default=5, help="fichiers testes par extension")
    ap.add_argument("--json", type=str, default="", help="ecrit le rapport complet en JSON")
    ap.add_argument("--min", type=int, default=0, help="ignore les extensions sous ce compte")
    args = ap.parse_args()

    for nom, url in (("nie-site", f"{SITE}/healthz"), ("nie-model-serve", f"{AMONT}/health")):
        code, _, _ = demander(url)
        if code != 200:
            print(f"{nom} ne repond pas ({url} -> {code}) — audit impossible", file=sys.stderr)
            return 2

    exts = [(e, n) for e, n in histogramme() if n >= args.min]
    if not exts:
        print("aucune extension mesuree — `niers vfs stats` a-t-il repondu ?", file=sys.stderr)
        return 2

    resultats = []
    for ext, total in exts:
        chemins = echantillonner(ext, args.echantillon)
        resultats.append(auditer(ext, total, chemins))

    couvert_brut = sum(r.total_vfs for r in resultats if r.echantillon and r.taux_brut == 100.0)
    total_vfs = sum(r.total_vfs for r in resultats)

    print(f"\n{'ext':<10} {'fichiers':>9} {'ech.':>5} {'brut':>7} {'decode':>8} {'ms':>7}")
    print("-" * 52)
    for r in sorted(resultats, key=lambda x: -x.total_vfs):
        decode = f"{r.taux_decode:.0f}%" if r.decodage_attendu else "—"
        brut = f"{r.taux_brut:.0f}%" if r.echantillon else "0 tire"
        print(
            f"{r.extension:<10} {r.total_vfs:>9} {r.echantillon:>5} {brut:>7} {decode:>8} "
            f"{r.ms_median_brut:>6.0f}"
        )

    print("-" * 52)
    print(f"{'TOTAL':<10} {total_vfs:>9}")
    print(f"\nOctets bruts servis a 100 % : {couvert_brut} fichiers sur {total_vfs} "
          f"({100.0 * couvert_brut / total_vfs:.1f} %)")

    fautifs = [r for r in resultats if r.echantillon and r.taux_brut < 100.0]
    if fautifs:
        print(f"\n{len(fautifs)} extension(s) avec des echecs sur les octets bruts :")
        for r in fautifs:
            print(f"  .{r.extension} — {r.brut_ok}/{r.echantillon}")
            for chemin, raison in r.brut_echecs[:3]:
                print(f"      {raison}  {chemin}")

    vides = [r for r in resultats if not r.echantillon]
    if vides:
        print(f"\n{len(vides)} extension(s) dont l'echantillonnage n'a rien rendu : "
              + ", ".join(f".{r.extension}" for r in vides))

    if args.json:
        with open(args.json, "w", encoding="utf-8") as f:
            json.dump(
                [
                    {
                        "extension": r.extension,
                        "total_vfs": r.total_vfs,
                        "echantillon": r.echantillon,
                        "brut_ok": r.brut_ok,
                        "taux_brut": round(r.taux_brut, 1),
                        "decodage_attendu": r.decodage_attendu,
                        "decode_ok": r.decode_ok,
                        "taux_decode": round(r.taux_decode, 1),
                        "ms_median_brut": round(r.ms_median_brut, 1),
                        "brut_echecs": r.brut_echecs[:10],
                        "decode_echecs": r.decode_echecs[:10],
                    }
                    for r in resultats
                ],
                f,
                ensure_ascii=False,
                indent=2,
            )
        print(f"\nrapport complet : {args.json}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
