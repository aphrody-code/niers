"use client";

/**
 * Îlot client de la galerie sonore : filtre les banques, ouvre l'une d'elles, joue ses cues.
 *
 * Le catalogue d'une banque n'est chargé qu'à son ouverture (`/audio-info`, ~13 ms pour 1 512
 * cues) puis mémorisé : rouvrir une banque déjà vue ne recoûte rien. Chaque cue est joué par
 * `/audio?id=<awbId>` — le décodage HCA/ADX se fait côté serveur, le navigateur ne reçoit qu'un
 * WAV. On adresse par `awbId` et non par rang : le rang dépend de l'ordre du fichier.
 */

import { useCallback, useMemo, useState } from "react";
import Link from "next/link";
import { AudioPlayer } from "@/app/sons/AudioPlayer";
import {
	audioBankKindLabel,
	fetchAudioBankOrNull,
	formatDuration,
	type AudioBank,
	type AudioBankKind,
} from "@rosegriffon/azalee/cpk/audio";

/** Une banque telle que la page serveur la transmet. */
export interface BankEntry {
	/** Chemin VFS de l'ACB. */
	path: string;
	/** Nom de fichier sans extension. */
	name: string;
	/** Taille de l'ACB en octets. */
	size: number;
	/** Famille déduite de l'emplacement et du nom. */
	kind: AudioBankKind;
	/** Langue (`ja`/`en`) pour les banques de voix, `null` sinon. */
	lang: string | null;
	/** Personnage sonorisé, `null` si non apparié. */
	owner: string | null;
	/** Slug de sa fiche, pour lier. */
	ownerSlug: string | null;
}

const FAMILLES: AudioBankKind[] = ["voix", "bgm", "technique", "effet", "systeme", "autre"];

/** Langues telles que les dossiers du jeu les nomment. */
const LANGUES: Record<string, string> = { ja: "japonais", en: "anglais" };

/**
 * Ce qu'une banque EST, quand on ne sait pas à qui elle appartient.
 *
 * 3 378 des 5 403 banques portent un code absent du miroir. Afficher ce code comme titre ne
 * renseigne personne : mieux vaut dire « Voix — japonais » et reléguer le code en dessous. On
 * ne devine pas de porteur, on décrit ce qui est attesté par l'emplacement du fichier.
 */
function contexte(b: BankEntry): string {
	const famille = audioBankKindLabel(b.kind);
	const langue = b.lang ? LANGUES[b.lang] : null;
	return langue ? `${famille} — ${langue}` : famille;
}

export function SoundGallery({ banks, namedCount }: { banks: BankEntry[]; namedCount: number }) {
	const [famille, setFamille] = useState<AudioBankKind | "all">("all");
	const [recherche, setRecherche] = useState("");
	const [ouverte, setOuverte] = useState<string | null>(null);
	const [catalogues, setCatalogues] = useState<Record<string, AudioBank | null>>({});
	const [chargement, setChargement] = useState<string | null>(null);
	// Ce qui joue : la banque et le RANG du cue dans son catalogue. Le rang permet de passer au
	// suivant sans rechercher la piste dans la liste.
	const [piste, setPiste] = useState<{ path: string; index: number } | null>(null);

	const comptes = useMemo(() => {
		const c = new Map<AudioBankKind, number>();
		for (const b of banks) c.set(b.kind, (c.get(b.kind) ?? 0) + 1);
		return c;
	}, [banks]);

	const visibles = useMemo(() => {
		const q = recherche.trim().toLowerCase();
		return banks.filter((b) => {
			if (famille !== "all" && b.kind !== famille) return false;
			if (!q) return true;
			// La recherche porte sur le nom du personnage autant que sur le code de fichier :
			// « Mark » doit trouver sa banque de voix sans qu'on ait à connaître son code.
			return b.name.toLowerCase().includes(q) || (b.owner?.toLowerCase().includes(q) ?? false);
		});
	}, [banks, famille, recherche]);

	const ouvrir = useCallback(
		async (path: string) => {
			if (ouverte === path) {
				setOuverte(null);
				return;
			}
			setOuverte(path);
			if (catalogues[path] !== undefined) return;
			setChargement(path);
			const bank = await fetchAudioBankOrNull(path);
			setCatalogues((c) => ({ ...c, [path]: bank }));
			setChargement(null);
		},
		[ouverte, catalogues],
	);

	// La piste transmise au lecteur, reconstruite depuis le catalogue déjà chargé — aucune
	// requête supplémentaire pour connaître son nom ou sa durée.
	const pisteCourante = useMemo(() => {
		if (!piste) return null;
		const cat = catalogues[piste.path];
		const cue = cat?.cues[piste.index];
		if (!cat || !cue) return null;
		const banque = banks.find((b) => b.path === piste.path);
		return {
			path: piste.path,
			awbId: cue.awbId,
			label: cue.name ?? `cue ${cue.index}`,
			banque: banque?.owner ?? cat.name ?? piste.path,
			duree: cue.durationSec,
		};
	}, [piste, catalogues, banks]);

	/** Passe au cue précédent (`-1`) ou suivant (`+1`), borné aux extrémités du catalogue. */
	const naviguer = useCallback(
		(delta: number) => {
			setPiste((p) => {
				if (!p) return p;
				const total = catalogues[p.path]?.cues.length ?? 0;
				const suivant = p.index + delta;
				return suivant >= 0 && suivant < total ? { ...p, index: suivant } : p;
			});
		},
		[catalogues],
	);

	return (
		<div className="space-y-4">

			<div className="flex flex-wrap items-center gap-2">
				<button
					type="button"
					onClick={() => setFamille("all")}
					className={`rounded-full px-3 py-1.5 text-xs font-medium transition ${
						famille === "all"
							? "bg-primary text-on-primary"
							: "bg-surface-container text-on-surface-variant hover:bg-surface-container-high"
					}`}
				>
					Toutes ({banks.length.toLocaleString("fr")})
				</button>
				{FAMILLES.filter((f) => comptes.get(f)).map((f) => (
					<button
						key={f}
						type="button"
						onClick={() => setFamille(f)}
						className={`rounded-full px-3 py-1.5 text-xs font-medium transition ${
							famille === f
								? "bg-primary text-on-primary"
								: "bg-surface-container text-on-surface-variant hover:bg-surface-container-high"
						}`}
					>
						{audioBankKindLabel(f)} ({(comptes.get(f) ?? 0).toLocaleString("fr")})
					</button>
				))}
			</div>

			<input
				type="search"
				value={recherche}
				onChange={(e) => setRecherche(e.target.value)}
				placeholder="Rechercher un personnage ou un code de banque…"
				className="w-full rounded-2xl border border-outline-variant bg-surface-container-low px-4 py-2.5 text-sm text-on-surface placeholder:text-on-surface-variant focus:border-primary focus:outline-none"
			/>

			<p className="px-1 text-xs text-on-surface-variant">
				{visibles.length.toLocaleString("fr")} banque{visibles.length > 1 ? "s" : ""} affichée
				{visibles.length > 1 ? "s" : ""} — {namedCount.toLocaleString("fr")} rattachée
				{namedCount > 1 ? "s" : ""} à un personnage. Les autres portent un code absent du miroir :
				elles restent jouables, sans nom.
			</p>

			<ul className="space-y-1.5">
				{visibles.slice(0, 400).map((b) => {
					const est_ouverte = ouverte === b.path;
					const cat = catalogues[b.path];
					return (
						<li
							key={b.path}
							className="overflow-hidden rounded-2xl border border-outline-variant bg-surface-container-low"
						>
							<button
								type="button"
								onClick={() => void ouvrir(b.path)}
								className="flex w-full items-center gap-3 px-4 py-3 text-left transition hover:bg-surface-container"
							>
								<span className="rounded-full bg-surface-container-high px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-on-surface-variant">
									{audioBankKindLabel(b.kind)}
								</span>
								{/* Le code de banque n'est pas un titre. À défaut d'un personnage identifié,
								    on affiche ce que le fichier EST — sa famille et sa langue — et le code
								    passe en dessous, en petit. */}
								<span className="min-w-0 flex-1">
									<span className="block truncate text-sm font-medium text-on-surface">
										{b.owner ?? contexte(b)}
									</span>
									<span className="block truncate text-[11px] text-on-surface-variant">
										{b.lang ? `${LANGUES[b.lang] ?? b.lang} · ` : ""}
										<span className="font-mono">{b.name}</span>
									</span>
								</span>
								<span className="shrink-0 text-xs text-on-surface-variant">
									{est_ouverte ? "▾" : "▸"}
								</span>
							</button>

							{est_ouverte && (
								<div className="border-t border-outline-variant px-4 py-3">
									{chargement === b.path && (
										<p className="text-xs italic text-on-surface-variant">
											Lecture du catalogue…
										</p>
									)}
									{cat === null && (
										<p className="text-xs italic text-on-surface-variant">
											Catalogue illisible pour cette banque.
										</p>
									)}
									{cat && (
										<>
											<div className="mb-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-on-surface-variant">
												<span>{cat.cueCount.toLocaleString("fr")} cues</span>
												{b.ownerSlug && (
													<Link
														href={`/chara/${b.ownerSlug}`}
														className="text-primary hover:underline"
													>
														Fiche du personnage
													</Link>
												)}
											</div>
											<ul className="max-h-80 space-y-0.5 overflow-y-auto">
												{cat.cues.map((c, rang) => {
													const cle = `${b.path}#${c.index}`;
													const actif = piste?.path === b.path && piste.index === rang;
													return (
														<li key={cle}>
															<button
																type="button"
																onClick={() => setPiste({ path: b.path, index: rang })}
																className={`flex w-full items-center gap-3 rounded-lg px-2 py-1.5 text-left text-xs transition hover:bg-surface-container ${
																	actif ? "bg-surface-container-high" : ""
																}`}
															>
																<span className="shrink-0 text-on-surface-variant">
																	{actif ? "◆" : "▶"}
																</span>
																<span className="min-w-0 flex-1 truncate font-mono text-on-surface">
																	{c.name ?? `cue ${c.index}`}
																</span>
																<span className="shrink-0 tabular-nums text-on-surface-variant">
																	{formatDuration(c.durationSec)}
																</span>
																{c.looped && (
																	<span
																		className="shrink-0 text-on-surface-variant"
																		title="Boucle"
																	>
																		↻
																	</span>
																)}
															</button>
														</li>
													);
												})}
											</ul>
										</>
									)}
								</div>
							)}
						</li>
					);
				})}
			</ul>

			{visibles.length > 400 && (
				<p className="px-1 text-xs italic text-on-surface-variant">
					400 banques affichées sur {visibles.length.toLocaleString("fr")} — affinez la recherche
					pour atteindre les autres.
				</p>
			)}

			<AudioPlayer
				piste={pisteCourante}
				onNavigate={naviguer}
				onClose={() => setPiste(null)}
			/>
		</div>
	);
}
