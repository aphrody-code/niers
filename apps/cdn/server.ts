import { createHash, randomBytes } from "node:crypto";
import { tmpdir } from "node:os";
import { join as pathJoin, resolve as pathResolve } from "node:path";

// Serverless (Cloud Run) : binder 0.0.0.0 + lire $PORT (injecté par la plateforme).
// `CDN_PORT` reste honoré pour le legacy/tests (CDN_PORT=0 → port éphémère) ;
// `PORT` (convention Cloud Run) prime quand il est défini.
const PORT = Number(process.env.PORT ?? process.env.CDN_PORT ?? 8804);
const HOST = process.env.CDN_HOST ?? "0.0.0.0";
// FS éphémère uniquement : tout write va sous os.tmpdir() (seul writable en lambda
// / Cloud Run). Aucune persistance garantie — c'est best-effort par instance.
const TMP_BASE = pathJoin(tmpdir(), "rpbey-cdn");
const STORAGE = process.env.CDN_STORAGE ?? pathJoin(TMP_BASE, "images");
const FALLBACK_PUBLIC_BASE = process.env.CDN_PUBLIC_BASE ?? "https://cdn.rosegriffon.fr";
const ASSETS_MANIFEST =
  process.env.CDN_ASSETS_MANIFEST ?? pathJoin(TMP_BASE, "assets-manifest.json");
const VARIANTS_ROOT = process.env.CDN_VARIANTS_ROOT ?? pathJoin(TMP_BASE, "variants");
const API_KEY = process.env.CDN_API_KEY;

// CORS « active cross origin partout » : ces endpoints sont NON-credentialed
// (auth = header `x-api-key`, jamais de cookie) → on autorise n'importe quelle
// origine via `*`, sans aucune allow-list par origine.
const CORS_HEADERS: Record<string, string> = {
  "access-control-allow-origin": "*",
  "access-control-allow-methods": "GET, POST, DELETE, OPTIONS",
  "access-control-allow-headers": "*, Content-Type, X-Api-Key, If-None-Match, Range",
  "access-control-expose-headers": "ETag, Content-Range, Accept-Ranges, Content-Length",
  "access-control-max-age": "86400",
};

/** Fusionne les en-têtes CORS ouverts dans un set d'en-têtes de réponse. */
function withCors(headers: Record<string, string> = {}): Record<string, string> {
  return { ...CORS_HEADERS, ...headers };
}

/** Réponse JSON avec CORS ouvert (helper pour ne jamais oublier les en-têtes). */
function corsJson(body: unknown, init?: ResponseInit): Response {
  const res = Response.json(body, init);
  for (const [k, v] of Object.entries(CORS_HEADERS)) res.headers.set(k, v);
  return res;
}

// Mapping scope → root directory absolu autorisé pour l'API /api/assets/<scope>/<path>.
// Chaque path demandé est résolu puis vérifié pour rester sous le root (anti-traversal).
// Doit rester aligné avec scripts/scan-assets.ts (mêmes 4 sources).
// rg ne sert pas de bibliotheque d'assets scopee via ce CDN (ses assets passent par
// /static, /model et /g4tx). Map vide -> /api/assets/* renvoie 404 proprement.
const ASSET_SCOPES: Record<string, string> = {};

const ASSET_MIME: Record<string, string> = {
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".webp": "image/webp",
  ".gif": "image/gif",
  ".svg": "image/svg+xml",
  ".avif": "image/avif",
  ".ico": "image/x-icon",
  ".mp4": "video/mp4",
  ".webm": "video/webm",
  ".ogg": "audio/ogg",
  ".mp3": "audio/mpeg",
  ".wav": "audio/wav",
  ".glb": "model/gltf-binary",
  ".gltf": "model/gltf+json",
  ".json": "application/json",
  ".woff": "font/woff",
  ".woff2": "font/woff2",
  ".ttf": "font/ttf",
  ".otf": "font/otf",
};

function detectAssetMime(path: string): string {
  const dot = path.lastIndexOf(".");
  if (dot < 0) return "application/octet-stream";
  const ext = path.slice(dot).toLowerCase();
  return ASSET_MIME[ext] ?? "application/octet-stream";
}

// Cache d'existence des variants AVIF/WebP : évite N stat() par requête image.
// TTL 60 s — suffisant pour que precompress-assets.ts publie ses variants
// sans que le cache serve de vieilles réponses négatives trop longtemps.
// Taille max 8192 entries (~4 scopes × ~2048 assets max).
const VARIANT_TTL_MS = 60_000;
const VARIANT_CACHE_MAX = 8192;
const variantCache = new Map<string, { exists: boolean; ts: number }>();
function variantCacheGet(path: string): boolean | undefined {
  const e = variantCache.get(path);
  if (!e) return undefined;
  if (Date.now() - e.ts > VARIANT_TTL_MS) {
    variantCache.delete(path);
    return undefined;
  }
  return e.exists;
}
function variantCacheSet(path: string, exists: boolean): void {
  if (variantCache.size >= VARIANT_CACHE_MAX) {
    // Eviction FIFO : supprimer la première entrée insérée
    variantCache.delete(variantCache.keys().next().value as string);
  }
  variantCache.set(path, { exists, ts: Date.now() });
}
async function fileExistsCached(path: string): Promise<boolean> {
  const cached = variantCacheGet(path);
  if (cached !== undefined) return cached;
  const exists = await Bun.file(path).exists();
  variantCacheSet(path, exists);
  return exists;
}

// Manifest cache : on lit le fichier au boot + sur stat-mtime change.
interface ManifestCache {
  bytes: Uint8Array;
  etag: string;
  mtimeMs: number;
}

let manifestCache: ManifestCache | null = null;

async function loadManifestCache(force = false): Promise<ManifestCache | null> {
  const file = Bun.file(ASSETS_MANIFEST);
  if (!(await file.exists())) return null;
  const stat = await file.stat();
  if (!force && manifestCache && manifestCache.mtimeMs === stat.mtimeMs) {
    return manifestCache;
  }
  const buf = new Uint8Array(await file.arrayBuffer());
  const etag = `"${createHash("sha1").update(buf).digest("hex").slice(0, 16)}"`;
  manifestCache = { bytes: buf, etag, mtimeMs: stat.mtimeMs };
  return manifestCache;
}

await loadManifestCache();

// Hôtes autorisés à servir des URLs publiques. nginx reverse-proxy fait
// croire au backend qu'il est appelé via l'un de ces vhosts (X-Forwarded-Host).
// On retourne dynamiquement l'URL publique selon le Host reçu :
//   - upload via cdn.rosegriffon.fr  → URL retournée = https://cdn.rosegriffon.fr/<id>
//   - upload via cdn.rpbey.fr        → URL retournée = https://cdn.rpbey.fr/<id>
// Achillea + RG continuent de pointer cdn.rosegriffon.fr ; gacha + RPB pointent
// cdn.rpbey.fr — chacun récupère l'URL canonique pour son propre domaine.
const ALLOWED_PUBLIC_HOSTS = new Set(["cdn.rosegriffon.fr", "cdn.rpbey.fr"]);

function publicBaseFor(req: Request): string {
  const xfh = req.headers.get("x-forwarded-host");
  const host = (xfh ?? req.headers.get("host") ?? "").toLowerCase();
  if (ALLOWED_PUBLIC_HOSTS.has(host)) return `https://${host}`;
  return FALLBACK_PUBLIC_BASE;
}

if (!API_KEY) {
  console.error("CDN_API_KEY required");
  process.exit(1);
}

await Bun.$`mkdir -p ${STORAGE}`.quiet();

const MIME_TO_EXT: Record<string, string> = {
  "image/png": "png",
  "image/jpeg": "jpg",
  "image/jpg": "jpg",
  "image/webp": "webp",
  "image/gif": "gif",
  "image/svg+xml": "svg",
};

function genId(ext: string): string {
  return `${randomBytes(8).toString("hex")}.${ext}`;
}

function unauthorized() {
  return corsJson({ error: "Unauthorized" }, { status: 401 });
}

async function serveAssetManifest(req: Request): Promise<Response> {
  const cache = await loadManifestCache();
  if (!cache) {
    return corsJson(
      { error: "Manifest not generated yet. Run scripts/scan-assets.ts" },
      { status: 503 },
    );
  }
  const ifNone = req.headers.get("if-none-match");
  if (ifNone && ifNone === cache.etag) {
    return new Response(null, { status: 304, headers: withCors({ ETag: cache.etag }) });
  }
  return new Response(cache.bytes, {
    status: 200,
    headers: withCors({
      "content-type": "application/json; charset=utf-8",
      "content-length": String(cache.bytes.byteLength),
      "cache-control": "public, max-age=300, must-revalidate",
      etag: cache.etag,
    }),
  });
}

/**
 * Catégorisation par extension pour cache TTL + headers spécifiques.
 *   - `image-raster`  : png/jpg/webp/avif → 30j + content negotiation AVIF/WebP
 *   - `image-vector`  : svg → 1y immutable (pré-compressé brotli côté nginx)
 *   - `video`         : mp4/webm → 30j, range support natif Bun.file
 *   - `audio`         : mp3/ogg/wav → 30j, range natif
 *   - `font`          : woff2 → 1y immutable, CORS *
 *   - `model`         : glb/gltf → 30j
 *   - `data`          : json → 5min must-revalidate (manifests volatiles)
 *   - `default`       : 1d + swr 1 semaine
 */
type AssetKind =
  | "image-raster"
  | "image-vector"
  | "video"
  | "audio"
  | "font"
  | "model"
  | "data"
  | "default";

function classify(path: string): AssetKind {
  const dot = path.lastIndexOf(".");
  if (dot < 0) return "default";
  const ext = path.slice(dot).toLowerCase();
  if (
    ext === ".png" ||
    ext === ".jpg" ||
    ext === ".jpeg" ||
    ext === ".webp" ||
    ext === ".avif" ||
    ext === ".gif"
  )
    return "image-raster";
  if (ext === ".svg") return "image-vector";
  if (ext === ".mp4" || ext === ".webm" || ext === ".mov") return "video";
  if (ext === ".mp3" || ext === ".ogg" || ext === ".oga" || ext === ".wav") return "audio";
  if (ext === ".woff2" || ext === ".woff" || ext === ".ttf" || ext === ".otf") return "font";
  if (ext === ".glb" || ext === ".gltf") return "model";
  if (ext === ".json") return "data";
  return "default";
}

interface CacheProfile {
  cacheControl: string;
  vary?: string;
  extra?: Record<string, string>;
}

function cacheProfileFor(kind: AssetKind): CacheProfile {
  switch (kind) {
    case "image-raster":
      return {
        // content negotiation server-side → varier sur Accept
        cacheControl: "public, max-age=2592000, stale-while-revalidate=604800",
        vary: "Accept",
      };
    case "image-vector":
      return {
        cacheControl: "public, max-age=31536000, immutable",
      };
    case "video":
    case "audio":
      return {
        cacheControl: "public, max-age=2592000",
        extra: { "accept-ranges": "bytes" },
      };
    case "font":
      return {
        cacheControl: "public, max-age=31536000, immutable",
        extra: {
          "access-control-allow-origin": "*",
          "timing-allow-origin": "*",
        },
      };
    case "model":
      return { cacheControl: "public, max-age=2592000" };
    case "data":
      return {
        cacheControl: "public, max-age=300, must-revalidate",
        vary: "Accept-Encoding",
      };
    default:
      return {
        cacheControl: "public, max-age=86400, stale-while-revalidate=604800",
      };
  }
}

/**
 * Content negotiation pour images raster : si le client envoie
 * `Accept: image/avif` ou `image/webp` et qu'un variant existe, on le sert.
 *
 * Lookup ordre :
 *   1. `<VARIANTS_ROOT>/<scope>/<rel>.avif` ou `.webp`  (off-tree, généré
 *       par scripts/precompress-assets.ts — pas committé)
 *   2. `<source>.avif` ou `<source>.webp`               (in-tree, optionnel)
 *
 * Avantage : les ~900 variants AVIF (gain bandwidth ~60 MB) ne polluent pas
 * `apps/<scope>/...` ni le repo git.
 */
async function negotiateImageVariant(
  target: string,
  rel: string,
  scope: string,
  accept: string,
): Promise<{ path: string; rel: string } | null> {
  const wants = accept.toLowerCase();
  const wantsAvif = wants.includes("image/avif");
  const wantsWebp = wants.includes("image/webp");
  if (!wantsAvif && !wantsWebp) return null;

  const dot = target.lastIndexOf(".");
  if (dot < 0) return null;
  const base = target.slice(0, dot);
  const relBase = rel.slice(0, rel.lastIndexOf("."));
  const exts: ("avif" | "webp")[] = [];
  if (wantsAvif) exts.push("avif");
  if (wantsWebp) exts.push("webp");

  for (const ext of exts) {
    // 1. Variants off-tree (généré par precompress) — lookup caché 60 s
    const offTree = `${VARIANTS_ROOT}/${scope}/${rel}.${ext}`;
    if (await fileExistsCached(offTree)) {
      return { path: offTree, rel: `${relBase}.${ext}` };
    }
    // 2. Sibling in-tree (si déposé manuellement à côté de la source)
    const sibling = `${base}.${ext}`;
    if (await fileExistsCached(sibling)) {
      return { path: sibling, rel: `${relBase}.${ext}` };
    }
  }
  return null;
}

async function serveAssetFile(req: Request, url: URL): Promise<Response> {
  // path = /api/assets/<scope>/<rest>
  const segments = url.pathname.split("/").filter(Boolean);
  if (segments.length < 4) return new Response("Not Found", { status: 404, headers: withCors() });
  const scope = segments[2] ?? "";
  const rel = segments.slice(3).join("/");
  const root = ASSET_SCOPES[scope];
  if (!root) {
    return corsJson({ error: `unknown scope: ${scope}` }, { status: 404 });
  }

  // Anti path-traversal : resolve absolu + check stay under root
  const target = pathResolve(root, rel);
  if (!target.startsWith(root + "/") && target !== root) {
    return corsJson({ error: "forbidden" }, { status: 403 });
  }

  let actualPath = target;
  let actualRel = rel;
  const kind = classify(rel);

  // Content negotiation AVIF/WebP pour images raster
  if (kind === "image-raster") {
    const accept = req.headers.get("accept") ?? "";
    const variant = await negotiateImageVariant(target, rel, scope, accept);
    if (variant) {
      actualPath = variant.path;
      actualRel = variant.rel;
    }
  }

  const file = Bun.file(actualPath);
  if (!(await file.exists()))
    return new Response("Not Found", { status: 404, headers: withCors() });
  const stat = await file.stat();
  if (!stat.isFile()) return new Response("Not Found", { status: 404, headers: withCors() });
  const etag = `"${stat.size.toString(36)}-${stat.mtimeMs.toString(36)}"`;
  const ifNone = req.headers.get("if-none-match");
  if (ifNone && ifNone === etag) {
    return new Response(null, { status: 304, headers: withCors({ ETag: etag }) });
  }

  const profile = cacheProfileFor(kind);
  const headers: Record<string, string> = withCors({
    "content-type": detectAssetMime(actualRel),
    "content-length": String(stat.size),
    "cache-control": profile.cacheControl,
    etag,
  });
  if (profile.vary) headers.vary = profile.vary;
  if (profile.extra) Object.assign(headers, profile.extra);

  // Range request handling (vidéo/audio) — Bun.file slice gère Content-Range
  const range = req.headers.get("range");
  if ((kind === "video" || kind === "audio") && range) {
    const m = /^bytes=(\d*)-(\d*)$/.exec(range);
    if (m) {
      const start = m[1] ? Number(m[1]) : 0;
      const end = m[2] ? Number(m[2]) : stat.size - 1;
      if (start <= end && end < stat.size) {
        const chunk = file.slice(start, end + 1);
        headers["content-range"] = `bytes ${start}-${end}/${stat.size}`;
        headers["content-length"] = String(end - start + 1);
        return new Response(chunk, { status: 206, headers });
      }
    }
  }

  return new Response(file, { status: 200, headers });
}

/** Préflight OPTIONS universel : 204 No Content + CORS ouvert pour toute origine. */
function preflight(): Response {
  return new Response(null, { status: 204, headers: withCors() });
}

const server = Bun.serve({
  hostname: HOST,
  port: PORT,
  routes: {
    "/health": {
      OPTIONS: preflight,
      GET: () =>
        corsJson({
          ok: true,
          storage: STORAGE,
          assetsManifest: manifestCache
            ? {
                bytes: manifestCache.bytes.byteLength,
                etag: manifestCache.etag,
              }
            : null,
        }),
    },

    "/api/assets/manifest": {
      OPTIONS: preflight,
      GET: (req: Request) => serveAssetManifest(req),
    },

    "/api/assets/manifest/refresh": {
      OPTIONS: preflight,
      POST: async (req: Request) => {
        if (req.headers.get("x-api-key") !== API_KEY) return unauthorized();
        const cache = await loadManifestCache(true);
        if (!cache) return corsJson({ ok: false, error: "missing" }, { status: 503 });
        return corsJson({
          ok: true,
          bytes: cache.bytes.byteLength,
          etag: cache.etag,
          mtimeMs: cache.mtimeMs,
        });
      },
    },

    "/upload": {
      OPTIONS: preflight,
      POST: async (req) => {
        if (req.headers.get("x-api-key") !== API_KEY) return unauthorized();

        const ct = req.headers.get("content-type") ?? "application/octet-stream";
        const ext = MIME_TO_EXT[ct.split(";")[0].trim().toLowerCase()];
        if (!ext) return corsJson({ error: `Unsupported content-type: ${ct}` }, { status: 415 });

        const id = genId(ext);
        const path = `${STORAGE}/${id}`;

        const body = await req.arrayBuffer();
        if (body.byteLength === 0) return corsJson({ error: "Empty body" }, { status: 400 });
        if (body.byteLength > 25 * 1024 * 1024)
          return corsJson({ error: "File too large (>25MB)" }, { status: 413 });

        await Bun.write(path, body);

        const base = publicBaseFor(req);
        return corsJson({ id, url: `${base}/${id}` });
      },
    },

    "/images/:id": {
      OPTIONS: preflight,
      DELETE: async (req) => {
        if (req.headers.get("x-api-key") !== API_KEY) return unauthorized();

        const id = (req as Request & { params: { id: string } }).params.id;
        if (!/^[a-f0-9]{16}\.(png|jpg|webp|gif|svg)$/.test(id))
          return corsJson({ error: "Invalid id" }, { status: 400 });

        const file = Bun.file(`${STORAGE}/${id}`);
        if (!(await file.exists())) return corsJson({ success: false }, { status: 404 });

        await Bun.$`rm -f ${STORAGE}/${id}`.quiet();
        return corsJson({ success: true });
      },
    },
  },

  fetch(req) {
    // Préflight CORS universel sur les paths non capturés par `routes`.
    if (req.method === "OPTIONS") return preflight();
    // Fallback wildcard pour les paths non capturés par routes — utilisé
    // par /api/assets/<scope>/<arbitrary/sub/path.ext>.
    const url = new URL(req.url);
    if (url.pathname.startsWith("/api/assets/")) {
      return serveAssetFile(req, url);
    }
    return new Response("Not Found", { status: 404, headers: withCors() });
  },
});

console.warn(`cdn listening on http://${server.hostname}:${server.port}`);

// Arrêt gracieux (Cloud Run / SIGTERM) : on draine puis on coupe le serveur.
function shutdown(signal: string): void {
  console.warn(`cdn received ${signal}, shutting down`);
  void server.stop(/* closeActiveConnections */ false);
  process.exit(0);
}
process.on("SIGTERM", () => shutdown("SIGTERM"));
process.on("SIGINT", () => shutdown("SIGINT"));
