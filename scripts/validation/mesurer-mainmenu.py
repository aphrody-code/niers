# /// script
# requires-python = ">=3.11"
# dependencies = ["numpy", "pillow"]
# ///
"""Mesure la geometrie de l'ecran `mainmenu01` sur une capture du jeu.

Sortie : les boites en pixels de la capture ET en pixels du canevas 1280x720, plus la pente
des parallelogrammes. Ce sont ces valeurs-la qui sont posees dans
`packages/inacord-ui/src/shell/geometrie-mainmenu.ts` — le script existe pour qu'aucune ne
soit un souvenir.

    uv run scripts/validation/mesurer-mainmenu.py [capture.png]

La capture par defaut est `data/design/aphrody-ui-ref-mainmenu-7.1.2.png`. Elle est
**gitignoree** (`/data/*`, assets (c) LEVEL-5) : sur un clone frais il faut la fournir en
argument. C'est un screenshot, donc le rang 4 des sources de la skill `pixel-perfect` — il sert
aux POSITIONS et aux ordres de grandeur, jamais aux couleurs destinees a du code, qui se
reprennent sur la texture du VFS.

Deux precautions apprises en le construisant :

1. **Une boite englobante se mesure dans une fenetre qui ne touche pas le voisin.** Sonder le
   panneau gauche sur `x 0..700` rend `x1=700` — la borne de la fenetre, pas le bord du
   panneau. Toute valeur egale a une borne de sonde est rejetee ici meme (`SATURE`).
2. **La pente se lit ligne par ligne, pas par un ajustement de boite.** L'analyse anterieure
   (`docs/mainmenu01-analyse-visuelle.md`) concluait « angle non mesurable, R2 < 0,45 » parce
   qu'elle ajustait les bords d'une boite qui coupait la forme. En prenant le premier pixel
   non-fond de chaque ligne dans une fenetre serree, la meme image rend R2 = 1,000.
"""

import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

# Le canevas du jeu, celui de l'export de layout — toutes les valeurs y sont ramenees.
CANVAS_W, CANVAS_H = 1280, 720

# Le fond de l'ecran, mesure : un blanc tres legerement verdi, 69 % de la surface.
FOND = (249, 253, 249)
# Distance L1 au-dela de laquelle un pixel n'est plus du fond. 40 sur 765 : assez lache pour
# ignorer le bruit de compression, assez serre pour attraper le bleu le plus pale des tuiles.
SEUIL_FOND = 40

DEFAUT = Path(__file__).resolve().parents[2] / "data/design/aphrody-ui-ref-mainmenu-7.1.2.png"

# Fenetres de sonde, en fraction de la capture — la reference archivee fait 2497x1414 et la
# copie de travail 2048x1159, mais leur RATIO est le meme (1,766) : des fractions valent pour
# les deux, des pixels non.
ZONES: dict[str, tuple[float, float, float, float]] = {
    "notice": (0.000, 0.000, 0.254, 0.198),
    "titre": (0.342, 0.000, 0.664, 0.405),
    "version": (0.903, 0.000, 1.000, 0.060),
    "encart_haut_droit": (0.742, 0.086, 1.000, 0.198),
    "panneau_gauche": (0.000, 0.180, 0.330, 0.470),
    "panneau_droit": (0.680, 0.180, 1.000, 0.470),
    "plaque": (0.405, 0.371, 0.610, 0.492),
    "rangee": (0.029, 0.509, 0.967, 0.656),
    "bandeau": (0.464, 0.638, 0.806, 0.729),
    "rangee_basse": (0.293, 0.727, 0.708, 0.901),
    "coin_bas_gauche": (0.000, 0.811, 0.254, 0.932),
    "bannieres": (0.781, 0.729, 1.000, 0.932),
    "aide": (0.440, 0.915, 0.561, 0.975),
    "mention": (0.757, 0.932, 1.000, 0.984),
}

# Bords dont la pente est mesurable : fenetre serree, et un seul bord dedans.
# (nom, x0, x1, y0, y1, sens) en fractions ; sens=1 bord gauche, -1 bord droit.
BORDS: list[tuple[str, float, float, float, float, int]] = [
    ("tuile 1, bord gauche", 0.029, 0.195, 0.531, 0.634, 1),
    ("tuile 8, bord droit", 0.830, 0.977, 0.531, 0.634, -1),
    ("tuile basse 3, bord droit", 0.586, 0.708, 0.742, 0.856, -1),
    # Les deux bords du "V" central : ils penchent dans l'AUTRE sens que les tuiles, et
    # sont mesures sous le logo (y 0,30..0,39), la seule bande ou rien ne les recouvre.
    ("panneau gauche, bord droit", 0.300, 0.400, 0.405, 0.455, -1),
    ("panneau droit, bord gauche", 0.560, 0.850, 0.298, 0.375, 1),
]


def charger(chemin: Path) -> tuple[np.ndarray, int, int]:
    img = Image.open(chemin).convert("RGB")
    a = np.asarray(img).astype(np.int16)
    h, w, _ = a.shape
    masque = np.abs(a - np.array(FOND, dtype=np.int16)).sum(axis=2) > SEUIL_FOND
    return masque, w, h


def boite(masque: np.ndarray, w: int, h: int, zone: tuple[float, float, float, float]):
    zx0, zy0 = round(zone[0] * w), round(zone[1] * h)
    zx1, zy1 = round(zone[2] * w), round(zone[3] * h)
    sub = masque[zy0:zy1, zx0:zx1]
    ys, xs = np.nonzero(sub)
    if len(xs) == 0:
        return None
    x0, x1 = zx0 + int(xs.min()), zx0 + int(xs.max()) + 1
    y0, y1 = zy0 + int(ys.min()), zy0 + int(ys.max()) + 1
    # Une borne atteinte = la fenetre a coupe le sujet : la valeur ne vaut rien.
    sature = [
        nom
        for nom, val, lim in (("x0", x0, zx0), ("y0", y0, zy0), ("x1", x1, zx1), ("y1", y1, zy1))
        if abs(val - lim) <= 1
    ]
    return {
        "px": (x0, y0, x1, y1),
        "canevas": (
            round(x0 * CANVAS_W / w),
            round(y0 * CANVAS_H / h),
            round((x1 - x0) * CANVAS_W / w),
            round((y1 - y0) * CANVAS_H / h),
        ),
        "remplissage": round(100 * float(sub.mean()), 1),
        "sature": sature,
    }


def pente(masque: np.ndarray, w: int, h: int, bord: tuple[str, float, float, float, float, int]):
    nom, fx0, fx1, fy0, fy1, sens = bord
    x0, x1 = round(fx0 * w), round(fx1 * w)
    y0, y1 = round(fy0 * h), round(fy1 * h)
    pts = []
    for y in range(y0, y1, max(1, (y1 - y0) // 14)):
        nz = np.nonzero(masque[y, x0:x1])[0]
        if len(nz):
            pts.append((y, x0 + int(nz[0] if sens == 1 else nz[-1])))
    if len(pts) < 3:
        return {"nom": nom, "erreur": "moins de 3 points"}
    ya = np.array([p[0] for p in pts], dtype=float)
    xa = np.array([p[1] for p in pts], dtype=float)

    def ajuster(ya: np.ndarray, xa: np.ndarray):
        (m, b), res, *_ = np.linalg.lstsq(np.vstack([ya, np.ones_like(ya)]).T, xa, rcond=None)
        ss_tot = float(((xa - xa.mean()) ** 2).sum())
        r2 = 1 - (float(res[0]) / ss_tot if len(res) and ss_tot > 0 else 0.0)
        return float(m), float(b), r2

    m, b, r2 = ajuster(ya, xa)
    # Un SEUL point aberrant — la ligne ou la fenetre touche le bas du panneau, ou un reflet du
    # sprite — suffit a faire tomber le R2 de 1,00 a 0,07 et a faire conclure « non mesurable ».
    # On retire ce qui s'ecarte de plus de 4 px de l'ajustement, et on DIT combien.
    residus = np.abs(xa - (m * ya + b))
    garde = residus <= max(4.0, 3.0 * float(np.median(residus)))
    rejetes = int((~garde).sum())
    if rejetes and int(garde.sum()) >= 3:
        m, b, r2 = ajuster(ya[garde], xa[garde])
    return {
        "nom": nom,
        "dx_dy": round(m, 4),
        "angle_deg": round(float(np.degrees(np.arctan(m))), 2),
        "r2": round(r2, 4),
        "points": int(garde.sum()) if rejetes else len(pts),
        "rejetes": rejetes,
    }


def main() -> int:
    chemin = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAUT
    if not chemin.exists():
        print(f"capture introuvable : {chemin}", file=sys.stderr)
        print("elle est gitignoree (/data/*) — la passer en argument", file=sys.stderr)
        return 2
    masque, w, h = charger(chemin)
    print(f"# {chemin.name}  {w}x{h}  (ratio {w / h:.3f})  -> canevas {CANVAS_W}x{CANVAS_H}")
    print(f"{'zone':20s} {'x':>5s} {'y':>5s} {'l':>5s} {'h':>5s}   remplissage  reserve")
    sortie: dict[str, object] = {"capture": chemin.name, "taille": [w, h], "zones": {}}
    for nom, zone in ZONES.items():
        b = boite(masque, w, h, zone)
        if b is None:
            print(f"{nom:20s} vide")
            continue
        cx, cy, cl, ch = b["canevas"]
        alerte = f"SATURE {'+'.join(b['sature'])}" if b["sature"] else ""
        print(f"{nom:20s} {cx:5d} {cy:5d} {cl:5d} {ch:5d}   {b['remplissage']:9.1f}%  {alerte}")
        sortie["zones"][nom] = b
    print()
    print(f"{'bord':28s} {'dx/dy':>8s} {'angle':>8s} {'R2':>7s}  points")
    sortie["pentes"] = []
    for bord in BORDS:
        p = pente(masque, w, h, bord)
        if "erreur" in p:
            print(f"{p['nom']:28s} {p['erreur']}")
        else:
            print(
                f"{p['nom']:28s} {p['dx_dy']:+8.3f} {p['angle_deg']:+7.2f}° {p["r2"]:7.3f}  {p["points"]:3d} ({p["rejetes"]} rejetes)"
            )
        sortie["pentes"].append(p)
    if "--json" in sys.argv:
        print(json.dumps(sortie, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
