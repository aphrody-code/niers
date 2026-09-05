/**
 * Bouton qui lance le flux d'autorisation VRoid Hub.
 *
 * Un `<form method="get">` plutôt qu'un lien : `/api/vroid/login` n'est pas une
 * page mais un point d'entrée serveur qui pose des cookies et redirige vers
 * hub.vroid.com. `next/link` y ferait une navigation cliente (préchargement RSC
 * compris) qui n'a aucun sens sur une route API, et une balise `<a>` nue est
 * refusée par la règle `next/no-html-link-for-pages`. Le formulaire dit
 * exactement ce qui se passe : une action, pas une navigation interne.
 *
 * Composant purement présentationnel, sans état : utilisable depuis un
 * composant serveur comme depuis un îlot client.
 */

export interface BoutonLiaisonProps {
	/** Chemin interne où revenir après l'autorisation (toujours relatif). */
	retour?: string;
	/** Libellé du bouton. */
	libelle?: string;
}

export function BoutonLiaison({ retour = "/vroid", libelle = "Lier mon compte VRoid Hub" }: BoutonLiaisonProps) {
	return (
		<form action="/api/vroid/login" method="get">
			<input name="retour" type="hidden" value={retour} />
			<button
				className="rounded-full bg-primary px-4 py-2 text-sm font-semibold text-on-primary transition hover:opacity-90"
				type="submit"
			>
				{libelle}
			</button>
		</form>
	);
}
