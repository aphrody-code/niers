/**
 * Télécharge les vues d'un modèle du zukan officiel, pour servir de référence de validation.
 *
 * Usage : `bun --bun scripts/validation/download-zukan-reference.ts [q_model|game_id]`
 *
 * L'argument est soit le paramètre `q` du visualiseur, soit un identifiant de jeu
 * (`c01001900`) — dans ce second cas, le `q_model` est retrouvé dans
 * `var/zukan/en/chara_ids.ndjson`, qui porte la correspondance pour tout le corpus. Sans
 * argument, le modèle par défaut est repris.
 *
 * Le script écrivait auparavant dans `outputs/zukan-reference/`, un chemin relatif au
 * répertoire courant : lancé depuis la racine, il créait un second arbre à côté de
 * `var/outputs/zukan-reference/` où vivent les 44 modèles déjà téléchargés. La destination est
 * désormais absolue, dérivée de l'emplacement du script.
 */
import { createHash } from 'node:crypto';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/** La racine du dépôt, déduite de l'emplacement de ce fichier — jamais du répertoire courant. */
const RACINE = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');

/** Le modèle par défaut, conservé pour que le script reste lançable sans argument. */
const Q_DEFAUT = 'hN2cl56NnpyLmo2glpvdxaTdnM_Kz83LyM_P3aKC';

/** Retrouve le `q_model` d'un identifiant de jeu dans l'index du corpus. */
async function qModelPour(gameId: string): Promise<string> {
 const index = Bun.file(`${RACINE}/var/zukan/en/chara_ids.ndjson`);
 if (!(await index.exists())) throw new Error(`Index du zukan absent : ${index.name}`);
 for (const ligne of (await index.text()).split('\n')) {
  if (!ligne.trim()) continue;
  const entree = JSON.parse(ligne) as { game_id: string; q_model?: string; name?: string };
  if (entree.game_id === gameId && entree.q_model) return entree.q_model;
 }
 throw new Error(`Aucun q_model pour ${gameId} dans l'index du zukan`);
}

const argument = process.argv[2] ?? Q_DEFAUT;
const q = /^c\d{8}$/.test(argument) ? await qModelPour(argument) : argument;

const pageUrl = `https://zukan.inazuma.jp/en/chara_model_view/?q=${q}`;
const response = await fetch(pageUrl);
if (!response.ok) throw new Error(`Page: HTTP ${response.status}`);
const html = await response.text();
const model = html.match(/const modelId = '([^']+)'/)?.[1];
const count = Number(html.match(/const imageCount = (\d+)/)?.[1]);
const base = html.match(/return `(https:\/\/[^`]+)` \+ `_r/)?.[1];
if (!model || !base || !count || count > 100) throw new Error('Configuration du visualiseur introuvable');
const root = `${RACINE}/var/outputs/zukan-reference/${model}`;
await Bun.write(`${root}/source.html`, html);
const frames: any[] = [];
for (const suffix of ['', '_fullbody']) {
 for (let index = 0; index < count; index++) {
  const url = `${base}_r${index}${suffix}.png`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  const bytes = Buffer.from(await res.arrayBuffer());
  if (bytes.subarray(0,8).toString('hex') !== '89504e470d0a1a0a') throw new Error(`PNG invalide: ${url}`);
  const name = `${model}_r${index}${suffix}.png`;
  await Bun.write(`${root}/${name}`,bytes);
  frames.push({index, view:suffix ? 'fullbody':'portrait', file:name, url, width:bytes.readUInt32BE(16),height:bytes.readUInt32BE(20),bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')});
 }
}
await Bun.write(`${root}/manifest.json`,JSON.stringify({source:pageUrl,downloadedAt:new Date().toISOString(),model,frames},null,2));
console.log(JSON.stringify({root,count:frames.length,bytes:frames.reduce((n,f)=>n+f.bytes,0),sizes:[...new Set(frames.map(f=>`${f.view}: ${f.width}x${f.height}`))]}));
