/**
 * Bloc « Conditions d'utilisation des données du modèle ».
 *
 * pixiv impose de présenter ces conditions **avant** tout chargement d'un
 * modèle, avec un titre fixe et une valeur explicite par condition.
 * Source : https://developer.vroid.com/en/guidelines/conditions_of_use.html
 *
 * Composant purement présentationnel : ni état, ni `"use client"`. Il est donc
 * rendu côté serveur quand la page le peut, et se laisse aussi importer par un
 * îlot client.
 */
import { conditionsDuModele, TEXTE_VERDICT, TITRE_CONDITIONS, type Verdict } from "@/lib/vroid/licence";
import type { ModeleVroid } from "@/lib/vroid/types";

/** Couleur du verdict — un refus doit se voir sans lire. */
const TEINTE: Record<Verdict, string> = {
	autorise: "text-primary",
	interdit: "text-error",
	"non-lucratif": "text-on-surface",
	requis: "text-on-surface",
	"non-requis": "text-on-surface-variant",
	inconnu: "text-on-surface-variant",
};

export interface ConditionsUtilisationProps {
	modele: ModeleVroid;
	/** Compacte l'affichage pour une carte de galerie. */
	compact?: boolean;
}

export function ConditionsUtilisation({ modele, compact = false }: ConditionsUtilisationProps) {
	const { conditions, specification } = conditionsDuModele(modele);

	if (conditions.length === 0) {
		return (
			<div className="rounded-2xl border border-outline-variant/40 bg-surface-container-low p-4">
				<h3 className="text-sm font-semibold text-on-surface">{TITRE_CONDITIONS}</h3>
				<p className="mt-2 text-sm text-on-surface-variant">
					VRoid Hub ne renvoie pas les conditions d&apos;utilisation dans cette liste. Ouvrez la fiche
					du modèle pour les consulter.
				</p>
			</div>
		);
	}

	return (
		<div className="rounded-2xl border border-outline-variant/40 bg-surface-container-low p-4">
			<div className="flex items-baseline justify-between gap-3">
				<h3 className="text-sm font-semibold text-on-surface">{TITRE_CONDITIONS}</h3>
				<span className="text-xs text-on-surface-variant">
					{specification === "vrm1" ? "VRM 1.0" : "VRM 0.0"}
				</span>
			</div>

			<dl className={`mt-3 grid gap-x-4 gap-y-1.5 ${compact ? "grid-cols-1" : "sm:grid-cols-2"}`}>
				{conditions.map((condition) => (
					<div className="flex items-baseline justify-between gap-3 text-sm" key={condition.cle}>
						<dt className="text-on-surface-variant">{condition.libelle}</dt>
						<dd className={`shrink-0 font-medium ${TEINTE[condition.verdict]}`}>
							{TEXTE_VERDICT[condition.verdict]}
						</dd>
					</div>
				))}
			</dl>

			<p className="mt-3 text-xs text-on-surface-variant">
				Conditions déclarées par l&apos;auteur sur VRoid Hub. Azalée les relaie sans les modifier et
				n&apos;héberge aucun modèle.
			</p>
		</div>
	);
}
