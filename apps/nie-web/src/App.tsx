import { creerWebSource } from "@niers/asset-source";
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
	const [vue, setVue] = useState<string>(VUES[0]);

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
							{VUES.map((nom) => {
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
							<TitleBand>{vue}</TitleBand>
							<dl style={{ marginTop: "var(--jeu-espace-m)", color: "var(--jeu-surface-craie)" }}>
								<dt>Entrées indexées</dt>
								<dd>{etat?.capacites.vfs_entrees.toLocaleString("fr") ?? "—"}</dd>
								<dt>Gisement</dt>
								<dd>{capacites.wiki ? "ouvert" : "absent"}</dd>
							</dl>
						</>
					)}
				</main>
			</div>
		</div>
	);
}
