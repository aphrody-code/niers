import type { Metadata } from "next";
import { SaveUploader } from "@/components/save/SaveUploader";

export const metadata: Metadata = {
	alternates: { canonical: "/save" },
	description:
		"Importez votre fichier de sauvegarde Inazuma Eleven: Victory Road et obtenez un résumé instantané (joueur, niveau, temps de jeu, roster). Tout est lu dans votre navigateur — la sauvegarde ne quitte jamais votre appareil.",
	title: "Lecteur de sauvegarde | Inazuma Eleven Victory Road - Azalée",
};

// Page statique : tout le travail est client-side (wasm). Pas de revalidation DB.
export const revalidate = 86_400;

export default function SavePage() {
	return (
		<div className="w-full max-w-4xl mx-auto">
			<header className="
     mb-6
     sm:mb-8
   ">
				<h1 className="
      font-[BradBunR] text-2xl
      sm:text-3xl
      md:text-4xl
      text-on-surface tracking-wide
    ">
					Lecteur de sauvegarde
				</h1>
				<p className="mt-2 type-body-medium text-on-surface-variant leading-relaxed">
					Déposez votre fichier de sauvegarde IEVR (ex.{" "}
					<code className="rounded bg-surface-container px-1.5 py-0.5 type-body-small text-on-surface">
						002AB8F4-USERDATALIVE
					</code>
					) pour en lire le détail. La sauvegarde est déchiffrée et analysée{" "}
					<strong className="text-on-surface">entièrement dans votre navigateur</strong> — aucun
					octet de la sauvegarde n&apos;est envoyé sur nos serveurs. Seule la liste des
					identifiants de personnages transite (jamais le fichier) pour résoudre les noms du
					roster contre la base Azalée.
				</p>
			</header>

			<SaveUploader />
		</div>
	);
}
