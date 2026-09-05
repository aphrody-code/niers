/**
 * Passerelle VRoid Hub — parcourir des modèles de personnages et les afficher
 * en 3D dans le navigateur.
 *
 * Ce que cette page peut faire, et pourquoi elle ne fait pas plus :
 *
 * - La **sélection éditoriale**, la **recherche** et la **fiche détaillée** sont
 *   des endpoints publics de VRoid Hub : ils répondent sans jeton (mesuré le
 *   2026-09-02, `X-Api-Version: 11` suffit). La galerie s'affiche donc pour un
 *   visiteur qui n'a rien lié.
 * - Les **modèles du compte** et les **coups de cœur** exigent un jeton OAuth
 *   (401 `COMMON_SIGNED_IN_REQUIRED` sinon), obtenu par le flux
 *   `authorization_code` + PKCE de `/api/vroid/login`.
 * - Le **`.vrm`** n'est chargeable qu'après liaison du compte, et seulement
 *   pour les modèles dont l'auteur autorise le téléchargement : azalée est une
 *   application **non approuvée** au sens de VRoid Hub
 *   (https://developer.vroid.com/en/api/recognize.html).
 * - Aucun modèle n'est **hébergé ni mis en cache** : `/api/vroid/vrm/{id}`
 *   relaie le flux en `no-store`. Un modèle VRoid Hub n'est pas redistribuable.
 * - Le scope `heart` n'ouvre que la **lecture** de `/api/hearts` : l'API ne
 *   documente aucun endroit où poser ou retirer un cœur. La page affiche donc
 *   le compteur et renvoie vers hub.vroid.com pour l'action.
 */
import type { Metadata } from "next";
import { ErreurVroid, selectionEditoriale } from "@/lib/vroid/client";
import { lireConfigVroid } from "@/lib/vroid/config";
import { jetonValide } from "@/lib/vroid/session";
import type { PageModeles } from "@/lib/vroid/types";
import { BoutonLiaison } from "./BoutonLiaison";
import { GalerieVroid } from "./GalerieVroid";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

export const metadata: Metadata = {
	alternates: { canonical: "https://azalee.rosegriffon.fr/vroid" },
	description:
		"Parcourez les modèles de personnages de VRoid Hub et affichez-les en 3D dans le navigateur. Les conditions d'utilisation fixées par leurs auteurs sont affichées avant tout chargement.",
	title: "VRoid Hub — modèles 3D | Azalée",
};

/** Messages de retour du flux d'autorisation, indexés par le code de `?vroid=`. */
const MESSAGES_RETOUR: Record<string, string> = {
	connecte: "Compte VRoid Hub lié.",
	refus: "Autorisation refusée sur VRoid Hub.",
	"flux-perdu": "La demande d'autorisation a expiré. Relancez la liaison.",
	"state-invalide": "Retour d'autorisation invalide : la demande a été rejetée.",
	"echange-refuse": "VRoid Hub a refusé l'échange du code d'autorisation.",
	"non-configure": "VRoid Hub n'est pas configuré sur cette instance.",
};

export default async function PageVroid({
	searchParams,
}: {
	searchParams: Promise<{ vroid?: string }>;
}) {
	const [{ vroid }, configure] = await Promise.all([
		searchParams,
		Promise.resolve(Boolean(lireConfigVroid())),
	]);

	// La liaison n'est possible que si l'application est configurée ; sans
	// configuration, `jetonValide()` léverait sur le déchiffrement du cookie.
	const connecte = configure ? Boolean(await jetonValide()) : false;

	let page: PageModeles = { modeles: [], curseurSuivant: null };
	let erreur: string | null = null;
	try {
		page = await selectionEditoriale({ nombre: 24 });
	} catch (cause) {
		// VRoid Hub injoignable ou quota atteint : on rend le cadre, jamais une 500.
		erreur =
			cause instanceof ErreurVroid
				? `VRoid Hub n'a pas répondu (${cause.statut || "réseau"}).`
				: "VRoid Hub est momentanément injoignable.";
	}

	const message = vroid ? MESSAGES_RETOUR[vroid] : null;

	return (
		<div className="w-full space-y-5">
			<header className="space-y-2">
				<h1 className="text-fluid-headline-md font-extrabold text-on-surface">VRoid Hub</h1>
				<p className="max-w-3xl text-sm text-on-surface-variant">
					VRoid Hub est la plateforme de partage de modèles 3D de personnages de pixiv. Azalée s&apos;y
					connecte pour les parcourir et les afficher dans le navigateur — elle n&apos;en héberge
					aucun, et affiche les conditions d&apos;utilisation fixées par leurs auteurs.
				</p>
			</header>

			{message && (
				<p className="rounded-2xl border border-outline-variant/40 bg-surface-container-low px-4 py-3 text-sm text-on-surface">
					{message}
				</p>
			)}

			{configure ? (
				<div className="flex flex-wrap items-center gap-3 rounded-2xl border border-outline-variant/40 bg-surface-container-low px-4 py-3">
					<p className="flex-1 text-sm text-on-surface-variant">
						{connecte
							? "Compte VRoid Hub lié : vos modèles, vos coups de cœur et l'affichage 3D sont accessibles."
							: "Liez votre compte VRoid Hub pour retrouver vos modèles et afficher les modèles téléchargeables en 3D."}
					</p>
					{connecte ? <FormulaireDeliaison /> : <BoutonLiaison />}
				</div>
			) : (
				<p className="rounded-2xl border border-outline-variant/40 bg-surface-container-low px-4 py-3 text-sm text-on-surface-variant">
					La liaison de compte est désactivée sur cette instance : les variables
					<code className="mx-1 rounded bg-surface-container px-1">VROID_APPLICATION_ID</code>
					et <code className="mx-1 rounded bg-surface-container px-1">VROID_SECRET</code>
					ne sont pas renseignées. La sélection publique reste consultable.
				</p>
			)}

			<GalerieVroid connecte={connecte} erreurInitiale={erreur} pageInitiale={page} />

			<footer className="rounded-2xl border border-outline-variant/40 bg-surface-container-low p-4 text-xs text-on-surface-variant">
				Les modèles, images et métadonnées proviennent de VRoid Hub (pixiv) et restent la propriété de
				leurs auteurs. Azalée ne les redistribue pas : les fichiers `.vrm` sont relayés à la demande,
				sans stockage ni cache, et uniquement lorsque leur auteur en autorise le téléchargement.
			</footer>
		</div>
	);
}

/**
 * Bouton de déliaison.
 *
 * `DELETE /api/vroid/session` révoque le jeton auprès de VRoid Hub avant
 * d'effacer le cookie : un simple `cookies().delete()` laisserait un jeton
 * valide traîner côté hub.vroid.com.
 */
function FormulaireDeliaison() {
	return (
		<form
			action={async () => {
				"use server";
				const { revoquerJeton } = await import("@/lib/vroid/oauth");
				const { effacerSession, jetonValide: lireJeton } = await import("@/lib/vroid/session");
				const { lireConfigVroid: lireConfig } = await import("@/lib/vroid/config");

				const config = lireConfig();
				const jeton = await lireJeton();
				if (config && jeton) await revoquerJeton(config, jeton);
				await effacerSession();

				// Sans invalidation, la page continuerait d'afficher « compte lié » :
				// le rendu serveur est mis en cache pour la navigation en cours.
				const { revalidatePath } = await import("next/cache");
				revalidatePath("/vroid");
			}}
		>
			<button
				className="rounded-full bg-surface-container px-4 py-2 text-sm font-medium text-on-surface transition hover:bg-surface-container-high"
				type="submit"
			>
				Délier mon compte
			</button>
		</form>
	);
}
