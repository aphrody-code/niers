/**
 * `/keshin` — conservée pour les liens existants, redirige vers la page qui la remplace.
 *
 * L'ancienne cible était `/modeles/keshin` ; les collections média ont migré vers l'explorateur
 * de bureau (`docs/MIGRATION-EXPLORATEUR.md`) et cette URL ne mène plus nulle part sur le wiki.
 * Une redirection permanente vers une page morte est pire qu'un 404 : le navigateur la met en
 * cache. Elle pointe donc vers la section du wiki qui traite encore des esprits guerriers.
 */
import { permanentRedirect } from "next/navigation";

export default function KeshinPage(): never {
	permanentRedirect("/aura/esprits-guerriers");
}
