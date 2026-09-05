"use client";

import { useMemo, useState } from "react";
import { Atelier } from "./Atelier";
import { composerUrlAvatar } from "./composition";
import type { EtatAvatar } from "./projet";
import type { Catalogue } from "./types";
export { MORPHOLOGIES } from "./libelles";

/** La recette reste assemblée par NIE ; l’atelier ne duplique ni parseur ni moteur. */
export function Editeur({ catalogue, cdn }: { catalogue: Catalogue; cdn: string }) {
	const [avatar, setAvatar] = useState<EtatAvatar>({ choix: {}, valeurs: {}, champs: {}, genre: 0, morphologie: 0 });
	const modeleUrl = useMemo(() => composerUrlAvatar(catalogue, cdn, avatar), [catalogue, cdn, avatar]);
	return <Atelier catalogue={catalogue} avatar={avatar} restaurer={setAvatar} url={modeleUrl} cdn={cdn} />;
}
