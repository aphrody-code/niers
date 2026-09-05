// Modèle pur des `.cfg.bin` décodés (aucun React, aucun Tauri) — toutes les règles risquées de
// l'édition structurée vivent ici, testables et lisibles d'un bloc.
//
// La forme JSON manipulée est celle produite par `nie_explore::bridge` (`vfs_decode_cfgbin`), et
// c'est elle qui repart dans `encode_cfgbin_config`. Deux contraintes en découlent, non
// négociables :
//
//  • T2B (`{"entries":[{name, variables:[{type,value}], children:[…]}]}`) — `value` est TOUJOURS
//    une CHAÎNE, même pour `Int` et `Float` (`t2b_value_to_json`), et `json_to_t2b_value` fait un
//    `.and_then(Value::as_str)` : écrire un nombre JSON brut casse l'encodage. Les variables sont
//    positionnelles, le format ne porte aucun nom de colonne.
//  • RDBN (`{"lists":[{name, typeName, values:[{champ: valeur}]}]}`) — le ré-encodage
//    (`json_to_rdbn_lists`) est un PATCH DE VALEURS : nombre/ordre/noms de listes, nombre de
//    lignes, nombre/noms de champs doivent rester identiques à l'original. D'où l'absence totale
//    d'ajout/suppression ici, et un tri de table qui n'est qu'un ordre d'AFFICHAGE.
//
// Le document n'est pas re-modélisé en objets JS : il est conservé sous forme d'arbre `JNode` qui
// mémorise le TEXTE BRUT des nombres. Sans ça, `JSON.parse`/`JSON.stringify` réécrirait le `1.0`
// d'un flottant RDBN en `1`, et la simple bascule Monaco → table modifierait le document sans que
// personne ne l'ait demandé.

/** Nœud JSON conservant la lexie d'origine des nombres et l'ordre des clés d'objet. */
export type JNode =
  | { t: "null" }
  | { t: "bool"; v: boolean }
  | { t: "num"; raw: string }
  | { t: "str"; v: string }
  | { t: "arr"; items: JNode[] }
  | { t: "obj"; entries: Array<[string, JNode]> };

/** Chemin dans un `JNode` : clé d'objet ou index de tableau. */
export type JsonPath = ReadonlyArray<string | number>;

export type CfgFormat = "t2b" | "rdbn";

const NUMBER_RE = /^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?$/;
/** Forme d'un `RdbnValue::Hash` sérialisé (`"0x%08X"`), seule acceptée par `patch_rdbn_value`. */
const HASH_RE = /^0x[0-9A-Fa-f]{1,8}$/;

// ---------------------------------------------------------------------------
// Analyse / sérialisation fidèles
// ---------------------------------------------------------------------------

/**
 * Analyse un texte JSON en conservant le lexème exact de chaque nombre.
 *
 * @throws Error avec la position du caractère fautif si le texte n'est pas du JSON valide.
 */
export function parseJson(text: string): JNode {
  let i = 0;

  function fail(msg: string): never {
    throw new Error(`JSON invalide en position ${i} : ${msg}`);
  }

  function skipWs(): void {
    while (i < text.length) {
      const c = text.charCodeAt(i);
      if (c === 0x20 || c === 0x0a || c === 0x0d || c === 0x09) i++;
      else break;
    }
  }

  function literal(word: string): void {
    if (text.slice(i, i + word.length) !== word) fail(`« ${word} » attendu`);
    i += word.length;
  }

  function readString(): string {
    const start = i;
    if (text[i] !== '"') fail("chaîne attendue");
    i++;
    while (i < text.length) {
      const c = text[i];
      if (c === "\\") {
        i += 2;
        continue;
      }
      if (c === '"') {
        i++;
        return JSON.parse(text.slice(start, i)) as string;
      }
      i++;
    }
    fail("chaîne non terminée");
  }

  function readNumber(): JNode {
    const start = i;
    if (text[i] === "-") i++;
    while (i < text.length && text[i] >= "0" && text[i] <= "9") i++;
    if (text[i] === ".") {
      i++;
      while (i < text.length && text[i] >= "0" && text[i] <= "9") i++;
    }
    if (text[i] === "e" || text[i] === "E") {
      i++;
      if (text[i] === "+" || text[i] === "-") i++;
      while (i < text.length && text[i] >= "0" && text[i] <= "9") i++;
    }
    const raw = text.slice(start, i);
    if (!NUMBER_RE.test(raw)) fail(`nombre attendu (lu « ${raw || text[start]} »)`);
    return { t: "num", raw };
  }

  function readValue(): JNode {
    skipWs();
    const c = text[i];
    if (c === "{") {
      i++;
      const entries: Array<[string, JNode]> = [];
      skipWs();
      if (text[i] === "}") {
        i++;
        return { t: "obj", entries };
      }
      for (;;) {
        skipWs();
        const key = readString();
        skipWs();
        if (text[i] !== ":") fail("« : » attendu");
        i++;
        entries.push([key, readValue()]);
        skipWs();
        if (text[i] === ",") {
          i++;
          continue;
        }
        if (text[i] === "}") {
          i++;
          return { t: "obj", entries };
        }
        fail("« , » ou « } » attendu");
      }
    }
    if (c === "[") {
      i++;
      const items: JNode[] = [];
      skipWs();
      if (text[i] === "]") {
        i++;
        return { t: "arr", items };
      }
      for (;;) {
        items.push(readValue());
        skipWs();
        if (text[i] === ",") {
          i++;
          continue;
        }
        if (text[i] === "]") {
          i++;
          return { t: "arr", items };
        }
        fail("« , » ou « ] » attendu");
      }
    }
    if (c === '"') return { t: "str", v: readString() };
    if (c === "t") {
      literal("true");
      return { t: "bool", v: true };
    }
    if (c === "f") {
      literal("false");
      return { t: "bool", v: false };
    }
    if (c === "n") {
      literal("null");
      return { t: "null" };
    }
    if (c === undefined) fail("fin de texte inattendue");
    return readNumber();
  }

  const root = readValue();
  skipWs();
  if (i !== text.length) fail("données en trop après la valeur racine");
  return root;
}

/**
 * Rend un `JNode` dans le format EXACT de `JSON.stringify(value, null, 2)` — c'est la forme que
 * produit `DetailPane`/`ConfigEditor` au décodage, donc la seule qui permette une bascule
 * Monaco ↔ table sans réécrire le document.
 */
export function stringifyJson(node: JNode, depth = 0): string {
  switch (node.t) {
    case "null":
      return "null";
    case "bool":
      return node.v ? "true" : "false";
    case "num":
      return node.raw;
    case "str":
      return JSON.stringify(node.v);
    case "arr": {
      if (node.items.length === 0) return "[]";
      const pad = "  ".repeat(depth + 1);
      const body = node.items.map((it) => pad + stringifyJson(it, depth + 1)).join(",\n");
      return `[\n${body}\n${"  ".repeat(depth)}]`;
    }
    case "obj": {
      if (node.entries.length === 0) return "{}";
      const pad = "  ".repeat(depth + 1);
      const body = node.entries
        .map(([k, v]) => `${pad}${JSON.stringify(k)}: ${stringifyJson(v, depth + 1)}`)
        .join(",\n");
      return `{\n${body}\n${"  ".repeat(depth)}}`;
    }
  }
}

// ---------------------------------------------------------------------------
// Accès et écriture immuables
// ---------------------------------------------------------------------------

/** Descend un chemin ; `null` dès que le chemin ne correspond pas à la forme réelle. */
export function getAt(root: JNode, path: JsonPath): JNode | null {
  let cur: JNode = root;
  for (const step of path) {
    if (typeof step === "number") {
      if (cur.t !== "arr") return null;
      const next = cur.items[step];
      if (next === undefined) return null;
      cur = next;
    } else {
      if (cur.t !== "obj") return null;
      const found = cur.entries.find(([k]) => k === step);
      if (!found) return null;
      cur = found[1];
    }
  }
  return cur;
}

/** Remplace le nœud désigné par `path`, en partageant tout le reste de l'arbre. */
export function setAt(root: JNode, path: JsonPath, next: JNode): JNode {
  if (path.length === 0) return next;
  const [step, ...rest] = path;
  if (typeof step === "number") {
    if (root.t !== "arr") throw new Error(`chemin invalide : tableau attendu à l'index ${step}`);
    const child = root.items[step];
    if (child === undefined) throw new Error(`chemin invalide : index ${step} hors du tableau`);
    const items = root.items.slice();
    items[step] = setAt(child, rest, next);
    return { t: "arr", items };
  }
  if (root.t !== "obj") throw new Error(`chemin invalide : objet attendu pour la clé « ${step} »`);
  const idx = root.entries.findIndex(([k]) => k === step);
  if (idx < 0) throw new Error(`chemin invalide : clé « ${step} » absente`);
  const entries = root.entries.slice();
  const [key, child] = entries[idx]!;
  entries[idx] = [key, setAt(child, rest, next)];
  return { t: "obj", entries };
}

// ---------------------------------------------------------------------------
// Document
// ---------------------------------------------------------------------------

export interface CfgDoc {
  format: CfgFormat;
  root: JNode;
  /**
   * `stringifyJson(root)` reproduit le texte d'origine caractère pour caractère. Faux ⇒ la
   * bascule vers les vues structurées doit rester en LECTURE SEULE : ré-écrire le document
   * modifierait des octets que personne n'a édités.
   */
  faithful: boolean;
}

export type ParseResult = { ok: true; doc: CfgDoc } | { ok: false; error: string };

/** Résultat d'une écriture : jamais de valeur devinée, jamais d'écrasement silencieux. */
export type EditResult = { ok: true; root: JNode } | { ok: false; error: string };

/**
 * Analyse le texte JSON affiché par Monaco et vérifie qu'il a bien la forme du format annoncé.
 * Un JSON syntaxiquement invalide (édition en cours) remonte une erreur : c'est l'appelant qui
 * désactive les onglets structurés, aucune tolérance ni réparation ici.
 */
export function parseConfig(text: string, format: CfgFormat): ParseResult {
  let root: JNode;
  try {
    root = parseJson(text);
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
  if (root.t !== "obj") {
    return { ok: false, error: "objet JSON attendu à la racine" };
  }
  const key = format === "t2b" ? "entries" : "lists";
  const list = getAt(root, [key]);
  if (!list || list.t !== "arr") {
    return { ok: false, error: `racine sans tableau « ${key} » (forme ${format.toUpperCase()} attendue)` };
  }
  return { ok: true, doc: { format, root, faithful: stringifyJson(root) === text } };
}

export function serializeConfig(doc: CfgDoc): string {
  return stringifyJson(doc.root);
}

// ---------------------------------------------------------------------------
// T2B — arbre
// ---------------------------------------------------------------------------

/** Une variable T2B telle qu'affichable : `value` est une chaîne par construction du format. */
export interface T2bVarView {
  type: string;
  value: string;
  /** `value` n'est pas une chaîne dans le JSON : `json_to_t2b_value` la rejetterait, donc non éditable. */
  malformed: boolean;
}

export interface T2bEntryView {
  name: string;
  vars: T2bVarView[];
  childCount: number;
}

/** Chemin JSON d'une entrée T2B désignée par ses indices successifs d'enfant. */
export function t2bPath(indices: ReadonlyArray<number>): JsonPath {
  const out: Array<string | number> = ["entries", indices[0] ?? 0];
  for (let d = 1; d < indices.length; d++) out.push("children", indices[d]!);
  return out;
}

function stringField(node: JNode, key: string): string | null {
  const v = getAt(node, [key]);
  return v && v.t === "str" ? v.v : null;
}

function arrayField(node: JNode, key: string): JNode[] {
  const v = getAt(node, [key]);
  return v && v.t === "arr" ? v.items : [];
}

/** Projette une entrée T2B ; `null` si le chemin ne désigne pas une entrée. */
export function t2bEntryAt(root: JNode, indices: ReadonlyArray<number>): T2bEntryView | null {
  const node = getAt(root, t2bPath(indices));
  if (!node || node.t !== "obj") return null;
  return {
    name: stringField(node, "name") ?? "",
    vars: arrayField(node, "variables").map((v) => {
      const value = getAt(v, ["value"]);
      return {
        type: stringField(v, "type") ?? "?",
        value: value === null ? "" : value.t === "str" ? value.v : stringifyJson(value),
        malformed: value === null || value.t !== "str",
      };
    }),
    childCount: arrayField(node, "children").length,
  };
}

/** Ligne d'arbre aplatie — la vue ne construit JAMAIS de DOM récursif (14 448 enfants mesurés). */
export interface TreeRow {
  /** Indices d'enfant depuis la racine. */
  indices: number[];
  /** Clé stable pour la sélection/l'expansion (`0/12/3`). */
  key: string;
  depth: number;
  name: string;
  varCount: number;
  childCount: number;
  expanded: boolean;
}

/**
 * Aplatit l'arbre T2B en une liste de lignes visibles, dans l'ordre du document. Seuls les nœuds
 * dépliés descendent : un fichier à 14 448 enfants ne coûte donc rien tant qu'il est replié.
 */
export function flattenT2b(root: JNode, expanded: ReadonlySet<string>): TreeRow[] {
  const out: TreeRow[] = [];
  const walk = (entries: JNode[], prefix: number[], depth: number): void => {
    for (let i = 0; i < entries.length; i++) {
      const node = entries[i]!;
      if (node.t !== "obj") continue;
      const indices = [...prefix, i];
      const key = indices.join("/");
      const children = arrayField(node, "children");
      const isOpen = expanded.has(key);
      out.push({
        indices,
        key,
        depth,
        name: stringField(node, "name") ?? "(sans nom)",
        varCount: arrayField(node, "variables").length,
        childCount: children.length,
        expanded: isOpen,
      });
      if (isOpen && children.length > 0) walk(children, indices, depth + 1);
    }
  };
  walk(arrayField(root, "entries"), [], 0);
  return out;
}

/** Colonnes déduites d'un groupe d'enfants homogènes — le format ne porte aucun nom de colonne. */
export interface T2bTable {
  columns: Array<{ index: number; type: string }>;
  rowCount: number;
}

/**
 * Détecte la forme tabulaire d'une entrée : au moins deux enfants, tous feuilles, tous porteurs de
 * la même séquence de types de variables. Sans cette homogénéité, une grille mentirait sur
 * l'alignement des colonnes — on retombe alors sur la forme clé/valeur.
 */
export function detectT2bTable(entryNode: JNode): T2bTable | null {
  const children = arrayField(entryNode, "children");
  if (children.length < 2) return null;
  let columns: Array<{ index: number; type: string }> | null = null;
  for (const child of children) {
    if (child.t !== "obj") return null;
    if (arrayField(child, "children").length > 0) return null;
    const vars = arrayField(child, "variables");
    if (vars.length === 0) return null;
    const shape = vars.map((v, index) => ({ index, type: stringField(v, "type") ?? "?" }));
    if (columns === null) {
      columns = shape;
    } else if (columns.length !== shape.length || columns.some((c, k) => c.type !== shape[k]!.type)) {
      return null;
    }
  }
  return columns ? { columns, rowCount: children.length } : null;
}

/** Une entrée T2B dont les enfants forment une table, telle qu'offerte au sélecteur de vue. */
export interface T2bTableCandidate {
  indices: number[];
  key: string;
  /** Chemin lisible (`root/list/data`) — le nom seul est souvent ambigu entre profondeurs. */
  label: string;
  table: T2bTable;
}

/** Parcourt tout l'arbre et remonte les entrées tabulaires, les plus grosses d'abord. */
export function findT2bTables(root: JNode, limit = 64): T2bTableCandidate[] {
  const found: T2bTableCandidate[] = [];
  const walk = (entries: JNode[], prefix: number[], names: string[]): void => {
    for (let i = 0; i < entries.length; i++) {
      const node = entries[i]!;
      if (node.t !== "obj") continue;
      const indices = [...prefix, i];
      const name = stringField(node, "name") ?? "(sans nom)";
      const path = [...names, name];
      const table = detectT2bTable(node);
      if (table) {
        found.push({ indices, key: indices.join("/"), label: path.join(" / "), table });
        continue; // les enfants d'une table sont des feuilles par construction
      }
      walk(arrayField(node, "children"), indices, path);
    }
  };
  walk(arrayField(root, "entries"), [], []);
  found.sort((a, b) => b.table.rowCount - a.table.rowCount);
  return found.slice(0, limit);
}

/** Valeur affichée de la variable `varIndex` du `rowIndex`-ième enfant de l'entrée `indices`. */
export function t2bChildVar(
  root: JNode,
  indices: ReadonlyArray<number>,
  rowIndex: number,
  varIndex: number,
): T2bVarView | null {
  const view = t2bEntryAt(root, [...indices, rowIndex]);
  return view?.vars[varIndex] ?? null;
}

const I32_MIN = -2147483648;
const I32_MAX = 2147483647;

/**
 * Écrit la variable `varIndex` de l'entrée `indices`. La valeur est TOUJOURS écrite en chaîne
 * (`json_to_t2b_value` fait `.and_then(Value::as_str)`), mais elle est validée selon `type` : un
 * `Int` hors i32 ou un `Float` non numérique ferait échouer le ré-encodage côté Rust, autant le
 * refuser ici avec un message qui dit pourquoi.
 */
export function setT2bVar(
  root: JNode,
  indices: ReadonlyArray<number>,
  varIndex: number,
  input: string,
): EditResult {
  const path: JsonPath = [...t2bPath(indices), "variables", varIndex];
  const varNode = getAt(root, path);
  if (!varNode || varNode.t !== "obj") {
    return { ok: false, error: `variable #${varIndex} introuvable à ce chemin` };
  }
  const type = stringField(varNode, "type") ?? "?";
  const trimmed = input.trim();
  if (type === "Int") {
    if (!/^-?\d+$/.test(trimmed)) return { ok: false, error: `Int attendu (entier), lu « ${input} »` };
    const n = Number(trimmed);
    if (n < I32_MIN || n > I32_MAX) return { ok: false, error: `Int hors plage i32 : ${trimmed}` };
  } else if (type === "Float") {
    if (trimmed === "" || !Number.isFinite(Number(trimmed))) {
      return { ok: false, error: `Float attendu (nombre fini), lu « ${input} »` };
    }
  } else if (type !== "String") {
    return { ok: false, error: `type de variable inconnu : ${type} (attendu String/Int/Float)` };
  }
  const value = type === "String" ? input : trimmed;
  return { ok: true, root: setAt(root, [...path, "value"], { t: "str", v: value }) };
}

// ---------------------------------------------------------------------------
// RDBN — listes
// ---------------------------------------------------------------------------

export interface RdbnListView {
  name: string;
  typeName: string;
  rowCount: number;
  /** Noms de champs, dans l'ordre de la PREMIÈRE ligne — l'ordre du document fait foi. */
  columns: string[];
}

export function rdbnLists(root: JNode): RdbnListView[] {
  return arrayField(root, "lists").map((list) => {
    const values = arrayField(list, "values");
    const first = values[0];
    return {
      name: stringField(list, "name") ?? "(sans nom)",
      typeName: stringField(list, "typeName") ?? "",
      rowCount: values.length,
      columns: first && first.t === "obj" ? first.entries.map(([k]) => k) : [],
    };
  });
}

export function rdbnCellPath(listIndex: number, rowIndex: number, field: string): JsonPath {
  return ["lists", listIndex, "values", rowIndex, field];
}

export function rdbnCell(root: JNode, listIndex: number, rowIndex: number, field: string): JNode | null {
  return getAt(root, rdbnCellPath(listIndex, rowIndex, field));
}

/** Rendu texte d'une cellule — même écriture que dans le JSON, pour que l'édition soit prévisible. */
export function formatCell(node: JNode | null): string {
  if (!node) return "";
  switch (node.t) {
    case "null":
      return "";
    case "bool":
      return node.v ? "true" : "false";
    case "num":
      return node.raw;
    case "str":
      return node.v;
    case "arr":
      return node.items.map((it) => formatCell(it)).join(", ");
    case "obj":
      return stringifyJson(node);
  }
}

/**
 * `Blob` et `Invalid` sortent en `null` de `rdbn_value_to_json`, et `patch_rdbn_value` réinjecte
 * alors la valeur d'origine quoi qu'il arrive : ces cellules ne sont pas éditables, les afficher
 * comme telles vaut mieux qu'accepter une saisie sans effet.
 */
export function cellEditable(node: JNode | null): boolean {
  if (!node) return false;
  return node.t !== "null" && node.t !== "obj";
}

/** Étiquette de type déduite de la forme JSON — le format RDBN ne la transporte pas. */
export function cellTypeHint(node: JNode | null): string {
  if (!node) return "?";
  switch (node.t) {
    case "null":
      return "agrégat (non éditable)";
    case "bool":
      return "booléen";
    case "num":
      return node.raw.includes(".") || node.raw.includes("e") || node.raw.includes("E") ? "flottant" : "entier";
    case "str":
      return HASH_RE.test(node.v) ? "hash" : "chaîne";
    case "arr":
      return `${node.items.length} nombres`;
    case "obj":
      return "objet";
  }
}

/**
 * Écrit une cellule RDBN en conservant la FORME de la valeur d'origine. `patch_rdbn_value` part
 * toujours de la variante décodée d'origine et refuse tout changement de type — un booléen
 * remplacé par un nombre, ou un `Rates` de 4 flottants passé à 3, est rejeté côté Rust. Autant le
 * dire ici plutôt que produire un JSON qui fera échouer l'enregistrement.
 */
export function setRdbnCell(
  root: JNode,
  listIndex: number,
  rowIndex: number,
  field: string,
  input: string,
): EditResult {
  const path = rdbnCellPath(listIndex, rowIndex, field);
  const cur = getAt(root, path);
  if (!cur) return { ok: false, error: `champ « ${field} » introuvable à la ligne ${rowIndex}` };
  const trimmed = input.trim();

  switch (cur.t) {
    case "bool": {
      if (trimmed !== "true" && trimmed !== "false") {
        return { ok: false, error: `champ « ${field} » : « true » ou « false » attendu, lu « ${input} »` };
      }
      return { ok: true, root: setAt(root, path, { t: "bool", v: trimmed === "true" }) };
    }
    case "num": {
      if (!NUMBER_RE.test(trimmed)) {
        return { ok: false, error: `champ « ${field} » : nombre JSON attendu, lu « ${input} »` };
      }
      const wasInteger = !/[.eE]/.test(cur.raw);
      if (wasInteger && /[.eE]/.test(trimmed)) {
        return {
          ok: false,
          error: `champ « ${field} » : entier attendu (le champ d'origine est entier), lu « ${input} »`,
        };
      }
      return { ok: true, root: setAt(root, path, { t: "num", raw: trimmed }) };
    }
    case "str": {
      // Un hash RDBN se ré-encode par `strip_prefix("0x")` + `from_str_radix(_, 16)` : hors de
      // cette forme, l'enregistrement échouerait. Un `Condition` en revanche est une chaîne
      // quelconque, dont les espaces de bord peuvent être significatifs — d'où `input` tel quel.
      if (HASH_RE.test(cur.v)) {
        if (!HASH_RE.test(trimmed)) {
          return { ok: false, error: `champ « ${field} » : hash « 0x… » (1 à 8 chiffres hexa) attendu, lu « ${input} »` };
        }
        return { ok: true, root: setAt(root, path, { t: "str", v: trimmed }) };
      }
      return { ok: true, root: setAt(root, path, { t: "str", v: input }) };
    }
    case "arr": {
      const parts = trimmed.split(/[,\s]+/).filter((p) => p.length > 0);
      if (parts.length !== cur.items.length) {
        return {
          ok: false,
          error: `champ « ${field} » : ${cur.items.length} valeur(s) attendue(s), ${parts.length} fournie(s) — la taille d'un tuple RDBN est fixée par le format`,
        };
      }
      const items: JNode[] = [];
      for (const p of parts) {
        if (!NUMBER_RE.test(p)) return { ok: false, error: `champ « ${field} » : « ${p} » n'est pas un nombre JSON` };
        items.push({ t: "num", raw: p });
      }
      return { ok: true, root: setAt(root, path, { t: "arr", items }) };
    }
    default:
      return {
        ok: false,
        error: `champ « ${field} » : agrégat non modélisé par le pont JSON (Blob/Invalid), non éditable`,
      };
  }
}

/** Ordre d'AFFICHAGE des lignes d'une liste RDBN — jamais une permutation du document. */
export function sortRdbnRows(
  root: JNode,
  listIndex: number,
  rowCount: number,
  field: string | null,
  dir: "asc" | "desc",
): number[] {
  const order = Array.from({ length: rowCount }, (_, i) => i);
  if (!field) return order;
  const sign = dir === "asc" ? 1 : -1;
  const keys = order.map((i) => {
    const cell = rdbnCell(root, listIndex, i, field);
    if (cell && cell.t === "num") return Number(cell.raw);
    return formatCell(cell);
  });
  return order.sort((a, b) => {
    const ka = keys[a]!;
    const kb = keys[b]!;
    if (typeof ka === "number" && typeof kb === "number") return sign * (ka - kb);
    return sign * String(ka).localeCompare(String(kb));
  });
}
