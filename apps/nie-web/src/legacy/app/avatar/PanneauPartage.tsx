"use client";

/**
 * Le panneau de sauvegarde et de partage de l'éditeur.
 *
 * Trois gestes, dans l'ordre où on s'en sert : relire le **code** de l'avatar courant pour le
 * transmettre, **ouvrir** celui de quelqu'un d'autre, et **enregistrer** le sien sous le compte.
 *
 * Le code n'est pas inventé : sa spécification — alphabet de 64 caractères, 410 bits, 86
 * emplacements — vient du catalogue du jeu, et le codec vit dans `partage.ts`. Ce qu'il garantit
 * et ce qu'il ne garantit pas y est écrit ; en particulier, un code produit ici se relit ici, et
 * rien ne permet aujourd'hui d'affirmer que le jeu le relirait.
 */

import { useCallback, useEffect, useState, useTransition } from "react";
import {
	enregistrerAvatar,
	listerAvatars,
	ouvrirParCode,
	supprimerAvatar,
	type AvatarEnregistre,
	type Sauvegarde,
} from "./actions";

/** Ce que le panneau doit savoir de l'éditeur, et ce qu'il lui rend. */
export type PartageProps = {
	/** L'état courant de l'éditeur, tel qu'il sera enregistré. */
	etat: AvatarEnregistre;
	/** Le code de partage de l'état courant, déjà calculé par l'éditeur. */
	code: string | null;
	/** Rétablit un avatar dans l'éditeur. */
	restaurer: (avatar: AvatarEnregistre) => void;
	/** Ferme le panneau. */
	fermer: () => void;
};

export function Partage({ etat, code, restaurer, fermer }: PartageProps) {
	const [avatars, setAvatars] = useState<Sauvegarde[]>([]);
	const [saisie, setSaisie] = useState("");
	const [nom, setNom] = useState("");
	const [message, setMessage] = useState<string | null>(null);
	const [copie, setCopie] = useState(false);
	const [enCours, demarrer] = useTransition();

	const recharger = useCallback(() => {
		void listerAvatars().then((r) => {
			if (r.avatars) setAvatars(r.avatars);
			if (r.error) setMessage(r.error);
		});
	}, []);

	useEffect(recharger, [recharger]);

	const copier = useCallback(() => {
		if (!code) return;
		void navigator.clipboard
			.writeText(code)
			.then(() => {
				setCopie(true);
				setTimeout(() => setCopie(false), 2000);
			})
			.catch(() => setMessage("La copie a échoué — le code reste sélectionnable."));
	}, [code]);

	const ouvrir = useCallback(() => {
		demarrer(() => {
			void ouvrirParCode(saisie).then((r) => {
				if (r.error) {
					setMessage(r.error);
					return;
				}
				if (r.avatar) {
					restaurer(r.avatar);
					setMessage(`Avatar « ${r.nom ?? "sans nom"} » ouvert.`);
				}
			});
		});
	}, [saisie, restaurer]);

	const enregistrer = useCallback(() => {
		demarrer(() => {
			void enregistrerAvatar(nom, etat, code).then((r) => {
				setMessage(r.error ?? "Avatar enregistré.");
				if (!r.error) {
					setNom("");
					recharger();
				}
			});
		});
	}, [nom, etat, code, recharger]);

	return (
		<div
			className="fixed inset-0 z-[2147483100] flex items-center justify-center bg-black/60 p-4"
			onClick={fermer}
			onKeyDown={(e) => e.key === "Escape" && fermer()}
			role="presentation"
		>
			{/* eslint-disable-next-line jsx-a11y/no-noninteractive-element-interactions */}
			<div
				className="w-full max-w-lg rounded-2xl bg-white p-6 text-slate-900 shadow-2xl"
				onClick={(e) => e.stopPropagation()}
				role="dialog"
				aria-modal="true"
				aria-label="Sauvegarder et partager cet avatar"
			>
				<h2 className="mb-4 text-xl font-semibold">Sauvegarder et partager</h2>

				<section className="mb-5">
					<h3 className="mb-1 text-sm font-medium text-slate-600">Le code de cet avatar</h3>
					<div className="flex items-center gap-2">
						<code className="flex-1 select-all break-all rounded-lg bg-slate-100 px-3 py-2 font-mono text-xs">
							{code ?? "indisponible"}
						</code>
						<button
							type="button"
							onClick={copier}
							disabled={!code}
							className="shrink-0 rounded-lg bg-slate-900 px-3 py-2 text-sm text-white disabled:opacity-40"
						>
							{copie ? "Copié" : "Copier"}
						</button>
					</div>
				</section>

				<section className="mb-5">
					<h3 className="mb-1 text-sm font-medium text-slate-600">Ouvrir un code</h3>
					<div className="flex items-center gap-2">
						<input
							value={saisie}
							onChange={(e) => setSaisie(e.target.value)}
							placeholder="Colle un code ici"
							className="flex-1 rounded-lg border border-slate-300 px-3 py-2 font-mono text-xs"
						/>
						<button
							type="button"
							onClick={ouvrir}
							disabled={enCours || saisie.trim().length === 0}
							className="shrink-0 rounded-lg bg-slate-900 px-3 py-2 text-sm text-white disabled:opacity-40"
						>
							Ouvrir
						</button>
					</div>
				</section>

				<section className="mb-5">
					<h3 className="mb-1 text-sm font-medium text-slate-600">Enregistrer sous mon compte</h3>
					<div className="flex items-center gap-2">
						<input
							value={nom}
							onChange={(e) => setNom(e.target.value)}
							placeholder="Nom de l'avatar"
							maxLength={40}
							className="flex-1 rounded-lg border border-slate-300 px-3 py-2 text-sm"
						/>
						<button
							type="button"
							onClick={enregistrer}
							disabled={enCours}
							className="shrink-0 rounded-lg bg-blue-600 px-3 py-2 text-sm text-white disabled:opacity-40"
						>
							Enregistrer
						</button>
					</div>
				</section>

				{avatars.length > 0 && (
					<section className="mb-4">
						<h3 className="mb-1 text-sm font-medium text-slate-600">
							Mes avatars ({avatars.length})
						</h3>
						<ul className="max-h-48 space-y-1 overflow-y-auto">
							{avatars.map((a) => (
								<li key={a.id} className="flex items-center gap-2 rounded-lg bg-slate-50 px-3 py-2">
									<span className="flex-1 truncate text-sm">{a.nom}</span>
									<button
										type="button"
										onClick={() => restaurer(a.donnees)}
										className="rounded px-2 py-1 text-xs text-blue-700 hover:bg-blue-50"
									>
										Ouvrir
									</button>
									<button
										type="button"
										onClick={() =>
											demarrer(() => {
												void supprimerAvatar(a.id).then((r) => {
													setMessage(r.error ?? "Avatar supprimé.");
													recharger();
												});
											})
										}
										className="rounded px-2 py-1 text-xs text-red-700 hover:bg-red-50"
									>
										Supprimer
									</button>
								</li>
							))}
						</ul>
					</section>
				)}

				{message && <p className="mb-3 text-sm text-slate-700">{message}</p>}

				<button
					type="button"
					onClick={fermer}
					className="w-full rounded-lg border border-slate-300 px-3 py-2 text-sm"
				>
					Fermer
				</button>
			</div>
		</div>
	);
}
