"use client";

/**
 * Enveloppe cliente de la visionneuse VRM.
 *
 * Double paresse, comme les autres îlots 3D d'azalée :
 *  1. `next/dynamic { ssr: false }` sort `three` + `@pixiv/three-vrm` du bundle
 *     de la page — le chunk n'est demandé qu'au montage de la scène ;
 *  2. un bouton d'activation retarde ce montage jusqu'au clic, pour ne pas
 *     ouvrir un contexte WebGL ni tirer plusieurs centaines de kilo-octets à
 *     chaque modèle survolé dans la galerie.
 *
 * Ce module n'importe QUE des types et des helpers client-safe
 * (`@/lib/vroid/types`, `@/lib/vroid/licence`) : aucun `server-only`, aucun
 * accès au jeton — le `.vrm` transite par `/api/vroid/vrm/{id}`.
 */
import dynamic from "next/dynamic";
import { useState } from "react";
import { estChargeable, nomModele } from "@/lib/vroid/licence";
import type { ModeleVroid } from "@/lib/vroid/types";
import { BoutonLiaison } from "./BoutonLiaison";
import { urlVignette } from "./vignette";

const SceneVrm = dynamic(() => import("./SceneVrm"), {
	ssr: false,
	loading: () => <CadreAttente texte="Préparation de la scène 3D…" />,
});

/** Cadre neutre aux dimensions de la scène, pour éviter tout saut de mise en page. */
function CadreAttente({ texte }: { texte: string }) {
	return (
		<div className="flex aspect-square w-full items-center justify-center gap-3 rounded-2xl bg-surface-container-high text-sm text-on-surface-variant">
			<div className="size-6 animate-spin rounded-full border-b-2 border-primary" />
			{texte}
		</div>
	);
}

export interface VisionneuseVrmProps {
	modele: ModeleVroid;
	/** L'internaute a lié son compte VRoid Hub (sans quoi aucun `.vrm` n'est chargeable). */
	connecte: boolean;
}

export function VisionneuseVrm({ modele, connecte }: VisionneuseVrmProps) {
	const [active, setActive] = useState(false);
	const nom = nomModele(modele);
	const chargeable = estChargeable(modele);

	if (active && connecte && chargeable) {
		return <SceneVrm idModele={modele.id} nom={nom} />;
	}

	const vignette = urlVignette(modele.portrait_image?.w600?.url ?? modele.portrait_image?.original?.url);

	return (
		<div className="relative aspect-square w-full overflow-hidden rounded-2xl bg-surface-container-high">
			{vignette && (
				// eslint-disable-next-line @next/next/no-img-element -- image relayée par /api/vroid/image, hors optimiseur Next
				<img
					src={vignette}
					alt={`Portrait du modèle ${nom}`}
					className="absolute inset-0 size-full object-cover opacity-40"
					loading="lazy"
					decoding="async"
				/>
			)}

			<div className="absolute inset-0 flex flex-col items-center justify-center gap-3 p-6 text-center">
				{!connecte ? (
					<>
						<p className="text-sm text-on-surface-variant">
							Liez votre compte VRoid Hub pour afficher ce modèle en 3D.
						</p>
						<BoutonLiaison />
					</>
				) : chargeable ? (
					<button
						className="rounded-full bg-primary px-4 py-2 text-sm font-semibold text-on-primary transition hover:opacity-90"
						onClick={() => setActive(true)}
						type="button"
					>
						Afficher en 3D
					</button>
				) : (
					<p className="text-sm text-on-surface-variant">
						L&apos;auteur n&apos;autorise pas le téléchargement de ce modèle. Azalée n&apos;étant pas
						une application approuvée par VRoid Hub, elle ne peut pas l&apos;afficher en 3D.
					</p>
				)}
			</div>
		</div>
	);
}
