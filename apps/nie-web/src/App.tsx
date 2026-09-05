import { creerWebSource, type VueCatalogue } from "@niers/asset-source";
import { type SanteApi, sante } from "@niers/asset-source/nie-site";
import {
	AssetSourceProvider,
	Badge,
	Callout,
	HeaderBanner,
	SidePanel,
	SkewTile,
	TileRow,
	TitleBand,
	useCapacites,
	useErreurSource,
	VersionChip,
} from "@niers/inacord-ui";
import "@niers/inacord-ui/shell/game-tokens.css";
import { useEffect, useMemo, useState } from "react";
import { Catalogue } from "./pages/Catalogue";
import { Explorateur } from "./pages/Explorateur";
import { cheminPourEntree, entreeDemandee, separerLangue } from "./routage";

/**
 * Coquille d'Aphrody.
 *
 * L'hôte n'a qu'un rôle : construire sa source et la monter. Tout le reste vient de
 * `@niers/inacord-ui` — la même interface que celle d'Inacord, dans la DA du menu principal du
 * jeu. C'est le point de la manœuvre.
 */
export function App() {
	// La source ne dépend d'aucun état : la mémoriser évite de relancer la mesure des capacités
	// à chaque rendu.
	const source = useMemo(() => creerWebSource(), []);
	return (
		<AssetSourceProvider source={source}>
			<Accueil />
		</AssetSourceProvider>
	);
}

/** Les quatre filtres enregistrés, dans l'ordre où `nie-site` les publie. */
const VUES = ["textures", "modeles", "sons", "videos"] as const;

/** L'explorateur n'est pas un filtre : il montre la STRUCTURE, pas une selection. */
const EXPLORATEUR = "explorateur";

/** Les entrees du menu. Annotee, sinon le spread elargit le type a `string[]`. */
const ENTREES: readonly (VueCatalogue | typeof EXPLORATEUR)[] = [...VUES, EXPLORATEUR];

/**
 * Ce que le serveur déclare savoir servir, ici et maintenant.
 *
 * L'index du VFS se monte en tâche de fond : l'interface distingue « on ne sait pas encore » de
 * « rien ne marche », au lieu d'afficher des vues vides pendant la première seconde.
 */
function Accueil() {
	const capacites = useCapacites();
	const erreur = useErreurSource();
	const [etat, setEtat] = useState<SanteApi | null>(null);
	// L'entree courante vit dans l'URL, pas seulement en memoire.
	//
	// Sans cela, un lien vers un catalogue ne mene qu'a l'accueil, le bouton « precedent » du
	// navigateur quitte le site au lieu de revenir a la vue precedente, et un rechargement perd
	// ou l'on etait. Une interface qui ne se laisse pas mettre en signet oblige a refaire le
	// chemin a chaque visite.
	// Le prefixe de langue de l'URL courante. Il ne change pas pendant la session : changer de
	// langue est une navigation entiere, servie par nie-site, pas un changement d'etat local.
	const prefixe = useMemo(() => separerLangue(window.location.pathname).prefixe, []);

	const [vue, setVueEtat] = useState<VueCatalogue | typeof EXPLORATEUR>(() => {
		const routeServeur = document.getElementById("racine")?.dataset.route;
		const demandee = entreeDemandee(ENTREES as readonly string[], window.location, routeServeur);
		return (demandee as VueCatalogue | typeof EXPLORATEUR | null) ?? VUES[0];
	});

	// Une ancienne URL `?vue=` doit continuer a fonctionner, mais pas a subsister : elle est
	// reecrite vers la forme canonique en `replaceState`, donc sans ajouter d'entree
	// d'historique — sinon le bouton « precedent » ramenerait a la meme page.
	useEffect(() => {
		if (!new URLSearchParams(window.location.search).get("vue")) {
			return;
		}
		const url = new URL(window.location.href);
		url.searchParams.delete("vue");
		url.pathname = cheminPourEntree(prefixe, vue);
		window.history.replaceState({ vue }, "", url);
	}, [prefixe, vue]);

	/** Change de vue ET d'URL, sans recharger la page. */
	const setVue = (suivante: VueCatalogue | typeof EXPLORATEUR) => {
		setVueEtat(suivante);
		const url = new URL(window.location.href);
		url.pathname = cheminPourEntree(prefixe, suivante);
		url.searchParams.delete("vue");
		window.history.pushState({ vue: suivante }, "", url);
	};

	// Le bouton « precedent » doit ramener a la vue precedente, pas sortir du site.
	useEffect(() => {
		const surRetour = () => {
			const demandee = entreeDemandee(ENTREES as readonly string[], window.location);
			if (demandee) {
				setVueEtat(demandee as VueCatalogue | typeof EXPLORATEUR);
			}
		};
		window.addEventListener("popstate", surRetour);
		return () => window.removeEventListener("popstate", surRetour);
	}, []);

	useEffect(() => {
		const ac = new AbortController();
		sante(ac.signal)
			.then(setEtat)
			.catch(() => {
				/* l'erreur est déjà portée par le fournisseur */
			});
		return () => ac.abort();
	}, []);

	const totaux = new Map(etat?.vues.map((v) => [v.nom, v.total]) ?? []);

	return (
		<div
			style={{
				minHeight: "100vh",
				display: "flex",
				flexDirection: "column",
				background: "var(--jeu-fond-abysse)",
				color: "var(--jeu-texte-vif)",
				fontFamily: "system-ui, sans-serif",
			}}
		>
			<HeaderBanner
				titre="Aphrody"
				actions={etat ? <VersionChip version={`${etat.service} ${etat.version || "—"}`} /> : null}
			/>

			<div style={{ display: "flex", flex: 1, minHeight: 0 }}>
				<SidePanel>
					<TitleBand>Catalogues</TitleBand>
					<div style={{ marginTop: "var(--jeu-espace-m)" }}>
						<TileRow>
							{ENTREES.map((nom) => {
								const total = totaux.get(nom);
								return (
									<SkewTile
										key={nom}
										actif={nom === vue}
										// Tant que l'index n'est pas prêt, la tuile est en sourdine : elle
										// ne promet pas un contenu qu'elle ne peut pas encore montrer.
										sourdine={!capacites?.vfs}
										onClick={() => setVue(nom)}
									>
										<span style={{ display: "flex", alignItems: "center", gap: 8 }}>
											<span style={{ flex: 1, textTransform: "capitalize" }}>{nom}</span>
											{typeof total === "number" ? (
												<Badge>{total.toLocaleString("fr")}</Badge>
											) : null}
										</span>
									</SkewTile>
								);
							})}
						</TileRow>
					</div>
				</SidePanel>

				<main style={{ flex: 1, padding: "var(--jeu-espace-xl)", overflowY: "auto" }}>
					{erreur ? (
						<Callout ton="alerte">nie-site injoignable : {erreur}</Callout>
					) : !capacites ? (
						<Callout>Mesure des capacités…</Callout>
					) : !capacites.vfs ? (
						<Callout>
							L'index du VFS n'est pas encore monté. Les catalogues apparaîtront dès qu'il sera
							prêt.
						</Callout>
					) : (
						<>
							{vue === EXPLORATEUR ? <Explorateur /> : <Catalogue vue={vue} />}
						</>
					)}
				</main>
			</div>
		</div>
	);
}
