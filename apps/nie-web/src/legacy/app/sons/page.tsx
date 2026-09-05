/**
 * Galerie sonore — tout le corpus audio d'Inazuma Eleven: Victory Road, jouable cue par cue.
 *
 * Ce que cette page débloque : un `.acb` est une BANQUE, pas un son. Le CDN ne savait décoder
 * que la piste la plus volumineuse de chaque banque, ce qui rendait l'essentiel du corpus
 * inatteignable — `waza_stream.acb` porte 1 512 cues, `bgm.acb` 825. La route `/audio-info`
 * publie désormais le catalogue d'une banque (nom, durée, codec, id AWB) et `/audio?id=` joue
 * n'importe lequel de ses cues.
 *
 * Server Component : la liste des banques vient du VFS **live** (`/vfs/ls`, crate
 * `nie_explore::listing`) et non d'un index figé. Le rattachement d'une banque de voix à son
 * personnage passe par le miroir SQLite (`lib/cpk/audio-names`). Le catalogue d'une banque,
 * lui, n'est chargé qu'à l'ouverture — 5 403 catalogues d'avance seraient absurdes.
 */
import type { Metadata } from "next";
import { SoundGallery, type BankEntry } from "@/app/sons/SoundGallery";
import { MediaEmpty, MediaHeader } from "@/components/wiki/MediaShell";
import { audioBankKind } from "@rosegriffon/azalee/cpk/audio";
import { lsOrNull } from "@rosegriffon/azalee/cpk/live";
import { bankOwner, ownerIndex } from "@/lib/cpk/audio-names";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

export const metadata: Metadata = {
	alternates: { canonical: "/sons" },
	description:
		"Toutes les banques sonores d'Inazuma Eleven: Victory Road — voix des personnages, musiques, techniques et effets, décodées à la volée depuis les fichiers du jeu.",
	openGraph: {
		description:
			"Voix, musiques, techniques et effets d'Inazuma Eleven: Victory Road, décodés live depuis les CPK.",
		locale: "fr_FR",
		siteName: "Azalée - Inazuma Eleven Victory Road",
		title: "Sons & voix | Azalée",
		type: "website",
		url: "/sons",
	},
	title: "Sons & voix — Inazuma Eleven Victory Road - Azalée",
};

/** Les trois emplacements où vivent les banques : la racine commune, puis les deux langues. */
const SOURCES = [
	{ dir: "data/common/sound_asset", lang: null },
	{ dir: "data/common/sound_asset/ja", lang: "ja" },
	{ dir: "data/common/sound_asset/en", lang: "en" },
] as const;

export default async function SonsPage() {
	const [listings, index] = await Promise.all([
		Promise.all(SOURCES.map((s) => lsOrNull(s.dir, 20_000))),
		ownerIndex(),
	]);

	const banks: BankEntry[] = [];
	for (const [i, listing] of listings.entries()) {
		if (!listing) continue;
		const lang = SOURCES[i].lang;
		for (const f of listing.files) {
			// L'ACB est le catalogue ; l'AWB frère n'est que le magasin d'octets, il ne se liste pas.
			if (f.ext !== "acb") continue;
			const owner = bankOwner(index, f.path);
			banks.push({
				path: f.path,
				name: f.name.replace(/\.acb$/i, ""),
				size: f.size,
				kind: audioBankKind(f.path),
				lang,
				owner: owner?.name ?? null,
				ownerSlug: owner?.slug ?? null,
			});
		}
	}
	banks.sort((a, b) => a.name.localeCompare(b.name, "fr", { numeric: true }));

	const indisponible = listings.every((l) => l === null);
	const nommees = banks.filter((b) => b.owner).length;

	return (
		<div className="w-full space-y-6">
			<MediaHeader
				title="Sons & voix"
				active="/sons"
				description={`${banks.length.toLocaleString("fr")} banques sonores d'Inazuma Eleven: Victory Road, décodées à la volée depuis les fichiers du jeu. Ouvrez une banque pour en lister les cues et les écouter un par un.`}
			/>

			{indisponible ? (
				<MediaEmpty>
					Le VFS du jeu est momentanément injoignable — la liste des banques ne peut pas être
					établie.
				</MediaEmpty>
			) : (
				<SoundGallery banks={banks} namedCount={nommees} />
			)}
		</div>
	);
}
