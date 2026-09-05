"use client";

/**
 * Galerie VRoid Hub : parcours des modèles et sélection de celui à visionner.
 *
 * Toutes les requêtes passent par `/api/vroid/models` — jamais `hub.vroid.com`
 * en direct : la CSP d'azalée limite `connect-src` à `'self'`, et surtout le
 * jeton d'accès ne doit pas exister côté navigateur.
 *
 * La première page de la sélection éditoriale est rendue côté serveur et
 * passée en `pageInitiale` : la galerie s'affiche pleine dès le premier octet,
 * sans écran de chargement.
 */
import { useCallback, useEffect, useState } from "react";
import { estChargeable, nomModele } from "@/lib/vroid/licence";
import type { ModeleVroid, PageModeles, SourceModeles } from "@/lib/vroid/types";
import { ConditionsUtilisation } from "./ConditionsUtilisation";
import { urlVignette } from "./vignette";
import { VisionneuseVrm } from "./VisionneuseVrm";

/** Onglets proposés, avec leur libellé et le besoin d'un compte lié. */
const ONGLETS: { source: SourceModeles; libelle: string; connexionRequise: boolean }[] = [
	{ source: "staff_picks", libelle: "Sélection VRoid Hub", connexionRequise: false },
	{ source: "recherche", libelle: "Recherche", connexionRequise: false },
	{ source: "compte", libelle: "Mes modèles", connexionRequise: true },
	{ source: "coeurs", libelle: "Mes coups de cœur", connexionRequise: true },
];

export interface GalerieVroidProps {
	/** Première page de la sélection éditoriale, chargée côté serveur. */
	pageInitiale: PageModeles;
	/** Message d'erreur du chargement serveur, s'il a échoué. */
	erreurInitiale?: string | null;
	/** L'internaute a lié son compte VRoid Hub. */
	connecte: boolean;
}

export function GalerieVroid({ pageInitiale, erreurInitiale = null, connecte }: GalerieVroidProps) {
	const [source, setSource] = useState<SourceModeles>("staff_picks");
	const [motCle, setMotCle] = useState("");
	const [motCleActif, setMotCleActif] = useState("");
	const [telechargeablesSeulement, setTelechargeablesSeulement] = useState(false);

	const [modeles, setModeles] = useState<ModeleVroid[]>(pageInitiale.modeles);
	const [curseur, setCurseur] = useState<string | null>(pageInitiale.curseurSuivant);
	const [chargement, setChargement] = useState(false);
	const [erreur, setErreur] = useState<string | null>(erreurInitiale);
	const [selection, setSelection] = useState<ModeleVroid | null>(pageInitiale.modeles[0] ?? null);

	/**
	 * Charge une page depuis le relais serveur.
	 *
	 * @param suite `true` pour ajouter à la suite, `false` pour repartir de zéro.
	 */
	const charger = useCallback(
		async (suite: boolean, signal?: AbortSignal) => {
			if (source === "recherche" && motCleActif.trim().length === 0) {
				setModeles([]);
				setCurseur(null);
				return;
			}

			setChargement(true);
			setErreur(null);
			try {
				const parametres = new URLSearchParams({ source, nombre: "24" });
				if (source === "recherche") parametres.set("q", motCleActif.trim());
				if (telechargeablesSeulement) parametres.set("telechargeables", "1");
				if (suite && curseur) parametres.set("curseur", curseur);

				const reponse = await fetch(`/api/vroid/models?${parametres}`, { signal });
				const charge = (await reponse.json()) as Partial<PageModeles> & { erreur?: string };

				if (!reponse.ok) throw new Error(charge.erreur ?? `Erreur ${reponse.status}.`);

				const recus = charge.modeles ?? [];
				// Déduplication par identifiant : le tri par pertinence de la
				// recherche VRoid Hub n'est pas stable dans le temps, et un curseur
				// repris peut ramener un modèle déjà affiché (cf. `rechercherModeles`).
				setModeles((precedents) => {
					if (!suite) return recus;
					const vus = new Set(precedents.map((modele) => modele.id));
					return [...precedents, ...recus.filter((modele) => !vus.has(modele.id))];
				});
				setCurseur(charge.curseurSuivant ?? null);
				if (!suite) setSelection(recus[0] ?? null);
			} catch (cause) {
				if (signal?.aborted) return;
				setErreur(cause instanceof Error ? cause.message : "Chargement impossible.");
				if (!suite) {
					setModeles([]);
					setCurseur(null);
					setSelection(null);
				}
			} finally {
				if (!signal?.aborted) setChargement(false);
			}
		},
		[source, motCleActif, telechargeablesSeulement, curseur]
	);

	// Rechargement à chaque changement de source, de mot-clé validé ou de filtre.
	// `charger` n'est volontairement PAS dans les dépendances : il change à chaque
	// mise à jour du curseur, ce qui relancerait la première page en boucle.
	useEffect(() => {
		// La page initiale est déjà en place : ne pas la redemander au montage.
		if (source === "staff_picks" && motCleActif === "" && !telechargeablesSeulement) return;

		const abandon = new AbortController();
		void charger(false, abandon.signal);
		return () => abandon.abort();
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [source, motCleActif, telechargeablesSeulement]);

	return (
		<div className="space-y-5">
			<div className="flex flex-wrap items-center gap-2">
				{ONGLETS.map((onglet) => {
					const indisponible = onglet.connexionRequise && !connecte;
					return (
						<button
							className={`rounded-full px-4 py-1.5 text-sm font-medium transition ${
								source === onglet.source
									? "bg-primary text-on-primary"
									: "bg-surface-container text-on-surface-variant hover:bg-surface-container-high"
							} ${indisponible ? "cursor-not-allowed opacity-40" : ""}`}
							disabled={indisponible}
							key={onglet.source}
							onClick={() => setSource(onglet.source)}
							title={indisponible ? "Liez votre compte VRoid Hub pour y accéder." : undefined}
							type="button"
						>
							{onglet.libelle}
						</button>
					);
				})}
			</div>

			{source === "recherche" && (
				<form
					className="flex flex-wrap items-center gap-2"
					onSubmit={(evenement) => {
						evenement.preventDefault();
						setMotCleActif(motCle);
					}}
				>
					<input
						className="min-w-0 flex-1 rounded-full border border-outline-variant/40 bg-surface-container-low px-4 py-2 text-sm text-on-surface outline-none focus:border-primary"
						onChange={(evenement) => setMotCle(evenement.target.value)}
						placeholder="Chercher un modèle sur VRoid Hub…"
						type="search"
						value={motCle}
					/>
					<button
						className="rounded-full bg-primary px-4 py-2 text-sm font-semibold text-on-primary transition hover:opacity-90"
						type="submit"
					>
						Chercher
					</button>
				</form>
			)}

			<label className="flex items-center gap-2 text-sm text-on-surface-variant">
				<input
					checked={telechargeablesSeulement}
					className="size-4 accent-[color:var(--color-primary)]"
					onChange={(evenement) => setTelechargeablesSeulement(evenement.target.checked)}
					type="checkbox"
				/>
				N&apos;afficher que les modèles affichables en 3D (téléchargement autorisé par l&apos;auteur)
			</label>

			{erreur && (
				<p className="rounded-2xl border border-outline-variant/40 bg-surface-container-low p-4 text-sm text-on-surface-variant">
					{erreur}
				</p>
			)}

			<div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_22rem]">
				<div className="space-y-4">
					{modeles.length === 0 && !chargement && !erreur ? (
						<p className="rounded-2xl border border-outline-variant/40 bg-surface-container-low p-6 text-sm text-on-surface-variant">
							{source === "recherche" && motCleActif.trim().length === 0
								? "Saisissez un mot-clé pour interroger VRoid Hub."
								: "Aucun modèle à afficher."}
						</p>
					) : (
						<ul className="grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-4">
							{modeles.map((modele) => (
								<CarteModele
									actif={selection?.id === modele.id}
									key={modele.id}
									modele={modele}
									onSelection={() => setSelection(modele)}
								/>
							))}
						</ul>
					)}

					{curseur && (
						<button
							className="w-full rounded-full bg-surface-container px-4 py-2.5 text-sm font-medium text-on-surface transition hover:bg-surface-container-high disabled:opacity-50"
							disabled={chargement}
							onClick={() => void charger(true)}
							type="button"
						>
							{chargement ? "Chargement…" : "Charger la suite"}
						</button>
					)}
				</div>

				<aside className="space-y-4 lg:sticky lg:top-4 lg:self-start">
					{selection ? (
						<>
							<VisionneuseVrm connecte={connecte} modele={selection} />
							<FicheModele modele={selection} />
							<ConditionsUtilisation compact modele={selection} />
						</>
					) : (
						<p className="rounded-2xl border border-outline-variant/40 bg-surface-container-low p-6 text-sm text-on-surface-variant">
							Choisissez un modèle pour l&apos;examiner.
						</p>
					)}
				</aside>
			</div>
		</div>
	);
}

/** Vignette cliquable d'un modèle dans la grille. */
function CarteModele({
	modele,
	actif,
	onSelection,
}: {
	modele: ModeleVroid;
	actif: boolean;
	onSelection: () => void;
}) {
	const vignette = urlVignette(modele.portrait_image?.sq300?.url ?? modele.portrait_image?.original?.url);
	const nom = nomModele(modele);

	return (
		<li>
			<button
				className={`group w-full overflow-hidden rounded-2xl border text-left transition ${
					actif
						? "border-primary bg-surface-container"
						: "border-outline-variant/40 bg-surface-container-low hover:border-primary/50 hover:bg-surface-container"
				}`}
				onClick={onSelection}
				type="button"
			>
				<div className="relative aspect-square w-full bg-surface-container-high">
					{vignette && (
						// eslint-disable-next-line @next/next/no-img-element -- image relayée par /api/vroid/image, hors optimiseur Next
						<img
							alt={`Portrait du modèle ${nom}`}
							className="size-full object-cover"
							decoding="async"
							loading="lazy"
							src={vignette}
						/>
					)}
					{estChargeable(modele) && (
						<span className="absolute left-2 top-2 rounded-full bg-primary px-2 py-0.5 text-xs font-semibold text-on-primary">
							3D
						</span>
					)}
				</div>
				<div className="space-y-0.5 p-2.5">
					<p className="truncate text-sm font-medium text-on-surface">{nom}</p>
					<p className="truncate text-xs text-on-surface-variant">{modele.character?.user?.name}</p>
					<p className="text-xs text-on-surface-variant">
						{modele.heart_count.toLocaleString("fr")} ❤ · {modele.view_count.toLocaleString("fr")} vues
					</p>
				</div>
			</button>
		</li>
	);
}

/** Attribution de l'auteur et lien vers la fiche d'origine. */
function FicheModele({ modele }: { modele: ModeleVroid }) {
	const version = modele.latest_character_model_version;

	return (
		<div className="space-y-2 rounded-2xl border border-outline-variant/40 bg-surface-container-low p-4">
			<h2 className="text-base font-semibold text-on-surface">{nomModele(modele)}</h2>
			<p className="text-sm text-on-surface-variant">
				Par <span className="font-medium text-on-surface">{modele.character?.user?.name}</span> — le
				crédit de l&apos;auteur suit le modèle.
			</p>

			{version && (
				<p className="text-xs text-on-surface-variant">
					{version.triangle_count.toLocaleString("fr")} triangles ·{" "}
					{version.material_count.toLocaleString("fr")} matériaux ·{" "}
					{version.joint_count.toLocaleString("fr")} os
				</p>
			)}

			<a
				className="inline-block text-sm font-medium text-primary hover:underline"
				href={`https://hub.vroid.com/characters/${modele.character?.id}/models/${modele.id}`}
				rel="noreferrer noopener"
				target="_blank"
			>
				Voir la fiche sur VRoid Hub ↗
			</a>
		</div>
	);
}
