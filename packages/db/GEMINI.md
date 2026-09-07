# Instructions @rose-griffon/db

See root [AGENTS.md](../../AGENTS.md) for global monorepo rules.

## Package Architecture & Constraints
- Specific entrypoints: `/browser`, `/server`, `/service` to prevent bundling Node.js modules into client bundles.
- **NEVER** import `@rose-griffon/db/service` or `@rose-griffon/db/server` in client components.
- Run `bun run types:gen` with Supabase CLI to update `src/types.gen.ts`.
- Always use `getAssetUrl(path)` to generate dynamic image URLs.
