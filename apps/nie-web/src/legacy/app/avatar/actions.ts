"use server";

/**
 * Appliquer l'avatar composé dans l'éditeur comme photo du compte.
 *
 * Le mécanisme de photo de profil existait déjà (téléversement dans le seau `avatars`, puis
 * écriture de `profiles.avatar_url`) : cette action s'y branche au lieu d'en ouvrir un second
 * chemin. Deux différences avec le téléversement depuis les réglages — l'image ne vient pas du
 * poste de l'utilisateur mais du CDN du jeu, et elle est donc récupérée **côté serveur**.
 *
 * L'identité vient de la session, jamais d'un paramètre : sans cela, n'importe quel appelant
 * réécrirait la photo d'un autre compte.
 */

import { revalidatePath } from "next/cache";

import { getServerSession } from "@/lib/auth-helpers";
import { createAdminClient } from "@/lib/supabase/admin";

/** Origine qui sert les vignettes décodées du jeu. */
const CDN = process.env.NEXT_PUBLIC_CDN_ORIGIN ?? "https://cdn.rosegriffon.fr";

/** Seau de stockage des photos de profil, celui des réglages du compte. */
const SEAU = "avatars";

/** Plafond de taille de la vignette récupérée — une icône du jeu pèse quelques kilo-octets. */
const TAILLE_MAX = 2 * 1024 * 1024;

/**
 * Nom d'icône du jeu : lettres, chiffres et soulignés uniquement.
 *
 * La valeur part dans une URL ; sans ce filtre, un nom fabriqué ferait sortir la requête du
 * chemin des vignettes.
 */
const NOM_VALIDE = /^[A-Za-z0-9_]{3,64}$/;

/**
 * Pose la vignette `icone` (une image du jeu) comme photo du compte connecté.
 *
 * @param icone nom d'icône tel que le catalogue le fournit, p. ex. `icon_ava_face05_001`.
 * @returns l'URL publique enregistrée, ou un message d'erreur affichable.
 */
export async function definirPhotoDepuisAvatar(
	icone: string,
): Promise<{ url?: string; error?: string }> {
	const session = await getServerSession();
	if (!session?.user) {
		return { error: "Connecte-toi pour utiliser cet avatar comme photo de profil." };
	}
	if (!NOM_VALIDE.test(icone)) {
		return { error: "Vignette inconnue." };
	}

	let octets: ArrayBuffer;
	try {
		// La vignette est un fichier du jeu : elle ne change pas. `no-store` faisait re-décoder
		// l'atlas au serveur d'assets à chaque clic, pour un résultat identique.
		const reponse = await fetch(`${CDN}/avatar/icon/${icone}.png`, { cache: "force-cache" });
		if (!reponse.ok) {
			return { error: "Cette vignette n'a pas pu être récupérée." };
		}
		octets = await reponse.arrayBuffer();
	} catch {
		return { error: "Le service d'images du jeu est injoignable." };
	}
	if (octets.byteLength === 0 || octets.byteLength > TAILLE_MAX) {
		return { error: "Cette vignette n'a pas pu être récupérée." };
	}

	const admin = createAdminClient();
	// Chemin daté, comme le téléversement des réglages : un chemin fixe resservirait l'ancienne
	// image depuis le cache du CDN et du navigateur.
	const chemin = `${session.user.id}/avatar-${Date.now()}.png`;
	const { error: erreurEnvoi } = await admin.storage
		.from(SEAU)
		.upload(chemin, octets, { contentType: "image/png", upsert: true });
	if (erreurEnvoi) {
		console.error("[avatar] téléversement impossible", erreurEnvoi);
		return { error: "Impossible d'enregistrer cette photo." };
	}

	const {
		data: { publicUrl },
	} = admin.storage.from(SEAU).getPublicUrl(chemin);

	const { error: erreurProfil } = await admin
		.from("profiles")
		.update({ avatar_url: publicUrl, updated_at: new Date().toISOString() })
		.eq("id", session.user.id);
	if (erreurProfil) {
		console.error("[avatar] écriture du profil impossible", erreurProfil);
		return { error: "Impossible d'enregistrer cette photo." };
	}

	revalidatePath("/settings");
	revalidatePath("/avatar");
	return { url: publicUrl };
}

// ─── Sauvegarde et partage ────────────────────────────────────────────────────

/** Ce qu'une sauvegarde retient de l'éditeur : de quoi rétablir l'avatar à l'identique. */
export type AvatarEnregistre = {
	choix: Record<number, string>;
	valeurs: Record<string, number>;
	champs: Record<string, string>;
	genre: number;
	morphologie: number;
};

/** Une sauvegarde telle qu'elle revient de la base. */
export type Sauvegarde = {
	id: string;
	nom: string;
	code: string | null;
	donnees: AvatarEnregistre;
	modifieLe: string;
};

/** Longueur du code de partage, imposée par la spécification du jeu : 410 bits sur 6. */
const LONGUEUR_CODE = 69;

/** Nombre de sauvegardes qu'un compte peut garder. */
const MAX_SAUVEGARDES = 24;

/** Un nom de sauvegarde lisible, borné, sans caractères de contrôle. */
function nomPropre(brut: string): string {
	const nettoye = [...brut.trim()].filter((c) => c >= " ").join("").slice(0, 40);
	return nettoye.length > 0 ? nettoye : "Avatar";
}

/**
 * Enregistre l'avatar courant sous le compte connecté.
 *
 * `code` est le code de partage produit par l'éditeur : il rend la sauvegarde lisible par
 * quiconque le possède, ce que la politique de lecture par code autorise explicitement. Le passer
 * à `null` garde la sauvegarde privée.
 */
export async function enregistrerAvatar(
	nom: string,
	donnees: AvatarEnregistre,
	code: string | null,
): Promise<{ id?: string; error?: string }> {
	const session = await getServerSession();
	if (!session?.user?.id) {
		return { error: "Connecte-toi pour enregistrer un avatar." };
	}
	if (code !== null && code.length !== LONGUEUR_CODE) {
		return { error: "Ce code de partage n'a pas la bonne longueur." };
	}

	const admin = createAdminClient();
	const { count } = await admin
		.from("avatar_saves")
		.select("id", { count: "exact", head: true })
		.eq("user_id", session.user.id);
	if ((count ?? 0) >= MAX_SAUVEGARDES) {
		return { error: `Tu as atteint ${MAX_SAUVEGARDES} avatars enregistrés — supprimes-en un.` };
	}

	const { data, error } = await admin
		.from("avatar_saves")
		.insert({ user_id: session.user.id, nom: nomPropre(nom), donnees, code })
		.select("id")
		.single();
	if (error) {
		// Le code est unique : deux avatars identiques donneraient le même, ce qui n'est pas une
		// panne mais un doublon à signaler comme tel.
		if (error.code === "23505") return { error: "Cet avatar est déjà partagé sous ce code." };
		console.error("[avatar] enregistrement impossible", error);
		return { error: "Impossible d'enregistrer cet avatar." };
	}

	revalidatePath("/avatar");
	return { id: data.id };
}

/** Les avatars enregistrés du compte connecté, du plus récent au plus ancien. */
export async function listerAvatars(): Promise<{ avatars?: Sauvegarde[]; error?: string }> {
	const session = await getServerSession();
	if (!session?.user?.id) return { avatars: [] };

	const admin = createAdminClient();
	const { data, error } = await admin
		.from("avatar_saves")
		.select("id, nom, code, donnees, modifie_le")
		.eq("user_id", session.user.id)
		.order("modifie_le", { ascending: false })
		.limit(MAX_SAUVEGARDES);
	if (error) {
		console.error("[avatar] lecture impossible", error);
		return { error: "Impossible de relire tes avatars." };
	}
	return {
		avatars: (data ?? []).map((r) => ({
			id: r.id,
			nom: r.nom,
			code: r.code,
			donnees: r.donnees as AvatarEnregistre,
			modifieLe: r.modifie_le,
		})),
	};
}

/**
 * Retrouve un avatar par son code de partage.
 *
 * Sert à ouvrir l'avatar de quelqu'un d'autre : la lecture par code est ouverte, l'écriture reste
 * réservée au propriétaire.
 */
export async function ouvrirParCode(
	code: string,
): Promise<{ avatar?: AvatarEnregistre; nom?: string; error?: string }> {
	const propre = code.trim();
	if (propre.length !== LONGUEUR_CODE) {
		return { error: "Ce code de partage n'a pas la bonne longueur." };
	}

	const admin = createAdminClient();
	const { data, error } = await admin
		.from("avatar_saves")
		.select("nom, donnees")
		.eq("code", propre)
		.maybeSingle();
	if (error) {
		console.error("[avatar] ouverture par code impossible", error);
		return { error: "Impossible d'ouvrir ce code." };
	}
	if (!data) return { error: "Aucun avatar ne porte ce code." };
	return { avatar: data.donnees as AvatarEnregistre, nom: data.nom };
}

/** Supprime une sauvegarde du compte connecté. */
export async function supprimerAvatar(id: string): Promise<{ error?: string }> {
	const session = await getServerSession();
	if (!session?.user?.id) return { error: "Connecte-toi pour supprimer un avatar." };

	const admin = createAdminClient();
	const { error } = await admin
		.from("avatar_saves")
		.delete()
		.eq("id", id)
		.eq("user_id", session.user.id);
	if (error) {
		console.error("[avatar] suppression impossible", error);
		return { error: "Impossible de supprimer cet avatar." };
	}
	revalidatePath("/avatar");
	return {};
}
