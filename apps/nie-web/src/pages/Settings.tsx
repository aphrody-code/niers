/**
 * `/settings` — l'écran des Options du jeu, avec les réglages d'Inacord dedans.
 *
 * La page ne dessine rien : `SettingsScreen` vient du paquet partagé et demande lui-même à
 * l'hôte ce qu'il sait faire. Ce qui reste ici est ce qui appartient à CET hôte : la langue.
 * Sous Aphrody, changer de langue n'est pas un état local — c'est une navigation entière,
 * servie par `nie-site` sous son préfixe (`/en/settings`, `/ja/settings`). La page aligne donc
 * le réglage `locale` sur l'URL à l'ouverture, et navigue quand « Appliquer » l'a changé.
 */
import {
	type Locale,
	SETTING_FAMILIES,
	type SettingFamily,
	SettingsScreen,
	getSettings,
	setSettings,
} from "@niers/inacord-ui";
import { useEffect } from "react";
import { SETTINGS } from "../entrees";
import { cheminPourEntree, localeDuPrefixe, prefixeDeLocale } from "../routage";

export function Settings({ prefixe, onRetour }: { prefixe: string; onRetour: () => void }) {
	// L'URL fait foi : un réglage `locale` qui contredirait la langue servie afficherait
	// « English » sur une page française.
	const localeServie = localeDuPrefixe(prefixe);
	useEffect(() => {
		if (getSettings().locale !== localeServie) setSettings({ locale: localeServie });
	}, [localeServie]);

	// `?tab=display` ouvre directement un onglet : un lien profond, et le moyen de prouver au
	// `--dump-dom` que chaque famille rend bien ses lignes.
	const tab = new URLSearchParams(window.location.search).get("tab");
	const initialFamily = SETTING_FAMILIES.find((f) => f.id === tab)?.id as
		| SettingFamily
		| undefined;

	return (
		<div style={{ position: "fixed", inset: 0 }}>
			<SettingsScreen
				initialFamily={initialFamily}
				onBack={onRetour}
				onApply={(reglages, changes) => {
					if (!changes.includes("locale")) return;
					const locale = reglages.locale as Locale;
					if (locale === localeServie) return;
					window.location.assign(cheminPourEntree(prefixeDeLocale(locale), SETTINGS));
				}}
			/>
		</div>
	);
}
