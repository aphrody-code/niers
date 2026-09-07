/**
 * Smoke-test de bout en bout : démarre le serveur MCP en sous-process (stdio),
 * s'y connecte avec un vrai client MCP, et appelle chaque outil contre les VRAIES
 * sources (Redis db3, var/niers.sqlite, nie-model-serve, repo).
 *
 *   bun run test/smoke.ts
 *
 * Sortie : un rapport lisible + code de sortie 0 (succès) / 1 (échec).
 */

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const serverEntry = resolve(here, "../src/index.ts");

let passed = 0;
let failed = 0;
let skipped = 0;

function check(label: string, ok: boolean, detail: string): void {
  if (ok) {
    passed++;
    console.log(`  PASS  ${label} — ${detail}`);
  } else {
    failed++;
    console.log(`  FAIL  ${label} — ${detail}`);
  }
}

/**
 * Ressource absente : on ANNONCE le saut. Ni un échec (la machine n'a pas la
 * base, ce n'est pas une régression du serveur), ni un succès silencieux — un
 * saut muet est un faux vert, et c'est exactement ce qui laisse passer une KB
 * vide pour une KB saine.
 */
function skip(label: string, raison: string): void {
  skipped++;
  console.log(`  SKIP  ${label} — ${raison}`);
}

type TextContent = { type: string; text?: string };

function textOf(result: { content: TextContent[]; isError?: boolean }): string {
  return result.content.map((c) => c.text ?? "").join("");
}

async function callJson<T>(
  client: Client,
  name: string,
  args: Record<string, unknown>,
): Promise<{ data: T; isError: boolean; raw: string }> {
  const res = (await client.callTool({ name, arguments: args })) as {
    content: TextContent[];
    isError?: boolean;
  };
  const raw = textOf(res);
  let data: T;
  try {
    data = JSON.parse(raw) as T;
  } catch {
    data = raw as unknown as T;
  }
  return { data, isError: res.isError === true, raw };
}

async function main(): Promise<void> {
  const env: Record<string, string> = {};
  for (const [k, v] of Object.entries(process.env)) if (v !== undefined) env[k] = v;

  const transport = new StdioClientTransport({
    command: process.execPath, // binaire bun
    args: ["run", serverEntry],
    env,
    stderr: "inherit",
  });
  const client = new Client({ name: "niers-game-smoke", version: "0.1.0" });

  console.log("→ démarrage du serveur MCP niers-game (stdio)…\n");
  await client.connect(transport);

  // Liste des outils exposés.
  const tools = await client.listTools();
  const names = tools.tools.map((t) => t.name).sort();
  // 9 outils de données + 5 de pilotage de l'explorateur + le lancement du jeu.
  // Le compte reste écrit EN DUR : dérivé du serveur, il ne pourrait plus rien
  // détecter. Il était resté à 14 alors que `aphrody_api_health` avait porté le
  // registre à 15 — le message de démarrage du serveur annonçait lui aussi 14,
  // et personne ne voyait la contradiction.
  check("listTools", names.length === 16, `${names.length} outils : ${names.join(", ")}`);

  // (1) re_coverage : pct plausible, et total COHÉRENT avec les lignes de `function`.
  // Pas de constante en dur : le nombre de racines `.pdata` dépend du build ciblé et d'un
  // `niers rebuild` (52 783 au 2026-08-10, 55 351 depuis le 2026-08-15). Le figer faisait
  // échouer la smoke sur une KB pourtant saine. Ce qui doit tenir, c'est la cohérence.
  //
  // La disponibilité de la KB est MESURÉE une fois, en l'interrogeant. Sans
  // elle, `re_coverage` rendait `pct=0 total=0` — un ÉCHEC indiscernable d'une
  // régression — et `re_query` faisait carrément planter la smoke sur un
  // `seed.rows` indéfini, avant même d'atteindre les contrôles suivants.
  const kbDisponible = await (async () => {
    const { data, isError } = await callJson<{ rows?: unknown[] }>(client, "re_query", {
      sql: "SELECT 1 AS ok",
    });
    return !isError && Array.isArray(data?.rows);
  })();
  if (!kbDisponible) {
    console.log("  ---   var/niers.sqlite indisponible : les contrôles de reverse sont IGNORÉS, pas réussis.");
  }

  if (!kbDisponible) {
    skip("re_coverage", "var/niers.sqlite absente");
  } else {
    const { data } = await callJson<{
      latest: { pct: number; total_funcs: number; named: number; classified: number };
      function_rows_total: number;
    }>(client, "re_coverage", {});
    const pct = data.latest?.pct ?? 0;
    const total = data.latest?.total_funcs ?? 0;
    check(
      "re_coverage",
      pct >= 75 && pct <= 100 && total > 50_000 && total === data.function_rows_total,
      `pct=${pct.toFixed(2)} total=${total} named=${data.latest?.named} rows=${data.function_rows_total}`,
    );
  }

  // Même traitement que la KB : la disponibilité du VFS est MESURÉE, et son
  // absence s'annonce. Sans cela, `data.matches` était indéfini et faisait
  // PLANTER la smoke à la première recherche — le rapport s'arrêtait là, et les
  // dix contrôles suivants n'étaient jamais exécutés ni comptés.
  const vfsDisponible = await (async () => {
    const { data, isError } = await callJson<{ total_files?: number; matches?: unknown[] }>(client, "vfs_search", {
      query: "c01000010",
      limit: 1,
    });
    return !isError && Array.isArray(data?.matches);
  })();
  if (!vfsDisponible) {
    console.log(
      "  ---   index VFS indisponible : les contrôles VFS et assets sont IGNORÉS, pas réussis. " +
        "Vérifier NIE_GAME_DIR (il doit désigner la racine du jeu, celle qui contient data/cpk_list.cfg.bin).",
    );
  }

  // (2) vfs_search "c01000010" : chemins renvoyés.
  if (!vfsDisponible) {
    skip("vfs_search c01000010", "index VFS indisponible");
  } else {
    const { data } = await callJson<{ total_matches: number; matches: { path: string }[] }>(
      client,
      "vfs_search",
      { query: "c01000010", limit: 5 },
    );
    check(
      "vfs_search c01000010",
      data.total_matches > 0 && data.matches.length > 0,
      `total=${data.total_matches} ex=${data.matches[0]?.path ?? "—"}`,
    );
  }

  // (3) vfs_list "data/dx11/chr" : sous-dossiers listés.
  if (!vfsDisponible) {
    skip("vfs_list data/dx11/chr", "index VFS indisponible");
  } else {
    const { data } = await callJson<{ directories: string[]; total_directories: number; total_files: number }>(
      client,
      "vfs_list",
      { prefix: "data/dx11/chr" },
    );
    check(
      "vfs_list data/dx11/chr",
      data.directories.length > 0,
      `dirs=${data.total_directories} [${data.directories.slice(0, 5).join(", ")}] files=${data.total_files}`,
    );
  }

  // (3b) vfs_stat sur un fichier connu.
  if (!vfsDisponible) {
    skip("vfs_stat cfg.bin", "index VFS indisponible");
  } else {
    const { data } = await callJson<{ kind: string; cpk?: string; decode?: string }>(client, "vfs_stat", {
      path: "data/common/text/en/event/ev20_03200.cfg.bin",
    });
    check(
      "vfs_stat cfg.bin",
      data.kind === "file" && data.decode === "cfg" && !!data.cpk,
      `kind=${data.kind} decode=${data.decode} cpk=${data.cpk?.slice(0, 12)}…`,
    );
  }

  // (3c) vfs_cat : lecture directe des octets d'un asset VFS via CPK.
  if (!vfsDisponible) {
    skip("vfs_cat", "index VFS indisponible");
  } else {
    const { data, isError } = await callJson<{ path: string; cpk: string; size: number; base64?: string }>(client, "vfs_cat", {
      path: "data/common/text/en/event/ev20_03200.cfg.bin",
    });
    check(
      "vfs_cat",
      !isError && data.size > 0 && !!data.base64 && !!data.cpk,
      `size=${data.size} bytes cpk=${data.cpk?.slice(0, 12)}… b64_len=${data.base64?.length ?? 0}`,
    );
  }

  // (4) asset_get cfg.bin -> JSON via model-serve.
  if (!vfsDisponible) {
    skip("asset_get cfg (model-serve)", "index VFS indisponible");
  } else {
    const { data } = await callJson<{ http_status: number; text?: string; url: string }>(client, "asset_get", {
      path: "data/common/text/en/event/ev20_03200.cfg.bin",
      decode: "cfg",
    });
    let jsonOk = false;
    try {
      jsonOk = !!data.text && typeof JSON.parse(data.text) === "object";
    } catch {
      jsonOk = false;
    }
    check(
      "asset_get cfg (model-serve)",
      data.http_status === 200 && jsonOk,
      `http=${data.http_status} json=${jsonOk} url=${data.url}`,
    );
  }

  // (4b) asset_get tex -> PNG. Deux voies possibles selon l'hôte, toutes deux valides :
  //  - `ffi`         : décodage en process par `nie` (CPK montés), l'URL est un `nie://…g4tx` ;
  //  - `model-serve` : service HTTP, et là la convention /tex impose '…/x.png' — jamais
  //                    '…/x.g4tx.png'. C'est ce piège-là que le test doit continuer de garder.
  if (!vfsDisponible) {
    skip("asset_get tex (PNG)", "index VFS indisponible");
  } else {
    const texPath = "data/dx11/menu/200_icon/10_icon_chr/uniform/u040607_20_04_l.g4tx";
    const { data } = await callJson<{
      http_status: number;
      content_type: string | null;
      url: string;
      base64?: string;
      source?: string;
    }>(client, "asset_get", { path: texPath, decode: "tex" });
    const urlOk =
      data.source === "ffi"
        ? data.url.endsWith("/u040607_20_04_l.g4tx")
        : data.url.endsWith("/u040607_20_04_l.png") && !data.url.includes(".g4tx.png");
    check(
      "asset_get tex (PNG)",
      data.http_status === 200 && (data.content_type ?? "").includes("png") && urlOk && (data.base64?.length ?? 0) > 0,
      `source=${data.source} http=${data.http_status} ct=${data.content_type} b64=${data.base64 ? data.base64.length + "c" : "—"} url=${data.url}`,
    );
  }

  // (4c) glob inter-dossiers (sémantique '**') doit matcher.
  if (!vfsDisponible) {
    skip("vfs_search glob **", "index VFS indisponible");
  } else {
    const { data } = await callJson<{ total_matches: number; mode: string }>(client, "vfs_search", {
      query: "data/dx11/chr/**/*.g4tx",
      limit: 3,
    });
    check("vfs_search glob **", data.mode === "glob" && data.total_matches > 0, `mode=${data.mode} total=${data.total_matches}`);
  }

  // (5) re_function + re_query.
  //
  // Le nom cherché est LU DANS LA BASE plutôt que codé en dur : `var/niers.sqlite` est refondée
  // (`niers rebuild`) au fil du reverse, et l'ancien littéral « CScene » n'y a plus aucune
  // fonction — la table `function` en compte 55 351 dont seule une fraction est nommée, et les
  // `CScene*` ne vivent plus que dans `rtti_class`. Le test échouait donc sur l'état de la base,
  // pas sur l'outil qu'il prétend vérifier.
  if (!kbDisponible) {
    skip("re_function", "var/niers.sqlite absente");
  } else {
    const { data: seed } = await callJson<{ rows: { name: string }[] }>(client, "re_query", {
      sql: "SELECT name FROM function WHERE name IS NOT NULL AND length(name) > 4 LIMIT 1",
    });
    const nom = seed.rows?.[0]?.name;
    if (!nom) {
      check("re_function", false, "aucune fonction nommée dans var/niers.sqlite");
    } else {
      // Un fragment plutôt que le nom entier : c'est la recherche par fragment qu'on teste.
      const fragment = nom.slice(0, Math.min(6, nom.length));
      const { data } = await callJson<{
        total_matches: number;
        matches: { name: string; vaddr: string }[];
      }>(client, "re_function", { name: fragment });
      check(
        `re_function « ${fragment} »`,
        data.total_matches > 0 && data.matches[0]?.vaddr?.startsWith("0x") === true,
        `matches=${data.total_matches} top=${data.matches[0]?.name} @ ${data.matches[0]?.vaddr}`,
      );
    }
  }
  if (!kbDisponible) {
    skip("re_query SELECT", "var/niers.sqlite absente");
  } else {
    const { data } = await callJson<{ rows: Record<string, unknown>[] }>(client, "re_query", {
      sql: "SELECT name, subsystem FROM function WHERE name IS NOT NULL LIMIT 5",
    });
    check("re_query SELECT", data.rows.length === 5, `rows=${data.rows.length} ex=${data.rows[0]?.["name"]}`);
  }
  {
    // Sécurité : une mutation doit être refusée.
    const { isError, raw } = await callJson(client, "re_query", { sql: "DELETE FROM function" });
    check("re_query rejette DELETE", isError, raw.slice(0, 60));
  }

  // (6) repo_read + garde anti-traversal.
  //
  // La lecture nominale conditionne les deux gardes qui suivent, et ce n'est
  // pas du confort. Si `NIERS_REPO` ne désigne pas le dépôt, les deux refus
  // passent au vert sur un ENOENT au lieu d'un refus : la garde ne prouve plus
  // rien tout en affichant PASS. On refuse ce faux vert — pas de racine
  // lisible, pas de garde.
  const { data: lecture } = await callJson<{ content?: string; size: number }>(client, "repo_read", {
    path: "docs/PLAN.md",
  });
  const repoLisible = typeof lecture.content === "string" && lecture.size > 0;
  check("repo_read docs/PLAN.md", repoLisible, `size=${lecture.size}`);
  if (!repoLisible) {
    console.log(
      "  ---   racine de dépôt illisible : les gardes anti-traversée passeraient au vert sur un " +
        "ENOENT au lieu d'un refus. Corriger NIERS_REPO (il doit désigner le dépôt niers).",
    );
  }
  if (!repoLisible) {
    skip("repo_read bloque var/", "racine de dépôt illisible — le refus ne serait pas discernable d'un ENOENT");
  } else {
    const { isError, raw } = await callJson(client, "repo_read", { path: "var/niers.sqlite" });
    // Le refus doit venir de la garde, pas d'un fichier manquant.
    check("repo_read bloque var/", isError && raw.includes("répertoire interdit"), raw.slice(0, 70));
  }
  if (!repoLisible) {
    skip("repo_read bloque traversal", "racine de dépôt illisible — le refus ne serait pas discernable d'un ENOENT");
  } else {
    const { isError, raw } = await callJson(client, "repo_read", { path: "../../etc/passwd" });
    check("repo_read bloque traversal", isError && raw.includes("hors du repo"), raw.slice(0, 70));
  }

  await client.close();

  // Les sauts sont COMPTÉS dans le rapport : un `0 FAIL` obtenu en ignorant la
  // moitié des contrôles doit se lire comme tel.
  console.log(`\n=== ${passed} PASS / ${failed} FAIL / ${skipped} SKIP ===`);
  if (failed > 0) process.exit(1);
}

main().catch((e) => {
  console.error("smoke-test : échec inattendu :", e);
  process.exit(1);
});
