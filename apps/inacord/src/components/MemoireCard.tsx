/**
 * Panneau mémoire — ce que l'explorateur retient en RAM, et comment le rendre.
 *
 * Le VFS garde les octets **bruts** de chaque paquet CPK ouvert dans un cache LRU. Quelques
 * lectures dans des paquets différents suffisent à retenir plusieurs centaines de mégaoctets,
 * et rien ne le disait nulle part : le symptôme (la machine qui rame) n'accuse jamais le cache.
 *
 * `nie-formats` fixe le budget par défaut à 16 Gio — cohérent pour un traitement par lots qui a
 * la machine pour lui, hors sujet pour une application de bureau qui tourne à côté du jeu.
 * L'explorateur l'abaisse à 1 Gio, sauf si `NIE_CPK_CACHE_BUDGET_GIB` est posée : une variable
 * définie est un choix délibéré, et on ne l'écrase pas.
 */

import { useCallback, useEffect, useState } from "react";
import { commands } from "@/lib/bindings";
import type { CacheCpkDto } from "@/lib/bindings";
import { toast } from "sonner";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";

/** Rafraîchissement automatique, en millisecondes. */
const PERIODE = 5000;

/** Affiche des mégaoctets sous une forme lisible. */
function lisible(mo: number): string {
	return mo >= 1024 ? `${(mo / 1024).toFixed(2)} Gio` : `${mo} Mio`;
}

export function MemoireCard() {
	const [stats, setStats] = useState<CacheCpkDto | null>(null);
	const [erreur, setErreur] = useState<string | null>(null);
	const [occupe, setOccupe] = useState(false);

	const rafraichir = useCallback(async () => {
		try {
			const r = await commands.vfsCacheStats(null);
			if (r.status === "ok") {
				setStats(r.data);
				setErreur(null);
			} else {
				setErreur(String(r.error));
			}
		} catch (e) {
			setErreur(String(e));
		}
	}, []);

	useEffect(() => {
		void rafraichir();
		const t = setInterval(() => void rafraichir(), PERIODE);
		return () => clearInterval(t);
	}, [rafraichir]);

	async function vider() {
		setOccupe(true);
		try {
			const r = await commands.vfsCacheVider(null);
			if (r.status === "ok") {
				toast.success(`Cache vidé — ${lisible(r.data)} rendus`);
				await rafraichir();
			} else {
				toast.error(String(r.error));
			}
		} catch (e) {
			toast.error(String(e));
		} finally {
			setOccupe(false);
		}
	}

	// Une part vaut 0 quand le budget est nul : sans cette garde, la barre part en NaN et
	// disparaît sans dire pourquoi.
	const part = stats && stats.budget_mo > 0 ? Math.min(1, stats.octets_mo / stats.budget_mo) : 0;

	return (
		<div className="rounded-lg border border-app-line bg-app-box p-4">
			<div className="mb-1 flex items-baseline justify-between">
				<h3 className="text-sm font-medium">Mémoire — cache des paquets CPK</h3>
				{stats && (
					<span className="font-mono text-xs text-muted-foreground">
						{stats.entrees} paquet{stats.entrees > 1 ? "s" : ""}
					</span>
				)}
			</div>

			<p className="mb-3 text-xs text-muted-foreground">
				Le VFS garde les octets bruts de chaque paquet ouvert pour éviter de le relire. Vider
				rend la RAM immédiatement ; les lectures suivantes relisent depuis le disque.
			</p>

			{erreur && (
				<Alert variant="destructive" className="mb-2">
					<AlertTitle>Mesure impossible</AlertTitle>
					<AlertDescription>{erreur}</AlertDescription>
				</Alert>
			)}

			{stats ? (
				<>
					<div className="mb-1 flex items-baseline justify-between gap-2 font-mono text-xs">
						<span>{lisible(stats.octets_mo)}</span>
						{/* Le seuil était porté par la COULEUR de la jauge (ambre au-delà de 85 %), donc
						    perdu pour qui ne la distingue pas — et perdu tout court une fois la jauge
						    passée au composant partagé, dont l'indicateur a une teinte fixe. Un badge
						    le dit en toutes lettres. */}
						{part > 0.85 && (
							<Badge variant="outline" className="border-amber-500/30 text-amber-500">
								{(part * 100).toFixed(0)} % du budget
							</Badge>
						)}
						<span className="text-muted-foreground">budget {lisible(stats.budget_mo)}</span>
					</div>
					{/* `Progress` du design system — la jauge était une paire de `<div>` avec une
					    largeur en pourcentage, invisible aux lecteurs d'écran (aucun rôle, aucune
					    valeur) là où les trois autres jauges de l'application l'exposent. */}
					<Progress value={Math.min(100, part * 100)} />
					<div className="mt-3 flex items-center gap-2">
						<Button
							size="sm"
							variant="outline"
							onClick={() => void vider()}
							disabled={occupe || stats.octets_mo === 0}
						>
							{occupe ? "…" : "Vider le cache"}
						</Button>
						<Button size="sm" variant="outline" onClick={() => void rafraichir()}>
							Rafraîchir
						</Button>
					</div>
				</>
			) : (
				!erreur && <div className="text-xs text-muted-foreground">Mesure…</div>
			)}
		</div>
	);
}

export default MemoireCard;
