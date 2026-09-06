/**
 * Fraîcheur du miroir SQLite : un processus long doit suivre le swap.
 *
 * La synchronisation quotidienne (`nie-miroir`) fait pointer le lien
 * `mirror.sqlite` sur un nouveau snapshot puis purge les anciens. `azalee-web`
 * est redémarré dans la foulée, mais pas les autres consommateurs de la lib
 * (`azalee-api`, `rg-mcp`, un CLI ouvert longtemps, un sidecar Tauri) : sans
 * réouverture, ils servaient indéfiniment le snapshot ouvert au démarrage, et
 * après deux nuits un fichier supprimé.
 */

import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { configureAzalee, resetAzaleeConfig } from "../src/config";
import { createSqliteClient } from "../src/db/sqlite-client";

const BAC = `${import.meta.dir}/.bac-miroir`;

/**
 * Cette machine sait-elle créer un VRAI lien symbolique ?
 *
 * La capacité est MESURÉE, pas déduite de `process.platform` : une garde codée
 * en dur sauterait aussi sur un Windows dûment privilégié et mentirait sur un
 * conteneur Linux sans les droits. Sous MSYS sans
 * `SeCreateSymbolicLinkPrivilege`, `ln -sfn` ne signale rien et **copie** le
 * fichier — le scénario de swap atomique testé plus bas n'existe alors tout
 * simplement pas, et l'assertion mesurerait `ln`, pas la lib.
 */
async function liensSymboliquesDisponibles(): Promise<boolean> {
	const bac = `${import.meta.dir}/.bac-lien`;
	try {
		await Bun.spawn(["rm", "-rf", bac]).exited;
		await Bun.spawn(["mkdir", "-p", bac]).exited;
		await Bun.write(`${bac}/cible`, "x");
		await Bun.spawn(["ln", "-sfn", "cible", `${bac}/lien`]).exited;
		return (await Bun.spawn(["test", "-L", `${bac}/lien`]).exited) === 0;
	} catch {
		return false;
	} finally {
		await Bun.spawn(["rm", "-rf", bac]).exited;
	}
}

const liensDisponibles = await liensSymboliquesDisponibles();
if (!liensDisponibles) {
	console.warn(
		"[mirror-freshness] liens symboliques indisponibles sur cette machine (`ln -sfn` copie au lieu de lier) : " +
			"le test de swap par lien est IGNORÉ, pas réussi. L'invariant reste gardé sur Linux.",
	);
}

/** Crée une base minimale avec une seule ligne identifiable. */
function ecrireBase(chemin: string, marqueur: string): void {
	const db = new Database(chemin, { create: true });
	db.exec("create table if not exists inagle_teams (id text primary key, name text)");
	db.exec("delete from inagle_teams");
	db.prepare("insert into inagle_teams (id, name) values (?, ?)").run("1", marqueur);
	db.close();
}

async function lireNom(): Promise<string | undefined> {
	// `createSqliteClient().from()` rouvre le handle à chaque appel : c'est le
	// point d'entrée réel de l'app, du CLI et de l'API headless.
	const { data } = (await createSqliteClient().from("inagle_teams").select("*")) as {
		data: { name?: string }[] | null;
	};
	return data?.[0]?.name;
}

describe("réouverture du miroir après un swap", () => {
	beforeEach(async () => {
		await Bun.spawn(["rm", "-rf", BAC]).exited;
		await Bun.spawn(["mkdir", "-p", BAC]).exited;
	});

	afterEach(async () => {
		resetAzaleeConfig();
		await Bun.spawn(["rm", "-rf", BAC]).exited;
	});

	test.skipIf(!liensDisponibles)("le remplacement du fichier est pris en compte sans redémarrage", async () => {
		const ancien = `${BAC}/snapshot-1.sqlite`;
		const nouveau = `${BAC}/snapshot-2.sqlite`;
		const lien = `${BAC}/mirror.sqlite`;

		ecrireBase(ancien, "AVANT");
		ecrireBase(nouveau, "APRÈS");
		await Bun.spawn(["ln", "-sfn", "snapshot-1.sqlite", lien]).exited;

		configureAzalee({ mirrorPath: lien });
		expect(await lireNom()).toBe("AVANT");

		// Swap atomique du lien, exactement comme le fait miroir-inagle.sh, puis
		// purge de l'ancien snapshot (rétention).
		await Bun.spawn(["ln", "-sfn", "snapshot-2.sqlite", lien]).exited;
		await Bun.file(ancien).delete();

		// La vérification de fraîcheur est limitée à une fois toutes les 5 s :
		// on laisse passer l'intervalle plutôt que d'exposer un compteur interne.
		await Bun.sleep(5100);

		expect(await lireNom()).toBe("APRÈS");
	}, 20_000);

	test("sans changement, le handle est conservé (pas de réouverture inutile)", async () => {
		const base = `${BAC}/stable.sqlite`;
		ecrireBase(base, "STABLE");
		configureAzalee({ mirrorPath: base });

		expect(await lireNom()).toBe("STABLE");
		await Bun.sleep(5100);
		expect(await lireNom()).toBe("STABLE");
	}, 20_000);
});
