import { creerWebSource, type VueCatalogue } from "@niers/asset-source";
import { type SanteApi, sante } from "@niers/asset-source/nie-site";
import { AssetSourceProvider, useCapacites, useErreurSource } from "@niers/inacord-ui";
import "@niers/inacord-ui/shell/game-tokens.css";
import { useEffect, useMemo, useState } from "react";
import { EXPLORATEUR, entreesMenu } from "./entrees";
import { Catalogue } from "./pages/Catalogue";
import { EcranSecondaire, Note } from "./pages/Ecran";
import { Explorateur } from "./pages/Explorateur";
import { MenuPrincipal } from "./pages/MenuPrincipal";
import { ACCUEIL, cheminPourEntree, entreeDemandee, separerLangue } from "./routage";

/**
 * Coquille d'Aphrody.
 *
 * L'hôte n'a qu'un rôle : construire sa source et la monter. Tout le reste vient de
 * `@niers/inacord-ui` — la même interface que celle d'Inacord, dans la DA du menu principal du
 * jeu. C'est le point de la manœuvre.
 */
export function App() {
	// La source ne dépend d'aucun état : la mémoriser évite de relancer la mesure des capacités
	// à chaque rendu.
	const source = useMemo(() => creerWebSource(), []);
	return (
		<AssetSourceProvider source={source}>
			<Site />
		</AssetSourceProvider>
	);
}

/**
 * L'application : une entrée courante, un écran.
 *
 * Il n'y a plus qu'UNE coquille. L'accueil est le menu principal reconstruit, les autres écrans
 * sont le même décor avec la rangée d'entrées réduite à une barre — voir `pages/Ecran.tsx` pour
 * ce que cette unification a remplacé.
 */
function Site() {
	const capacites = useCapacites();
	const erreurSource = useErreurSource();
	const [etat, setEtat] = useState<SanteApi | null>(null);

	// Le prefixe de langue de l'URL courante. Il ne change pas pendant la session : changer de
	// langue est une navigation entiere, servie par nie-site, pas un changement d'etat local.
	const prefixe = useMemo(() => separerLangue(window.location.pathname).prefixe, []);

	// Les entrees reconnues dans l'URL. L'accueil n'en fait PAS partie : il vit a la racine, et
	// `entreeDemandee` rend `null` pour elle — y ajouter `accueil` creerait un second chemin
	// vers la meme page.
	const entrees = useMemo(() => entreesMenu(etat).map((e) => e.vue), [etat]);

	// L'entree courante vit dans l'URL, pas seulement en memoire : sans cela, un lien vers un
	// catalogue ne mene qu'a l'accueil, le bouton « precedent » quitte le site, et un
	// rechargement perd ou l'on etait.
	const [vue, setVueEtat] = useState<string>(() => {
		const routeServeur = document.getElementById("racine")?.dataset.route;
		return entreeDemandee(DEPART, window.location, routeServeur) ?? ACCUEIL;
	});

	/** Change de vue ET d'URL, sans recharger la page. */
	const setVue = (suivante: string) => {
		setVueEtat(suivante);
		const url = new URL(window.location.href);
		url.pathname = cheminPourEntree(prefixe, suivante);
		window.history.pushState({ vue: suivante }, "", url);
	};

	// Le bouton « precedent » doit ramener a la vue precedente, pas sortir du site. Une URL qui
	// ne designe aucune entree est l'accueil — c'est aussi ce qui ramene au menu depuis un
	// catalogue.
	useEffect(() => {
		const surRetour = () => {
			setVueEtat(entreeDemandee(entrees, window.location) ?? ACCUEIL);
		};
		window.addEventListener("popstate", surRetour);
		return () => window.removeEventListener("popstate", surRetour);
	}, [entrees]);

	useEffect(() => {
		const ac = new AbortController();
		sante(ac.signal)
			.then(setEtat)
			.catch(() => {
				/* l'erreur est déjà portée par le fournisseur */
			});
		return () => ac.abort();
	}, []);

	// Le catalogue est-il consultable ? `capacites` vaut `null` tant que la mesure court : on
	// distingue « on ne sait pas encore » de « rien ne marche », au lieu d'afficher des vues
	// vides pendant la premiere seconde.
	const pret = Boolean(capacites?.vfs);

	// L'accueil occupe tout l'écran : le menu principal EST la page, pas un panneau dedans.
	if (vue === ACCUEIL) {
		return (
			// `fixed; inset: 0` et non `height: 100vh` : la seconde forme depend de la hauteur de
			// tous ses ancetres, et il suffit qu'un seul ne la propage pas pour que la zone mesuree
			// soit plus courte que la fenetre. Le canevas se met alors a l'echelle d'une hauteur
			// qu'il n'a pas, et laisse une bande vide en bas — sans qu'aucune valeur soit fausse.
			<div style={{ position: "fixed", inset: 0, background: "var(--jeu-ciel-clair)" }}>
				<MenuPrincipal
					vue={vue}
					onChoisir={setVue}
					etat={etat}
					pret={pret}
					panne={Boolean(erreurSource)}
				/>
			</div>
		);
	}

	return (
		<EcranSecondaire vue={vue} onChoisir={setVue} etat={etat}>
			{erreurSource ? (
				// Le detail technique de la panne ne s'affiche pas : il ne dit rien a qui consulte
				// le site, et le seul geste utile — reessayer — ne depend pas de lui.
				<Note ton="alerte">
					Le site ne parvient pas à joindre ses ressources. Réessayez dans un instant.
				</Note>
			) : !capacites ? (
				<Note>Chargement…</Note>
			) : !pret ? (
				<Note>Le catalogue est en cours de préparation. Il s'affichera dès qu'il sera prêt.</Note>
			) : vue === EXPLORATEUR ? (
				<Explorateur />
			) : (
				// La vue vient de l'URL, et l'URL n'a ete acceptee que parce qu'elle figure dans
				// les entrees connues — celles du serveur, ou les quatre catalogues qu'il publie
				// dans son document. Le type ne couvre que ces quatre-la ; si le serveur en
				// annonce un cinquieme, la page le demande sous SON nom plutot que de le refuser.
				<Catalogue vue={vue as VueCatalogue} />
			)}
		</EcranSecondaire>
	);
}

/**
 * Les entrées reconnues au tout premier rendu, avant la réponse du serveur.
 *
 * Elles servent à lire l'URL d'arrivée : sans elles, ouvrir `/textures` directement afficherait
 * l'accueil le temps d'un aller-retour réseau, puis basculerait — un saut visible qu'aucune
 * donnée ne justifie.
 */
const DEPART = entreesMenu(null).map((e) => e.vue);
