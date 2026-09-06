"""Génère le SQL d'insertion d'Astro Lor (personnage original) dans le wiki azalée.

Rejouable : tout passe par des `INSERT ... ON CONFLICT (id) DO UPDATE`, aucune ligne
existante du jeu n'est touchée. Les identifiants sont dans un espace de noms réservé
aux OC (`c99…` pour les codes internes, préfixes `ocs/oco/ocd/ock` pour les techniques)
qui ne peut pas entrer en collision avec les hachages extraits de `nie.exe`.

Source des textes : document de présentation d'Astro Lor (Google Docs, provenance
`data/oc/astro-lor/provenance/google-doc-10a6b9.json`) et les deux planches de référence
signées @Karumina_san. Les statistiques ne sont PAS inventées :
dans Victory Road elles ne dépendent que du couple (poste × rareté), et les blocs
repris ici sont ceux, mesurés en base, de tous les gardiens Normal et Héros.

    uv run scripts/donnees/astro-lor-oc.py > /tmp/astro.sql
    sudo -u postgres psql -d rg -f /tmp/astro.sql
"""

import json
import sys

if (sys.stdout.encoding or "").lower() != "utf-8":
    sys.stdout.reconfigure(encoding="utf-8")

# ---------------------------------------------------------------- identifiants

CHARA_ID = "0x796FA17D"
ID_OG = "0x9983CCE2"
ID_VR = "0x0C78B74B"
BASE_SLUG = "astro-lor"
TEAM_RAIMON = {"id": "0xF01BB293", "names": {"fr": "Raimon", "en": "Raimon", "ja": "雷門中"}}

# Blocs canoniques mesurés : tous les gardiens d'une rareté partagent le même Lv99.
STATS_LV99 = {
    "Normal": {"kick": 133, "control": 146, "technique": 137, "pressure": 164,
               "physical": 157, "agility": 166, "intelligence": 150},
    "Héros": {"kick": 207, "control": 223, "technique": 210, "pressure": 242,
              "physical": 225, "agility": 256, "intelligence": 223},
}
# Le Lv1 est la part individuelle ; les deux blocs ci-dessous existent tels quels en
# base sur d'autres gardiens — technique et maîtrise hautes, agilité basse, ce qui
# correspond au profil décrit (précision maximale, endurance et vitesse faibles).
STATS_LV1_OG = {"kick": 11, "control": 12, "technique": 13, "pressure": 11,
                "physical": 10, "agility": 9, "intelligence": 11}
STATS_LV1_VR = {"kick": 12, "control": 13, "technique": 12, "pressure": 11,
                "physical": 10, "agility": 9, "intelligence": 11}

# ---------------------------------------------------------------- techniques

CAT = {"Tir": (1, "ocs", "Shoot"), "Dribble": (2, "oco", "Dribble"),
       "Défense": (3, "ocd", "Block"), "Arrêt": (4, "ock", "Catch")}
ELEMENT = ("Forêt", 2, {"en": "Forest", "fr": "Forêt", "ja": "林"})

# (rang, nom FR, nom EN, catégorie, description FR ou None)
# Le rang est celui du classement de puissance donné par l'auteur·rice (1 → 6).
# Une description n'est écrite que lorsque le document en donne une : rien n'est inventé.
TECHNIQUES = [
    (1, "Étoile Filante", "Shooting Star", "Tir", None),
    (2, "Pluie d’Étoiles", "Star Rain", "Tir",
     "Technique créée par Asta Lor et réservée à un gardien de but : un lancer haut et "
     "long qui porte jusqu’au milieu de terrain, prolongé par un tir plongeant."),
    (3, "Signe du Bélier", "Sign of Aries", "Tir", "Seconde attaque."),
    (4, "Cauchemar Imminent", "Looming Nightmare", "Tir",
     "Employée durant la période Inazuma Eleven."),
    (5, "Rêve Lucide", "Lucid Dream", "Tir", None),
    (6, "Signe du Gémeaux", "Sign of Gemini", "Tir",
     "Tir en duo, avec Shawn Froste ou avec Asta Lor."),
    (3, "Baguette Fluorescente", "Fluorescent Baguette", "Tir",
     "Tir de la période Rose Griffon ; se placerait au milieu du classement."),
    (4, "Tempête de Sable", "Sandstorm", "Tir",
     "Créée dans Ares / Orion en s’inspirant de la super-technique d’Axel Blaze, "
     "gaucher comme iel."),

    (1, "Saute-Mouton", "Leapfrog", "Dribble", None),
    (2, "Patinage", "Ice Skating", "Dribble", None),
    (3, "Passe Fantôme", "Phantom Pass", "Dribble",
     "Employée durant la période Inazuma Eleven."),
    (4, "Rose Endormie", "Sleeping Rose", "Dribble", None),
    (5, "Effet Miroir", "Mirror Effect", "Dribble", None),
    (6, "Signe du Scorpion", "Sign of Scorpio", "Dribble", None),

    (1, "Chasseur de Rêve", "Dream Hunter", "Défense", None),
    (2, "Anneau de Jupiter", "Ring of Jupiter", "Défense", None),
    (3, "Signe du Taureau", "Sign of Taurus", "Défense", None),
    (4, "Sable Mouvant", "Quicksand", "Défense", "Défense en duo avec Jack Wallside."),
    (5, "Double Reverse", "Double Reverse", "Défense",
     "Contre-attaque : renvoie l’attaque adverse en doublant sa puissance."),
    (6, "Contre en Tandem", "Tandem Counter", "Défense",
     "Contre-attaque en duo avec Archer Hawkins, qui partage le même type de contre. "
     "Aussi appelée « Contre-sort »."),

    (1, "Rideau Boréal", "Aurora Curtain", "Arrêt", None),
    (2, "Lambswool", "Lambswool", "Arrêt", None),
    (3, "Système Solaire", "Solar System", "Arrêt", None),
    (4, "Signe du Capricorne", "Sign of Capricorn", "Arrêt", None),
    (5, "Trou Noir", "Black Hole", "Arrêt", "Employée durant la période Inazuma Eleven."),
    (6, "Vœux Précieux", "Precious Wish", "Arrêt",
     "Arrêt le plus puissant d’Astro, employé aussi avec son esprit guerrier, "
     "Morphée le Dieu des Rêves."),
]


def skill_id(categorie: str, rang: int, ordre: int) -> str:
    return f"{CAT[categorie][1]}{rang}{ordre:03d}"


skills_rows = []
by_name = {}
compteur = {c: 0 for c in CAT}
for rang, nom_fr, nom_en, categorie, desc in TECHNIQUES:
    compteur[categorie] += 1
    sid = skill_id(categorie, rang, compteur[categorie])
    cat_id, _prefix, cat_en = CAT[categorie]
    power_min = 20 + rang * 10
    power_max = 150 + rang * 45
    tp = 25 + rang * 8
    data = {
        "skillID": sid,
        "skillIDStr": sid,
        "internalCode": sid,
        "name_FR": nom_fr, "name_EN": nom_en,
        "displayName": nom_fr,
        "desc_FR": desc,
        "category": cat_id,
        "categoryName": {"fr": categorie, "en": cat_en},
        "element": ELEMENT[1],
        "elementName": ELEMENT[2],
        "power_min": power_min, "power_max": power_max,
        "powerRange": f"{power_min}-{power_max}",
        "consumeTp": tp,
        "recastTime": 50,
        "growthType": 0,
        "skillType": "main",
        "hasPartner": "duo" in (desc or "").lower(),
        "partnerCount": 1 if "duo" in (desc or "").lower() else 0,
        "tags": ["oc", "astro-lor"],
        "origin": "Personnage original — Astro Lor",
    }
    skills_rows.append((sid, nom_fr, nom_en, desc, categorie, cat_id, power_min,
                        power_max, tp, data))
    by_name[nom_fr] = sid

# ---------------------------------------------------------------- movesets

MOVESET_OG = [("Rideau Boréal", 1), ("Étoile Filante", 1), ("Saute-Mouton", 13),
              ("Chasseur de Rêve", 20), ("Pluie d’Étoiles", 30), ("Système Solaire", 43)]
MOVESET_VR = [("Vœux Précieux", 1), ("Rêve Lucide", 20), ("Double Reverse", 30),
              ("Signe du Capricorne", 43), ("Signe du Gémeaux", 55), ("Effet Miroir", 60)]


def moveset(paires):
    return [{"skillId": by_name[n], "learnLevel": lv} for n, lv in paires]


# ---------------------------------------------------------------- biographie

WIKI_SECTIONS = [
    {"title": "Identité", "content": (
        "Prénom Astro, nom Lor à l’origine et Kin en japonais. Surnommé·e « le monstre » en "
        "Russie, « le marchand de sable » à Raimon et à l’Inazuma Japon, « la baguette d’RG » "
        "à Rose Griffon et « l’étoile filante » dans GO.\n\n"
        "Origines russes, japonaises et d’Asie occidentale ; nationalité japonaise, russe et "
        "française dans les branches Ares / Orion et Rose Griffon. Non-binaire, pronoms iel et "
        "il. Né·e le 15 juin, Gémeaux, groupe sanguin AB+. 162 cm enfant, 168 cm adulte.\n\n"
        "Numéro 19. Gardien, libéro et manageur·se, de type Bois. Sa force : une vue qu’iel sait "
        "approfondir et une précision successive. Sa faiblesse : une endurance physique minime — "
        "c’est pourquoi iel n’entre qu’en seconde mi-temps, ou en cas d’urgence."
    )},
    {"title": "Apparence", "content": (
        "Roux et les yeux brillants à l’origine ; le stress et les traumatismes ont viré les "
        "cheveux au blond rosé et les yeux au vert sombre. La couleur d’origine n’est revenue que "
        "sur la frange, et les yeux ont retrouvé leurs teintes sans leur éclat.\n\n"
        "Cheveux bouclés et longs, qu’iel refuse de couper — contre ses parents, et par question "
        "de genre. Taches de rousseur, cernes d’insomniaque dans la période Inazuma Eleven, "
        "disparues une fois rétabli·e.\n\n"
        "Les yeux caméléon viennent de sa mère : verts à la base, avec du bleu, du bleu foncé, du "
        "violet, et du jaune au centre. Ils changent de forme et de couleur selon l’émotion — le "
        "violet signale l’autre personnalité, précédée d’une lueur sombre. Avec eux, iel a le "
        "niveau de vision des lunettes de Kidou, en plus précis, et l’habitude d’analyser "
        "quelqu’un d’un seul regard.\n\n"
        "Cicatrices aux mains (entraînements en cage), dans le dos (griffures d’ours, qui "
        "évoquent des ailes), en haut du torse (tentative d’assassinat), aux cuisses. Dans GO, "
        "iel a perdu la jambe droite et porte une prothèse mécanique."
    )},
    {"title": "Personnalité", "content": (
        "Enfant : jovial·e, très expressif·ve, beaucoup d’énergie et le franc-parler facile — "
        "mais silencieux·se dès que ses parents sont là.\n\n"
        "Saison 1 : ferme et distant·e, tout en faisant des efforts dès qu’il s’agit d’aider. "
        "Visage neutre, et aucune hésitation à se défendre. La part joviale ne ressort qu’avec la "
        "confiance : un sourire au coin suffit, sauf quand une super-technique lui allume les "
        "yeux.\n\n"
        "Saison 2 : plus craintif·ve, ce qu’iel vit comme une faiblesse à cacher — quatre amis "
        "seulement en savent une partie. Saison 3 : pire encore au départ, jusqu’à la finale "
        "contre les Gigantes où la voix de sa mère lui rend son énergie.\n\n"
        "GO : après le coma, iel sourit tout le temps et taquine beaucoup, retrouvant peu à peu "
        "son caractère d’enfant. À trente ans, rééducation et suivi psychologique terminés, iel "
        "redevient l’Astro qu’iel aurait dû être : à peu près le caractère de Mark Evans, en plus "
        "taquin. Tendre et débordant·e à la fois."
    )},
    {"title": "Santé", "content": (
        "Trouble anxieux, trouble dissociatif de l’identité, TDAH, HPI diagnostiqué plus tard, "
        "syndrome de l’aidant, insomnie et hypersomnie. Il aura fallu attendre l’après-coma pour "
        "qu’iel consulte enfin une psychologue.\n\n"
        "Côté physique, la douleur la plus vive vient de la jambe droite : les injections reçues "
        "enfant ont fini par s’infecter, sans qu’aucun médecin ne pose de diagnostic. Iel a fini "
        "par perdre la jambe.\n\n"
        "Phobies : les hôpitaux, le sang, les miroirs. Et détester qu’on le prenne par les "
        "poignets — on le faisait pour le forcer."
    )},
    {"title": "Parcours", "content": (
        "Enfance — Né·e à Hokkaido avec sa sœur jumelle Asta, qu’iel initie au football. La "
        "famille se délite : trahison du père, violences, fuite de la mère, arrivée d’une "
        "belle-mère aux intentions malveillantes. Des injections censées améliorer leurs "
        "performances laissent des séquelles durables. En l’absence d’Astro, la belle-mère tire "
        "sur Asta, qui meurt dans ses bras.\n\n"
        "Russie — Expatrié·e de force, entraînements d’une brutalité extrême, harcèlement de "
        "Spyke, rumeur de sa culpabilité dans la mort d’Asta. En finale de FFI contre la France, "
        "son Double Reverse blesse gravement le capitaine adverse ; la Russie est disqualifiée. "
        "Son grand-père Aleksandr retire les droits parentaux et le prend sous son aile.\n\n"
        "Raimon — Arrivé·e à Inazuma, iel s’inscrit à Raimon après avoir vu la main magique de "
        "Mark Evans. Manageur d’abord, joueur ensuite, à partir du match contre Zeus. En saison 3 "
        "iel prend la place que Darren Lachance lui cède, et gagne la FFI.\n\n"
        "Adolescence — Capitaine de Raimon après le départ de Mark. À quinze ans, iel rencontre "
        "puis adopte Heka. Deux ans plus tard, une ordonnance trafiquée par le cinquième secteur "
        "provoque une surdose : trois ans de coma.\n\n"
        "GO — Réveil, jambe droite amputée, deux semaines d’hôpital arrachées de force, une "
        "rééducation menée en secret. Iel se montre officiellement en finale contre Kirkwood.\n\n"
        "Chrono Stone et Galaxy — Anomalie du système, iel suit Clark Wonderbot et découvre ses "
        "origines ; puis répond à une invitation venue d’une planète lointaine.\n\n"
        "Victory Road — À trente-cinq ans, iel exerce enfin son métier de rêve : thérapeute pour "
        "jeunes enfants."
    )},
    {"title": "Relations", "content": (
        "Famille — Alan Lor, le père, qui ne sera jamais fier d’iel. Rinko Chiaki, la mère, qui "
        "voulait l’appeler Aurore et se rattrape en saison 3. Asta Lor, sa sœur jumelle. Veronica "
        "Lor Miles, la belle-mère, qu’iel déteste. Zurvan Lor Miles, le beau-frère médecin. "
        "Aleksandr Lor, le grand-père astrologue qui l’a libéré·e. Chou Lor, la grand-mère. Heka "
        "Lor, sa fille adoptive.\n\n"
        "Raimon — Mark Evans, son gardien modèle. Jude Sharp, un père de cœur et un rival de "
        "stratégie. Kevin Dragonfly, rival devenu grand frère. Axel Blaze, frère de cœur, dont le "
        "père est le seul médecin qu’iel laisse l’approcher. Celia Hills, son premier crush. "
        "Darren Lachance, Scotty Banks, Austin Hobbes, Archer Hawkins, Willy Glass, Sue.\n\n"
        "Hokkaido — Shawn Froste, ami d’enfance devenu compagnon : ensemble à partir de Chrono "
        "Stone, demande en mariage cinq ans plus tard, un jour de Noël. Aiden Froste, rival "
        "d’enfance. Dawn Froste, grande sœur.\n\n"
        "Ailleurs — Ichihoshi Hikaru, l’ami d’hôpital. Nosaka Yuuma et Umahira Norika dans Ares "
        "et Orion. Victoria Day, seule alliée en Russie. Spyke, son harceleur. Wondeba, ours en "
        "peluche et exception à sa phobie. Master Dragon. Chitoh, son descendant, gardien de type "
        "Bois comme iel. Addison Norris, son cousin."
    )},
    {"title": "Techniques hors moveset", "content": (
        "Le classement complet établi par l’auteur·rice va de 1 (la moins puissante) à 6.\n\n"
        "Tir — Étoile Filante, Pluie d’Étoiles, Signe du Bélier, Cauchemar Imminent, Rêve Lucide, "
        "Signe du Gémeaux. Plus, hors classement, la Baguette Fluorescente (Rose Griffon) et la "
        "Tempête de Sable (Ares / Orion).\n\n"
        "Attaque — Saute-Mouton, Patinage, Passe Fantôme, Rose Endormie, Effet Miroir, Signe du "
        "Scorpion.\n\n"
        "Défense — Chasseur de Rêve, Anneau de Jupiter, Signe du Taureau, Sable Mouvant, Double "
        "Reverse, Contre en Tandem.\n\n"
        "Gardien — Rideau Boréal, Lambswool, Système Solaire, Signe du Capricorne, Trou Noir, "
        "Vœux Précieux.\n\n"
        "Stratégie — Signe du Lion et Chemin d’Étoile ; ces deux-là n’ont pas d’équivalent parmi "
        "les quatre catégories du jeu et ne sont donc pas fichées comme techniques.\n\n"
        "Dans Orion, iel prend la place de Shawn Froste sur l’Icebreaker, technique duo interdite : "
        "sa prothèse l’en immunise.\n\n"
        "Esprit guerrier : Morphée, le Dieu des Rêves. Mixi Max recensés : Master Dragon, Shawn "
        "Froste, Celia Hills et Asta Lor."
    )},
    {"title": "Hors du terrain", "content": (
        "Style — d’abord des pulls et des sweats, un peu geek, parfois rock. Puis, après le match "
        "contre les otakus, la mode Lolita, styles royal et casual, sur une base de fantaisie "
        "naturelle aux couleurs de l’automne.\n\n"
        "Chambre — peluches, attrape-rêves, cristaux, huiles essentielles, étoiles au plafond, un "
        "petit atelier de travail, et des livres sur les rêves, l’astrologie, l’histoire de l’art, "
        "le football et la stratégie.\n\n"
        "Aime — le violet, qui est la définition même du rêve, et l’orange pour sa gaieté. Les "
        "hortensias. Les takoyaki, la glace, le citron. Le chocolat chaud, nostalgie d’Hokkaido. "
        "Les moutons et les chats. Le patinage. Halloween. Son carnet, les mangas, les rêves "
        "lucides.\n\n"
        "Déteste — les tricheurs, les conflits, les trahisons, les oignons, les boissons "
        "énergisantes, le rugby, les ours, l’higanbana, et la Russie.\n\n"
        "Métier — manageur·se professionnel·le indépendant·e, spécialité analyse et stratégie ; "
        "assistant·e au copyrighting auprès de son grand-père ; puis thérapeute pour jeunes "
        "enfants dans Victory Road."
    )},
    {"title": "À propos de cette fiche", "content": (
        "Astro Lor est un personnage original (OC), pas un personnage du jeu. Il est fiché ici "
        "avec les mêmes structures que le reste du wiki.\n\n"
        "Ses statistiques ne sont pas inventées : dans Victory Road, le bloc de statistiques d’un "
        "joueur au niveau 99 ne dépend que de son poste et de sa rareté. Les valeurs affichées "
        "sont donc exactement celles de tous les gardiens Normal, et de tous les gardiens Héros.\n\n"
        "Les descriptions de techniques ne sont renseignées que là où le document de présentation "
        "en donne une ; les autres sont volontairement laissées vides plutôt qu’inventées.\n\n"
        "Character design et planches de référence : @Karumina_san."
    )},
]


def q(valeur):
    """Littéral SQL."""
    if valeur is None:
        return "NULL"
    if isinstance(valeur, bool):
        return "TRUE" if valeur else "FALSE"
    if isinstance(valeur, (int, float)):
        return str(valeur)
    if isinstance(valeur, (dict, list)):
        return "$j$" + json.dumps(valeur, ensure_ascii=False) + "$j$::jsonb"
    return "$t$" + str(valeur) + "$t$"


def personnage(*, id_, code, slug, series, rarity_label, rarity_code, lv1, lv99,
               image, zukan, is_primary, description, moveset_pairs, nickname,
               age_group, wiki_sections):
    skills = moveset(moveset_pairs)
    data = {
        "slug": slug, "base_slug": BASE_SLUG,
        "charaId": CHARA_ID, "charaParamId": id_, "internalCode": code,
        "names": {"fr": "Astro Lor", "en": "Astro Lor", "ja": "金 アストロ"},
        "descriptions": {"fr": description},
        "element": "Forest", "elementRaw": 2,
        "position": "GK", "positionRaw": 1, "subPosition": "DF",
        "rarity": rarity_label, "rarityCode": rarity_code,
        "series": series,
        "gender": 2,
        "stats": {"lv1": lv1, "lv99": lv99},
        "skills": skills,
        "teams": [TEAM_RAIMON],
        "image": image,
        "icons": {"face": image},
        "is_primary": is_primary,
        "zukanOrder": zukan,
        "growthPattern": 2,
        "uniformNumber": 19,
        "origin": "Personnage original — fiche établie d’après le document de présentation d’Astro Lor.",
        "author": "@Karumina_san",
    }
    colonnes = {
        "id": id_, "chara_id": CHARA_ID, "internal_code": code,
        "name_fr": "Astro Lor", "name_en": "Astro Lor", "name_ja": "金 アストロ",
        "description_fr": description, "description_en": None, "description_ja": None,
        "rarity": rarity_label, "rarity_code": rarity_code, "rarity_label": rarity_label,
        "element": "Forêt", "position": "Gardien", "gender": "X",
        "image_url": image,
        "skills": skills, "teams": [TEAM_RAIMON],
        "series": series, "slug": slug, "team_id": TEAM_RAIMON["id"],
        "stat_frappe": lv99["kick"], "stat_controle": lv99["control"],
        "stat_technique": lv99["technique"], "stat_pression": lv99["pressure"],
        "stat_physique": lv99["physical"], "stat_agilite": lv99["agility"],
        "stat_intelligence": lv99["intelligence"],
        "stat_lv1_frappe": lv1["kick"], "stat_lv1_controle": lv1["control"],
        "stat_lv1_technique": lv1["technique"], "stat_lv1_pression": lv1["pressure"],
        "stat_lv1_physique": lv1["physical"], "stat_lv1_agilite": lv1["agility"],
        "stat_lv1_intelligence": lv1["intelligence"],
        "zukan_order": zukan, "base_slug": BASE_SLUG,
        "data": data, "is_controllable": True, "is_primary": is_primary,
        "control_type": "1", "age_group": age_group, "nickname": nickname,
        "uniform_number": 19, "wiki_sections": wiki_sections,
    }
    noms = ", ".join(colonnes)
    valeurs = ", ".join(q(v) for v in colonnes.values())
    maj = ", ".join(f"{c} = EXCLUDED.{c}" for c in colonnes if c != "id")
    return (f"INSERT INTO public.inagle_characters ({noms})\nVALUES ({valeurs})\n"
            f"ON CONFLICT (id) DO UPDATE SET {maj}, updated_at = now();")


DESC_OG = ("Gardien, libéro et manageur venu d’Hokkaido puis de Russie. Analyse une équipe "
           "d’un seul regard, mais son endurance ne lui permet d’entrer qu’en seconde mi-temps.")
DESC_VR = ("Ancien gardien de Raimon, aujourd’hui thérapeute pour jeunes enfants. Marche avec "
           "une prothèse à la jambe droite, et lit toujours le jeu avant qu’il n’arrive.")

print("BEGIN;")
print("-- Astro Lor — personnage original. Rejouable.")
print()

for sid, nom_fr, nom_en, desc, categorie, cat_id, pmin, pmax, tp, data in skills_rows:
    colonnes = {
        "id": sid, "internal_code": sid, "hash_id": sid,
        "name_fr": nom_fr, "name_en": nom_en, "name_ja": None,
        "description_fr": desc, "description_en": None, "description_ja": None,
        "category": categorie, "category_id": cat_id,
        "element": ELEMENT[0], "element_id": ELEMENT[1],
        "power_min": pmin, "power_max": pmax,
        "tp_cost": tp, "tension_cost": tp, "recast_time": 50,
        "growth_type": "Lente", "foul_rate": 0,
        "is_hyper": False, "is_eldorado": False,
        "partner_count": data["partnerCount"],
        "tags": None,
        "data": data,
    }
    noms = ", ".join(colonnes)
    valeurs = ", ".join(q(v) for v in colonnes.values())
    maj = ", ".join(f"{c} = EXCLUDED.{c}" for c in colonnes if c != "id")
    print(f"INSERT INTO public.inagle_skills ({noms})\nVALUES ({valeurs})\n"
          f"ON CONFLICT (id) DO UPDATE SET {maj}, updated_at = now();")
    print()

print(personnage(
    id_=ID_OG, code="c99019010", slug=f"{BASE_SLUG}-{ID_OG}", series="Inazuma Eleven",
    rarity_label="Normal", rarity_code=0, lv1=STATS_LV1_OG, lv99=STATS_LV99["Normal"],
    image="/oc/astro-lor/face-og.webp", zukan=5407, is_primary=True,
    description=DESC_OG, moveset_pairs=MOVESET_OG,
    nickname="Le marchand de sable", age_group="Collège",
    wiki_sections=WIKI_SECTIONS,
))
print()
print(personnage(
    id_=ID_VR, code="c99019020", slug=f"{BASE_SLUG}-{ID_VR}", series="Victory Road",
    rarity_label="Héros", rarity_code=10, lv1=STATS_LV1_VR, lv99=STATS_LV99["Héros"],
    image="/oc/astro-lor/face-go.webp", zukan=5408, is_primary=False,
    description=DESC_VR, moveset_pairs=MOVESET_VR,
    nickname="L’étoile filante", age_group="Adulte",
    wiki_sections=WIKI_SECTIONS,
))
print()
print("COMMIT;")
print("NOTIFY pgrst, 'reload schema';")
