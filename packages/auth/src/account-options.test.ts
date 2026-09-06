/**
 * Ce qui est vérifié ici : l'effacement des données personnelles part bien, et
 * il part sur la bonne clé.
 *
 * Le défaut d'origine ne se voyait NULLE PART : Better Auth répondait « compte
 * supprimé », la ligne `public."user"` disparaissait, et le profil — nom
 * complet, adresse postale, ville, pays — restait en base sans que rien ne le
 * signale. Un test qui compte les requêtes est le seul endroit où ce silence
 * devient bruyant.
 */
// `vitest`, pas `bun:test` : ce paquet est lancé par `bun --bun vitest run`
// (cf. son `package.json` et son `vitest.config.ts`), comme son voisin
// `index.test.ts`. Importé de `bun:test`, ce fichier ne se CHARGEAIT même pas
// — « Cannot use describe outside of the test runner » — et l'invariant
// ci-dessous, l'effacement effectif des données personnelles, n'a jamais été
// vérifié une seule fois. Le paquet annonçait pourtant « 12 passed ».
import { describe, expect, test } from "vitest";

import { createAccountOptions } from "./account-options";

function options(executees: Array<{ requete: string; parametres: unknown[] }>) {
	return createAccountOptions({
		appName: "Test",
		executerSql: async (requete, parametres) => {
			executees.push({ parametres, requete });
			return null;
		},
		sendEmail: () => undefined,
	});
}

describe("suppression de compte", () => {
	test("un identifiant uuid efface le profil ET la ligne auth", async () => {
		const executees: Array<{ requete: string; parametres: unknown[] }> = [];
		await options(executees).deleteUser.beforeDelete({
			email: "membre@exemple.fr",
			id: "16bf61fd-edb7-445b-9a9f-848e63974ce7",
		});
		expect(executees).toHaveLength(2);
		expect(executees[0]?.requete).toContain("delete from public.profiles");
		expect(executees[0]?.parametres).toEqual(["16bf61fd-edb7-445b-9a9f-848e63974ce7"]);
		expect(executees[1]?.requete).toContain("delete from auth.users");
	});

	test("un identifiant qui n'est pas un uuid retombe sur l'adresse", async () => {
		// Sans ce repli, le `::uuid` lèverait et l'effacement n'aurait pas lieu —
		// en silence, puisque l'erreur est journalisée et non propagée.
		const executees: Array<{ requete: string; parametres: unknown[] }> = [];
		await options(executees).deleteUser.beforeDelete({
			email: "ancien@exemple.fr",
			id: "user_2abcdef",
		});
		expect(executees).toHaveLength(2);
		expect(executees[0]?.parametres).toEqual(["ancien@exemple.fr"]);
		expect(executees[0]?.requete).toContain("where email");
	});

	test("ni uuid ni adresse : on n'efface rien plutôt que d'effacer au hasard", async () => {
		const executees: Array<{ requete: string; parametres: unknown[] }> = [];
		await options(executees).deleteUser.beforeDelete({ id: "inconnu" });
		expect(executees).toHaveLength(0);
	});

	test("une base en panne ne fait pas échouer la suppression du compte", async () => {
		// La suppression du compte prime : un profil déjà absent, ou un Postgres
		// momentanément indisponible, ne doit pas laisser quelqu'un avec un compte
		// qu'il a demandé à supprimer.
		const options = createAccountOptions({
			appName: "Test",
			executerSql: async () => {
				throw new Error("connexion refusée");
			},
			sendEmail: () => undefined,
		});
		await expect(
			options.deleteUser.beforeDelete({
				email: "membre@exemple.fr",
				id: "16bf61fd-edb7-445b-9a9f-848e63974ce7",
			})
		).resolves.toBeUndefined();
	});

	test("sans exécuteur, l'option reste utilisable — c'est l'ancien comportement", async () => {
		const sans = createAccountOptions({ appName: "Test", sendEmail: () => undefined });
		await expect(sans.deleteUser.beforeDelete({ id: "16bf61fd-edb7-445b-9a9f-848e63974ce7" }))
			.resolves.toBeUndefined();
	});
});
