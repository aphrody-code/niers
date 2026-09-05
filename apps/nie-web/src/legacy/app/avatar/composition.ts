import type { EtatAvatar } from "./projet";
import type { Catalogue } from "./types";

/** Compose la requête canonique depuis les identifiants de pièces, indépendamment de leur rang UI. */
export function composerUrlAvatar(catalogue: Catalogue, cdn: string, avatar: EtatAvatar): string | null {
	const { choix, valeurs, champs, genre, morphologie } = avatar;
	// Le SQUELETTE de la morphologie choisie, et rien d'autre : le serveur en déduit le corps
	// habillé qui va avec. L'appariement corps↔squelette est mesuré (chaque variante de corps
	// épouse un squelette à 33 mm près, tout autre appariement dépassant 194 mm) et vit dans
	// `nie_formats::assemble::avatar_bodies_for_skeleton` — pas ici.
	//
	// Le squelette est celui que le catalogue donne : `modeles2` de la part de la catégorie 17
	// pointe `_bodySK/<code>_edit/<code>_edit.g4sk`. C'est cette pièce, et elle seule, qui fait
	// changer la silhouette avec le genre et la taille — de 1,25 m à 2,08 m selon le choix.
	// La part se retrouve par son NOM, pas par son rang : la catégorie 17 compte 13 entrées
	// (7 masculines et 6 féminines) alors que le curseur de physionomie en pilote 8. Chaque
	// morphologie du catalogue a exactement une part `edit_body_<nom>`, et les variantes
	// féminines partagent le squelette de leur équivalent — `smallfemale` comme `small`
	// pointent `c000201_edit`, ce qui rend le genre sans effet ici.
	const catMorpho = catalogue.categories.find((c) => c.faceSettingType === 17);
	// Le GENRE et la MORPHOLOGIE sont deux réglages distincts de l'interface, mais le jeu, lui,
	// les fond dans une seule liste : `male` et `female` y sont deux morphologies parmi les
	// huit. Sans ce raccord, changer le genre ne changeait rien au modèle — l'état `genre`
	// pilotait l'interface sans jamais atteindre l'URL.
	const morphos = catalogue.modelesDeBase.morphologies;
	const nomBrut = morphos[morphologie];
	const nomMorpho =
		genre === 1 && nomBrut === "male"
			? "female"
			: genre === 0 && nomBrut === "female"
				? "male"
				: nomBrut;
	const partMorpho =
		catMorpho?.parts.find((p) => p.id === choix[17]) ??
		catMorpho?.parts.find((p) => p.resource === `edit_body_${nomMorpho}`);
	const cheminSk = partMorpho?.modeles2?.find((m) => m.endsWith(".g4sk"));
	const squelette = cheminSk?.split("/").pop()?.replace(/\.g4sk$/, "");

	const pieces: string[] = [];
	if (squelette) pieces.push(`_bodySK/${squelette}`);

	// La MAILLE de tête ne dépend que de deux choses : la morphologie et le nez. Les 42 entrées
	// de `visages` sont indexées nez-major et ne portent que 7 ressources distinctes par
	// morphologie. Tout le reste du visage (forme, yeux, pupilles, sourcils, bouche) est de la
	// TEXTURE, pas de la géométrie.
	const catNez = catalogue.categories.find((c) => c.faceSettingType === 9);
	const iNez = Math.max(0, catNez?.parts.findIndex((p) => p.id === choix[9]) ?? 0);
	const visage = catalogue.modelesDeBase.visages[iNez] ?? catalogue.modelesDeBase.visages[0];
	const res = visage?.resources[morphologie] ?? visage?.resources[0];
	if (res) pieces.push(`_facebase/${res}`);

	// Chaque rubrique choisie apporte soit une MAILLE (`.g4md` — coiffure, oreilles,
	// accessoire), soit une COUCHE DE TEXTURE (`.g4tx` sous `_facetex` — peau, yeux, pupilles,
	// reflets, sourcils, bouche). Les secondes sont de loin les plus nombreuses, et c'est
	// pourquoi tant de rubriques semblaient sans effet tant que seules les mailles étaient
	// envoyées : le visage du jeu n'est pas une planche par combinaison, c'est un empilement.
	const couches: string[] = [];
	for (const c of catalogue.categories) {
		// À défaut de choix, la PREMIÈRE part de la rubrique. Sans ce repli, l'avatar de départ
		// arrivait sans yeux, sans bouche et sans sourcils : `choix` est vide tant que rien n'a
		// été sélectionné, alors que le jeu, lui, ouvre l'éditeur sur un visage complet.
		// Quelle part exacte le jeu retient au départ n'est pas établi — la première est un
		// défaut assumé, pas un relevé.
		const id = choix[c.faceSettingType];
		const part = id ? c.parts.find((p) => p.id === id) : c.parts[0];
		if (!part) continue;

		// `modeles` ET `modeles2` : une coiffure est en DEUX morceaux, l'avant (`_hairF`) et
		// l'arrière (`_hairB`). Sur les 98 coiffures, 45 n'ont QUE `modeles2` — ne lire que
		// `modeles` les rendait chauves — et les 53 autres perdaient leur nuque.
		const mailles = [...part.modeles, ...(part.modeles2 ?? [])].filter(
			(m) => m.includes("/20_EDIT/") && m.endsWith(".g4md"),
		);
		if (mailles.length > 0) {
			for (const maille of mailles) {
				const bouts = maille.split("/20_EDIT/")[1]?.split("/");
				const dossier = bouts?.[0];
				const nom = bouts?.[1]?.replace(/\.g4md$/, "");
				if (dossier && nom) pieces.push(`${dossier}/${nom}`);
			}
			continue;
		}

		const texture = part.modeles.find(
			(m) => m.includes("/_facetex/") && m.endsWith(".g4tx"),
		);
		if (texture) {
			const rel = texture.split("/_facetex/")[1]?.replace(/\.g4tx$/, "");
			if (rel) couches.push(rel);
		}
	}

	if (pieces.length === 0) return null;
	// Dédupliqué comme les couches : deux rubriques peuvent désigner la même maille, et le
	// serveur l'incorporerait deux fois, superposée à elle-même.
	const piecesUniques = [...new Set(pieces)];
	// Les familles sont numérotées dans leur ordre de superposition — `00_face` la peau, puis
	// `01_eye`, `02_pupil`, `03_highlight`, `04_eyebrow`, `05_mouth` : trier par nom donne
	// l'ordre d'empilement sans avoir à le décider.
	// Dédupliqué : deux rubriques distinctes peuvent désigner la même planche (les types 3 et
	// 13 pointent tous deux `00_face`), et chacune coûterait un décodage de 2048×1024.
	const uniques = [...new Set(couches)].sort();

	// La TEINTE. Le canal rouge des planches de `_facetex` porte la carnation : la couleur de
	// peau choisie y va. Les valeurs RGB des palettes ne vivent que dans la mémoire du jeu —
	// `niers mem palettes` les relève et les fusionne dans le catalogue sous `couleursRgb`.
	// À défaut de choix, la route retombe sur la couleur des recettes du jeu.
	//
	// Une couleur LIBRE prime sur la palette. Les 65 teintes de cheveux et les 49 d'yeux du jeu
	// sont un choix de jeu, pas une charte : une autrice ou un auteur arrive avec la couleur de
	// son personnage déjà fixée, et l'approximation la plus proche n'est pas la bonne couleur.
	// Elle se range dans `champs["couleur.libre.<type>"]` sous la forme de six chiffres
	// hexadécimaux, ce qui ne change rien au format d'échange : `champs` accepte déjà toute clé
	// en `[a-zA-Z0-9_.-]` et toute valeur texte, et un projet écrit sans elle se relit tel quel.
	const rgbDe = (type: number): string | null => {
		const libre = champs[`couleur.libre.${type}`];
		if (libre && /^[0-9A-Fa-f]{6}$/.test(libre)) return libre.toUpperCase();
		const cat = catalogue.categories.find((c) => c.faceSettingType === type);
		const i = valeurs[`couleur.${type}`];
		const id = cat?.couleurs?.[i ?? -1];
		return id ? (catalogue.couleursRgb?.[id]?.rgb ?? null) : null;
	};
	// Les TROIS canaux de teinte du visage : le rouge porte la carnation, le vert l'iris, le
	// bleu reste clair. La couleur d'œil est la catégorie 6, celle de peau la 3.
	const peau = rgbDe(3);
	const iris = rgbDe(6);
	const teinte =
		peau || iris ? `&tint=${peau ?? "F3CAC1"},${iris ?? "533B3B"},FFFFFF` : "";

	// La chevelure a sa propre couleur (catégorie 4, 98 coupes et 65 teintes). Sa planche est
	// NEUTRE dans les fichiers — `hair_10` vaut 255,255,255 partout — donc sans cette couleur
	// l'avatar porte un casque blanc. La route la multiplie sur la planche.
	const cheveux = rgbDe(4);
	const teinteCheveux = cheveux ? `&hair=${cheveux}` : "";

	// La MORPHOLOGIE désigne le corps exact. Le squelette seul ne suffit pas : il n'en réduit
	// le choix qu'à deux, et c'est la corpulence mesurée qui départage — `female` a les épaules
	// plus étroites et le tour de taille plus large que `male`, `big` un tour de taille de
	// 0,99 m quand `muscle`, aussi grand, garde 0,65. La table vit côté serveur.
	const morpho = nomMorpho ? `&morpho=${nomMorpho}` : "";

	// La TAILLE : quinze crans, que le jeu fait correspondre à une stature de 1,25 m à 2,08 m.
	// Le modèle est mis à l'échelle côté serveur.
	const cranTaille = valeurs["taille"];
	const taille = cranTaille === undefined ? "" : `&taille=${cranTaille}`;

	// La FORME DE VISAGE : ses six parts ne désignent aucune ressource dans le catalogue
	// (`resource = 0xFFFFFFFF`), le choix ne pouvait donc rien changer. La route l'applique en
	// déformant la tête ; on lui transmet le rang de la part choisie.
	const catForme = catalogue.categories.find((c) => c.faceSettingType === 2);
	const iForme = catForme?.parts.findIndex((p) => p.id === choix[2]) ?? -1;
	const forme = iForme >= 0 ? `&forme=${iForme}` : "";

	// Les HABITS — col (19), manches (20), ourlet (21). Leurs parts ne portent aucune maille ni
	// texture, rien qu'un nom de découpe (`fashion_collar`, `fashion_shoulder_baring`,
	// `fashion_shirt_out`, `fashion_navel_baring`) : la route ajuste la coupe du maillot.
	const rangDe = (type: number) => {
		const cat = catalogue.categories.find((c) => c.faceSettingType === type);
		const i = cat?.parts.findIndex((p) => p.id === choix[type]) ?? -1;
		return Math.max(0, i);
	};
	const habits = `&habits=${rangDe(19)},${rangDe(20)},${rangDe(21)}`;

	const q =
		uniques.length > 0
			? `?face=${encodeURIComponent(uniques.join(","))}${teinte}${teinteCheveux}${morpho}${taille}${forme}${habits}`
			: `${teinte}${teinteCheveux}${morpho}${taille}${forme}${habits}`.replace("&", "?");
	return `${cdn}/model-avatar/${piecesUniques.join("+")}.glb${q}`;
}
