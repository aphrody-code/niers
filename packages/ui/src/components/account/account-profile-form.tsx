"use client";

import { zodResolver } from "@hookform/resolvers/zod";
import {
	AtSign,
	Check,
	Image as ImageIcon,
} from "lucide-react";
import { useState } from "react";
import { useForm } from "react-hook-form";
import { toast } from "sonner";
import { z } from "zod";

import { AVAILABLE_BADGES, MAX_BADGES } from "../../lib/badges-profil";
import { cn } from "../../lib/utils";
import { Badge } from "../badge";
import { Button } from "../button";
import { Card, CardContent, CardHeader, CardTitle } from "../card";
import {
	Form,
	FormControl,
	FormDescription,
	FormField,
	FormItem,
	FormLabel,
	FormMessage,
} from "../form";
import { Input } from "../input";
import { Label } from "../label";
import { Slider } from "../slider";
import { BannerPicker } from "./banner-picker";
import { Textarea } from "../textarea";
import type { AccountActionResult, AccountProfileValues } from "./types";

// `AVAILABLE_BADGES` et `MAX_BADGES` vivent dans `lib/badges-profil.ts` : lus
// depuis un module `"use client"`, ils devenaient des références client et la
// page serveur `/profil/[username]` échouait au build.

/**
 * La bannière par défaut : un dégradé de la charte, et rien d'autre.
 *
 * Les deux vignettes qui l'accompagnaient étaient des images générées par IA
 * (`/images/banners/griffon_banner_one.png`, `lightning_banner_two.png`) : un
 * griffon dans un cadre dans un cadre, un stade au texte illisible, et aucune
 * des deux ne montrant le jeu. Elles ont été retirées le 31/8/2026 — le CDN
 * sert les vraies illustrations d'Inazuma Eleven: Victory Road, décodées en
 * direct depuis les archives, et c'est ce que propose désormais la popup.
 */
const BANNIERE_DEFAUT = {
	className: "bg-linear-to-r from-rg-brique via-rg-brique-clair to-rg-brique",
	name: "Aura Rose Griffon (défaut)",
	url: "",
} as const;

/** Une bannière venue du CDN se reconnaît à son hôte, pas à une liste en dur. */
function estIllustrationCdn(url: string | null | undefined): boolean {
	return typeof url === "string" && url.includes("cdn.rosegriffon.fr");
}

/** Les quatre postes du jeu, plus « aucun » — l'ordre du terrain. */
const POSTES_CHOISISSABLES = [
	{ libelle: "Aucun", valeur: "" },
	{ libelle: "Gardien", valeur: "GAR" },
	{ libelle: "Défenseur", valeur: "DEF" },
	{ libelle: "Milieu", valeur: "MIL" },
	{ libelle: "Attaquant", valeur: "ATT" },
] as const;

const BIO_MAX = 500;

const schema = z.object({
	badges: z.array(z.string()).max(MAX_BADGES).default([]),
	banner_url: z.string().optional().or(z.literal("")),
	banner_position: z.coerce.number().int().min(0).max(100).default(50),
	poste: z.string().nullable().default(null),
	bio: z.string().max(BIO_MAX, `La bio ne peut pas dépasser ${BIO_MAX} caractères.`).optional(),
	full_name: z.string().optional(),
	twitter_handle: z.string().optional(),
	username: z
		.string()
		.min(3, "Le nom d'utilisateur doit faire au moins 3 caractères.")
		.max(20, "Le nom d'utilisateur ne peut pas dépasser 20 caractères."),
	website: z.string().url("URL invalide").optional().or(z.literal("")),
});

interface AccountProfileFormProps {
	values: AccountProfileValues;
	/** Base de l'URL publique du profil, affichée sous le champ pseudo. */
	profileUrlPrefix?: string;
	/**
	 * Bannière et badges : réservés aux apps qui les affichent réellement sur le
	 * profil public. Les masquer ailleurs évite de promettre une personnalisation
	 * qui ne se voit nulle part.
	 */
	showCosmetics?: boolean;
	onSubmit: (values: AccountProfileValues) => Promise<AccountActionResult>;
}

export function AccountProfileForm({
	values,
	profileUrlPrefix,
	showCosmetics = false,
	onSubmit,
}: AccountProfileFormProps) {
	// « URL personnalisée » ne s'ouvre que si la bannière posée n'est ni le
	// défaut ni une illustration du CDN — sinon le champ libre s'afficherait
	// pré-rempli à chaque visite de quelqu'un qui a choisi dans la galerie.
	const [customBanner, setCustomBanner] = useState(
		Boolean(values.banner_url) && !estIllustrationCdn(values.banner_url)
	);
	const [galerieOuverte, setGalerieOuverte] = useState(false);

	const form = useForm({
		defaultValues: values,
		resolver: zodResolver(schema),
	});

	const submit = async (next: z.infer<typeof schema>) => {
		const t = toast.loading("Mise à jour du profil…");
		try {
			const res = await onSubmit({
				badges: next.badges ?? [],
				banner_position: next.banner_position ?? 50,
				poste: next.poste || null,
				banner_url: next.banner_url ?? "",
				bio: next.bio ?? "",
				full_name: next.full_name ?? "",
				twitter_handle: next.twitter_handle ?? "",
				username: next.username,
				website: next.website ?? "",
			});
			if (res?.error) {
				// Le conflit d'unicité est la seule erreur rattachable à un champ.
				if (/utilisateur.*pris|already|unique/i.test(res.error)) {
					form.setError("username", { message: res.error, type: "manual" });
				}
				toast.error(res.error, { id: t });
				return;
			}
			toast.success("Profil mis à jour.", { id: t });
			form.reset(next);
		} catch (error) {
			console.error("[compte] échec de la mise à jour du profil", error);
			toast.error("Erreur lors de la mise à jour du profil.", { id: t });
		}
	};

	const badges = form.watch("badges") ?? [];
	const bannerUrl = form.watch("banner_url");
	// `z.coerce.number()` donne un type d'ENTRÉE incertain au formulaire (on peut
	// lui passer une chaîne) : on ramène la valeur observée à un nombre avant de
	// la donner au curseur, qui n'accepte rien d'autre.
	const cadrage = Number(form.watch("banner_position") ?? 50) || 50;
	const posteChoisi = form.watch("poste") ?? null;
	const bio = form.watch("bio") ?? "";
	const username = form.watch("username");

	const toggleBadge = (id: string) => {
		const current = form.getValues("badges") ?? [];
		if (current.includes(id)) {
			form.setValue(
				"badges",
				current.filter((b) => b !== id),
				{ shouldDirty: true }
			);
			return;
		}
		if (current.length >= MAX_BADGES) {
			toast.error(`Tu ne peux pas afficher plus de ${MAX_BADGES} badges à la fois.`);
			return;
		}
		form.setValue("badges", [...current, id], { shouldDirty: true });
	};

	return (
		<Card>
			<CardHeader>
				<CardTitle>Identité publique</CardTitle>
				{profileUrlPrefix && (
					<p className="text-sm text-muted-foreground">
						Visible par les autres membres sur{" "}
						<span className="font-medium text-foreground">
							{profileUrlPrefix}
							{username || "…"}
						</span>
					</p>
				)}
			</CardHeader>

			<CardContent>
				<Form {...form}>
					<form onSubmit={form.handleSubmit(submit)} className="space-y-6">
						<div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
							<FormField
								control={form.control}
								name="username"
								render={({ field }) => (
									<FormItem>
										<FormLabel className="flex items-center gap-1.5">
											<AtSign className="size-3.5 shrink-0" aria-hidden />
											Nom d'utilisateur
										</FormLabel>
										<FormControl>
											<Input placeholder="pseudo" autoComplete="username" {...field} />
										</FormControl>
										<FormDescription>Unique. Apparaît dans l'URL de ton profil.</FormDescription>
										<FormMessage />
									</FormItem>
								)}
							/>
							<FormField
								control={form.control}
								name="full_name"
								render={({ field }) => (
									<FormItem>
										<FormLabel>Nom complet</FormLabel>
										<FormControl>
											<Input placeholder="Prénom Nom" autoComplete="name" {...field} />
										</FormControl>
										<FormMessage />
									</FormItem>
								)}
							/>
						</div>

						<FormField
							control={form.control}
							name="bio"
							render={({ field }) => (
								<FormItem>
									<FormLabel>Biographie</FormLabel>
									<FormControl>
										<Textarea
											placeholder="Raconte-nous ton parcours de coach…"
											className="min-h-32 resize-none"
											maxLength={BIO_MAX}
											{...field}
										/>
									</FormControl>
									<div className="flex items-start justify-between gap-3">
										<FormMessage />
										<span className="ml-auto shrink-0 text-xs tabular-nums text-muted-foreground">
											{bio.length}/{BIO_MAX}
										</span>
									</div>
								</FormItem>
							)}
						/>

						<div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
							<FormField
								control={form.control}
								name="website"
								render={({ field }) => (
									<FormItem>
										<FormLabel>Site web</FormLabel>
										<FormControl>
											<Input
												placeholder="https://exemple.com"
												inputMode="url"
												autoComplete="url"
												{...field}
											/>
										</FormControl>
										<FormMessage />
									</FormItem>
								)}
							/>
							<FormField
								control={form.control}
								name="twitter_handle"
								render={({ field }) => (
									<FormItem>
										<FormLabel>Twitter / X</FormLabel>
										<FormControl>
											<Input placeholder="@pseudo" {...field} />
										</FormControl>
										<FormMessage />
									</FormItem>
								)}
							/>
						</div>

						{showCosmetics && (
							<>
								<div className="space-y-4 border-t border-border pt-6">
									<div>
										<h3 className="text-sm font-semibold text-foreground">Bannière de profil</h3>
										<p className="text-xs text-muted-foreground">
											Arrière-plan affiché en haut de ton profil public.
										</p>
									</div>

									{/* Aperçu réel plutôt qu'une vignette de catalogue : ce qu'on
									    voit ici est ce que le profil public affichera. */}
									<div className="relative h-24 overflow-hidden rounded-xl border-2 border-border sm:h-28">
										{bannerUrl ? (
											// eslint-disable-next-line @next/next/no-img-element
											<img
												src={bannerUrl}
												alt=""
												className="absolute inset-0 size-full object-cover"
												style={{ objectPosition: `50% ${cadrage}%` }}
											/>
										) : (
											<div className={cn("absolute inset-0", BANNIERE_DEFAUT.className)} />
										)}
										<span className="absolute inset-0 flex items-end bg-foreground/40 p-2 text-[10px] leading-tight font-bold text-background">
											{bannerUrl ? "Bannière choisie" : BANNIERE_DEFAUT.name}
										</span>
									</div>

									{/* Le curseur ne s'affiche que s'il y a une image à cadrer : sur
									    le dégradé par défaut, il ne déplacerait rien. */}
									{bannerUrl && (
										<div className="space-y-1.5">
											<div className="flex items-center justify-between gap-2">
												<Label htmlFor="banniere-cadrage">Cadrage vertical</Label>
												<span className="text-xs text-muted-foreground">
													{cadrage === 0
														? "Haut de l'image"
														: cadrage === 100
															? "Bas de l'image"
															: `${cadrage} %`}
												</span>
											</div>
											<Slider
												id="banniere-cadrage"
												min={0}
												max={100}
												step={1}
												value={[cadrage]}
												onValueChange={(valeurs) =>
													form.setValue("banner_position", Number(valeurs[0] ?? 50), {
														shouldDirty: true,
													})
												}
											/>
											<p className="text-xs text-muted-foreground">
												Une illustration du jeu est bien plus haute que la bande affichée :
												déplace-la pour choisir ce qui reste visible.
											</p>
										</div>
									)}

									<div className="flex flex-wrap gap-2">
										<Button
											type="button"
											variant="secondary"
											size="sm"
											className="rounded-full"
											onClick={() => setGalerieOuverte(true)}
										>
											<ImageIcon className="size-4" aria-hidden />
											Choisir une illustration du jeu
										</Button>
										{bannerUrl && (
											<Button
												type="button"
												variant="outline"
												size="sm"
												className="rounded-full"
												onClick={() => {
													setCustomBanner(false);
													form.setValue("banner_url", "", { shouldDirty: true });
												}}
											>
												Remettre le défaut
											</Button>
										)}
										<Button
											type="button"
											variant={customBanner ? "secondary" : "outline"}
											size="sm"
											className="rounded-full"
											aria-pressed={customBanner}
											onClick={() => {
												if (!customBanner) {
													form.setValue("banner_url", "", { shouldDirty: true });
												}
												setCustomBanner(true);
											}}
										>
											Utiliser une URL personnalisée
										</Button>
									</div>

									<BannerPicker
										open={galerieOuverte}
										onOpenChange={setGalerieOuverte}
										valeur={bannerUrl}
										onChoisir={(url) => {
											setCustomBanner(false);
											form.setValue("banner_url", url, { shouldDirty: true });
										}}
									/>

									{customBanner && (
										<FormField
											control={form.control}
											name="banner_url"
											render={({ field }) => (
												<FormItem>
													<FormLabel>URL de ta bannière</FormLabel>
													<FormControl>
														<Input placeholder="https://…" inputMode="url" {...field} />
													</FormControl>
													<FormDescription>Format recommandé : 1200 × 400 px.</FormDescription>
													<FormMessage />
												</FormItem>
											)}
										/>
									)}
								</div>

								<div className="space-y-4 border-t border-border pt-6">
									<div>
										<h3 className="text-sm font-semibold text-foreground">Poste</h3>
										<p className="text-xs text-muted-foreground">
											Affiché sur ta carte, comme sur une fiche de joueur du jeu.
										</p>
									</div>
									<div className="flex flex-wrap gap-2">
										{POSTES_CHOISISSABLES.map((entree) => {
											const choisi = posteChoisi === entree.valeur;
											return (
												<button
													key={entree.valeur || "aucun"}
													type="button"
													aria-pressed={choisi}
													onClick={() =>
														form.setValue("poste", entree.valeur || null, { shouldDirty: true })
													}
													className={cn(
														"rounded-full border-2 px-3 py-1 text-xs font-bold transition-colors",
														choisi
															? "border-primary bg-primary/10 text-foreground"
															: "border-border text-muted-foreground hover:border-muted-foreground/50"
													)}
												>
													{entree.libelle}
												</button>
											);
										})}
									</div>
								</div>

								<div className="space-y-4 border-t border-border pt-6">
									<div>
										<h3 className="flex flex-wrap items-center gap-2 text-sm font-semibold text-foreground">
											Badges de profil
											<Badge variant="outline" className="rounded-full text-[10px] font-bold">
												{badges.length} / {MAX_BADGES} sélectionnés
											</Badge>
										</h3>
										<p className="text-xs text-muted-foreground">
											Jusqu'à {MAX_BADGES} badges affichés à côté de ton nom.
										</p>
									</div>

									<div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
										{AVAILABLE_BADGES.map((badge) => {
											const selected = badges.includes(badge.id);
											return (
												<button
													type="button"
													key={badge.id}
													aria-pressed={selected}
													onClick={() => toggleBadge(badge.id)}
													className={cn(
														"flex items-start gap-3 rounded-2xl border-2 p-3 text-left transition-colors",
														selected
															? "border-primary bg-primary/5"
															: "border-border hover:border-muted-foreground/30 hover:bg-muted/40"
													)}
												>
													<span className={cn("shrink-0 rounded-xl border p-2", badge.tone)}>
														{/* Icône extraite du jeu, pas un pictogramme générique.
														    eslint-disable-next-line @next/next/no-img-element */}
														<img src={badge.iconeUrl} alt="" aria-hidden className="size-5" />
													</span>
													<span className="min-w-0 flex-1">
														<span className="block text-sm font-bold text-foreground">
															{badge.label}
														</span>
														<span className="mt-0.5 block text-xs leading-relaxed text-muted-foreground">
															{badge.description}
														</span>
													</span>
													<span
														className={cn(
															"mt-0.5 flex size-5 shrink-0 items-center justify-center rounded-full border-2 transition-colors",
															selected
																? "border-primary bg-primary text-primary-foreground"
																: "border-muted-foreground/30"
														)}
													>
														{selected && <Check className="size-3" strokeWidth={3} aria-hidden />}
													</span>
												</button>
											);
										})}
									</div>
								</div>
							</>
						)}

						<div className="flex justify-end border-t border-border pt-6">
							<Button
								type="submit"
								disabled={form.formState.isSubmitting}
								className="rounded-full px-8"
							>
								{form.formState.isSubmitting ? "Enregistrement…" : "Enregistrer le profil"}
							</Button>
						</div>
					</form>
				</Form>
			</CardContent>
		</Card>
	);
}
