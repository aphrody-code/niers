/**
 * La liste des réglages d'une famille, avec sa barre de défilement fine à droite.
 *
 * Le sous-titre de la famille (« Paramètres du jeu ») n'est PAS écrit ici : la bande d'onglets
 * l'affiche déjà sous la rangée, et une information n'apparaît qu'à un seul endroit.
 */
import type React from "react";
import type { Settings } from "../../lib/settings";
import { SettingChoiceList, SettingRow } from "./SettingRow";
import type { SettingDefinition, SettingId } from "./settings-model";

export function SettingList({
	label,
	style,
	definitions,
	values,
	focus,
	listOpen,
	onFocus,
	onChange,
	onOpen,
}: {
	/** Le nom de la famille, pour les technologies d'assistance. */
	label: string;
	style?: React.CSSProperties;
	definitions: readonly SettingDefinition[];
	values: Settings;
	focus: number;
	/** Le choix de la ligne active est-il déplié ? */
	listOpen: boolean;
	onFocus: (index: number) => void;
	onChange: <K extends SettingId>(id: K, value: Settings[K]) => void;
	onOpen: () => void;
}) {
	// Le pouce de la barre : la part visible de la liste. Tout tient à l'écran ici, il est
	// donc plein — la variable existe pour le jour où la liste défilera.
	const thumb = "100%";
	return (
		<section
			className="game-setting-list"
			aria-label={label}
			style={{ ...style, "--game-scroll-thumb": thumb } as React.CSSProperties}
		>
			<ul className="game-setting-list__rows" style={{ display: "contents" }}>
				{definitions.map((def, index) => {
					const focused = index === focus;
					return [
						<SettingRow
							key={def.id}
							def={def}
							value={values[def.id]}
							focused={focused}
							onFocus={() => onFocus(index)}
							onChange={(v) => onChange(def.id, v)}
							onOpen={onOpen}
						/>,
						focused && listOpen && def.kind === "choice" ? (
							<SettingChoiceList
								key={`${def.id}-list`}
								def={def}
								value={values[def.id]}
								onChange={(v) => onChange(def.id, v)}
							/>
						) : null,
					];
				})}
			</ul>
			<div className="game-setting-list__scrollbar" aria-hidden="true" />
		</section>
	);
}
