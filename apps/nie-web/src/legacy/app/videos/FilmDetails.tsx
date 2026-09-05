/**
 * Le panneau technique d'une cinématique — tout ce que le catalogue mesure et que la page
 * n'affichait pas.
 *
 * `nie_explore::cinema` publie une trentaine de champs par film : le compte d'images réel contre
 * celui que l'en-tête annonce, les octets vidéo, le conteneur web produit par le remux et ce
 * qu'il économise, les images-clés, la bande-son résolue dans `anime_stream`, et la ligne de
 * `movie_playing_config` qui déclenche le film. Aucun n'était référencé nulle part dans le site —
 * le catalogue était lu, puis les trois quarts jetés.
 *
 * Deux règles tenues ici, parce que les afficher naïvement ferait dire au site des choses fausses :
 *
 * 1. **Les sentinelles du gamedata ne sont pas des valeurs.** Les tables du jeu écrivent
 *    `0x00000000` (identifiant nul) et `0xFFFFFFFF` (chaîne absente) pour dire « rien ». Les
 *    rendre tels quels afficherait un `captionId` sur 97 films dont aucun n'a de légende.
 * 2. **L'écart d'images n'est pas nécessairement un manque.** Sur les 18 écrans-titres MPEG-2, le
 *    conteneur porte exactement le DOUBLE de ce que l'en-tête annonce (130 contre 65). On énonce
 *    donc l'écart et son sens, sans en proposer la cause : rien dans le catalogue ne l'atteste.
 */

import type { FilmDto } from "@rosegriffon/azalee/cpk/video";
import { formatDuree } from "@rosegriffon/azalee/cpk/video";

/** Ce que les tables du jeu écrivent pour dire « aucune valeur ». */
const SENTINELLES = new Set(["0X00000000", "0XFFFFFFFF"]);

/** La valeur d'un champ de gamedata, ou `null` quand c'est une sentinelle. */
function valeurJeu(valeur: string | undefined): string | null {
	if (valeur == null || valeur === "") return null;
	return SENTINELLES.has(valeur.toUpperCase()) ? null : valeur;
}

/**
 * Taille lisible jusqu'au kibioctet.
 *
 * `formatOctets` du client arrondit au mébioctet : une piste de 147 040 octets y devient
 * « 0 Mio ». Ce panneau descend sous le mégaoctet, il lui faut sa propre échelle.
 */
function octets(n: number): string {
	if (n < 1024) return `${n.toLocaleString("fr")} o`;
	const kio = n / 1024;
	if (kio < 1024) return `${Math.round(kio).toLocaleString("fr")} Kio`;
	const mio = kio / 1024;
	return mio < 1024
		? `${mio.toFixed(1).replace(".", ",")} Mio`
		: `${(mio / 1024).toFixed(2).replace(".", ",")} Gio`;
}

/** Un entier, groupé à la française. */
function entier(n: number): string {
	return n.toLocaleString("fr");
}

/** Une durée en millisecondes, avec sa forme `m:ss` quand elle en a une. */
function millisecondes(ms: number): string {
	const lisible = formatDuree(ms / 1000);
	return lisible ? `${entier(ms)} ms (${lisible})` : `${entier(ms)} ms`;
}

/** Une ligne du panneau : intitulé à gauche, valeur mesurée à droite. */
function Ligne({
	intitule,
	aide,
	children,
}: {
	intitule: string;
	aide?: string;
	children: React.ReactNode;
}) {
	return (
		<div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-0.5 border-b border-outline-variant/15 py-1 last:border-b-0">
			<dt className="text-[11px] uppercase tracking-wide text-on-surface-variant/80" title={aide}>
				{intitule}
			</dt>
			<dd className="min-w-0 break-all text-right font-mono text-[11px] text-on-surface">
				{children}
			</dd>
		</div>
	);
}

/** Un groupe de lignes sous son titre. Rien n'est rendu si le groupe est vide. */
function Groupe({ titre, children }: { titre: string; children: React.ReactNode }) {
	return (
		<section className="min-w-0">
			<h3 className="mb-1 text-[11px] font-bold uppercase tracking-wider text-primary">{titre}</h3>
			<dl className="min-w-0">{children}</dl>
		</section>
	);
}

/** Le compte d'images, et l'écart avec ce que l'en-tête annonce quand il y en a un. */
function Images({ film }: { film: FilmDto }) {
	const ecart = film.images - film.totalImagesDeclare;
	if (ecart === 0) return <>{entier(film.images)}</>;
	return (
		<>
			{entier(film.images)} présentes / {entier(film.totalImagesDeclare)} annoncées
			<span className="ml-1 text-on-surface-variant" title={
				ecart > 0
					? "Le conteneur porte plus d'images que son en-tête n'en déclare."
					: "Des images déclarées par l'en-tête manquent au conteneur."
			}>
				({ecart > 0 ? "+" : "−"}
				{entier(Math.abs(ecart))})
			</span>
		</>
	);
}

/**
 * Le panneau technique, replié par défaut.
 *
 * `<details>`/`<summary>` natifs : le clavier, le lecteur d'écran et la recherche du navigateur
 * les gèrent sans une ligne de JavaScript, et l'état de repli survit à un changement de film
 * parce que l'élément n'est pas démonté entre deux.
 */
export function FilmDetails({ film }: { film: FilmDto }) {
	const jeu = film.gamedata;
	const economie =
		film.conteneurOctets != null && film.conteneurOctets < film.octets
			? film.octets - film.conteneurOctets
			: null;

	// Les champs de gamedata réellement renseignés — les sentinelles sont écartées ici, une fois,
	// pour que le groupe entier disparaisse quand la ligne du jeu ne dit rien.
	const champsJeu: { intitule: string; valeur: string; aide?: string }[] = [];
	if (jeu) {
		const ajoute = (intitule: string, valeur: string | undefined, aide?: string) => {
			const v = valeurJeu(valeur);
			if (v != null) champsJeu.push({ aide, intitule, valeur: v });
		};
		ajoute("movieId", jeu.movieId, "Identifiant du film tel que le jeu le hache");
		ajoute("eventId", jeu.eventId, "Événement d'histoire qui déclenche le film");
		ajoute("menuId", jeu.menuId, "Menu depuis lequel le film est joué");
		ajoute("captionId", jeu.captionId, "Identifiant de la légende associée");
		ajoute(
			"bgmName",
			jeu.bgmName,
			"« Nom de musique » du gamedata — en réalité le CRC32 du nom du film",
		);
		ajoute("staffroll", jeu.staffrollDataName, "Générique joué par-dessus le film");
		ajoute(
			"texte des sous-titres",
			jeu.subtitleTextPath,
			"Chemin VFS ; <LG> est substitué par la langue à l'exécution",
		);
		ajoute(
			"réglages des sous-titres",
			jeu.subtitleSettingPath,
			"Chemin VFS ; <VLG> est substitué par la langue à l'exécution",
		);
	}

	return (
		<details className="group rounded-2xl border border-outline-variant/30 bg-surface-container-low">
			<summary className="cursor-pointer list-none px-4 py-2.5 text-xs font-semibold text-on-surface-variant transition-colors hover:text-on-surface">
				<span className="inline-block w-4 transition-transform group-open:rotate-90">›</span>
				Détails techniques
				<span className="ml-2 font-normal text-on-surface-variant/70">
					conteneur, remux, bande-son, données du jeu
				</span>
			</summary>

			<div className="grid gap-x-8 gap-y-4 px-4 pb-4 sm:grid-cols-2">
				<Groupe titre="Conteneur">
					<Ligne intitule="chemin VFS">{film.chemin}</Ligne>
					<Ligne intitule="codec">{film.codec}</Ligne>
					<Ligne
						intitule="images"
						aide="Images présentes dans le conteneur, et total annoncé par l'en-tête"
					>
						<Images film={film} />
					</Ligne>
					{film.cadence != null && film.cadence > 0 && (
						<Ligne intitule="cadence">
							{film.cadence.toFixed(3).replace(".", ",")} i/s
						</Ligne>
					)}
					<Ligne intitule="poids du .usm">{octets(film.octets)}</Ligne>
					<Ligne
						intitule="octets vidéo"
						aide="Total des octets vidéo, hors en-têtes de bloc et bourrage"
					>
						{octets(film.octetsVideo)}
						<span className="ml-1 text-on-surface-variant">
							({((film.octetsVideo / film.octets) * 100).toFixed(1).replace(".", ",")} %)
						</span>
					</Ligne>
					{film.dechiffre && (
						<Ligne
							intitule="chiffrement"
							aide="Le conteneur portait l'enveloppe CRI ; il a fallu la déchiffrer pour le lire"
						>
							enveloppe CRI déchiffrée
						</Ligne>
					)}
					{film.sousTitres != null && film.sousTitres > 0 && (
						<Ligne intitule="blocs de sous-titres">{entier(film.sousTitres)}</Ligne>
					)}
					{film.nomOrigine && (
						<Ligne
							intitule="nom d'origine"
							aide="Nom inscrit par l'encodeur. Les chemins japonais sont rendus tels quels : le conteneur ne déclare pas leur encodage."
						>
							{film.nomOrigine}
						</Ligne>
					)}
					{film.erreur && (
						<Ligne intitule="erreur de lecture">
							<span className="text-error">{film.erreur}</span>
						</Ligne>
					)}
				</Groupe>

				<Groupe titre="Remux web">
					{film.remuxImpossible ? (
						<Ligne
							intitule="impossible"
							aide="Aucun conteneur web ne peut porter ce flux tel quel"
						>
							<span className="text-error">{film.remuxImpossible}</span>
						</Ligne>
					) : (
						<>
							{film.conteneur && <Ligne intitule="conteneur">{film.conteneur}</Ligne>}
							{film.conteneurOctets != null && (
								<Ligne
									intitule="poids remuxé"
									aide="Taille du conteneur web produit — remux sans réencodage"
								>
									{octets(film.conteneurOctets)}
								</Ligne>
							)}
							{film.gainRemux != null && (
								<Ligne
									intitule="gain"
									aide="Part du fichier économisée en remplaçant l'emballage Sofdec2 par un conteneur web"
								>
									{film.gainRemux.toFixed(2).replace(".", ",")} %
									{economie != null && (
										<span className="ml-1 text-on-surface-variant">(−{octets(economie)})</span>
									)}
								</Ligne>
							)}
							{film.cles != null && (
								<Ligne
									intitule="images-clés"
									aide="Les seuls points sur lesquels un lecteur peut se repositionner"
								>
									{entier(film.cles)}
								</Ligne>
							)}
						</>
					)}
				</Groupe>

				{film.audio.length > 0 && (
					<Groupe titre="Pistes du conteneur">
						{film.audio.map((piste) => (
							<Ligne key={piste.canal} intitule={`canal ${piste.canal}`}>
								{piste.codec} · {entier(piste.frequence)} Hz · {piste.canaux} canaux ·{" "}
								{octets(piste.octets)}
							</Ligne>
						))}
					</Groupe>
				)}

				{film.bandeSon && (
					<Groupe titre="Bande-son externe">
						<Ligne intitule="cue" aide="Nom de la cue dans anime_stream">
							{film.bandeSon.cue}
						</Ligne>
						<Ligne intitule="id AWB" aide="Identifiant AFS2 de la forme d'onde">
							{film.bandeSon.awbId}
						</Ligne>
						<Ligne intitule="format">
							{film.bandeSon.codec} · {entier(film.bandeSon.frequence)} Hz ·{" "}
							{film.bandeSon.canaux} canaux
						</Ligne>
						<Ligne intitule="durée jouée" aide="Ce que le jeu joue de la cue">
							{millisecondes(film.bandeSon.dureeMs)}
						</Ligne>
						<Ligne intitule="durée du fichier" aide="Ce que la forme d'onde contient">
							{millisecondes(film.bandeSon.dureeOndeMs)}
							{film.bandeSon.dureeOndeMs !== film.bandeSon.dureeMs && (
								<span className="ml-1 text-on-surface-variant">
									(+{entier(film.bandeSon.dureeOndeMs - film.bandeSon.dureeMs)} ms)
								</span>
							)}
						</Ligne>
						<Ligne
							intitule="appariement"
							aide="Vrai quand le bgmName du gamedata confirme la cue trouvée par son nom"
						>
							{film.bandeSon.confirmeParHash
								? "confirmé par le gamedata"
								: "par le nom seul, non confirmé"}
						</Ligne>
					</Groupe>
				)}

				{jeu && (
					<Groupe titre="Données du jeu">
						<Ligne intitule="source" aide="Fichier de jeu d'où vient la ligne">
							{jeu.source}
						</Ligne>
						{champsJeu.map((c) => (
							<Ligne key={c.intitule} intitule={c.intitule} aide={c.aide}>
								{c.valeur}
							</Ligne>
						))}
						{jeu.fedeInTime != null && (
							<Ligne intitule="fondu d'entrée">
								{jeu.fedeInTime.toFixed(1).replace(".", ",")} s
							</Ligne>
						)}
						{jeu.fedeOutTime != null && (
							<Ligne intitule="fondu de sortie">
								{jeu.fedeOutTime.toFixed(1).replace(".", ",")} s
							</Ligne>
						)}
						<p className="mt-1.5 text-[10px] leading-snug text-on-surface-variant/70">
							Les champs valant <code>0x00000000</code> ou <code>0xFFFFFFFF</code> — ce que le jeu
							écrit pour « aucun » — ne sont pas listés.
						</p>
					</Groupe>
				)}
			</div>
		</details>
	);
}
