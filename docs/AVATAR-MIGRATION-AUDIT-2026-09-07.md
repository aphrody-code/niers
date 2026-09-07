# Avatar editor migration audit

## Sources recovered

The historical Azalée editor was recovered from Git commit 2d825125 (feat(avatar): livrer atelier, imports locaux et audit des liaisons) and its preceding implementation history (19831b52, c4a1da80). The live URL https://azalee.rosegriffon.fr/avatar currently returns the site's 404 page; it is not a recoverable runtime source.

The canonical data source is still live and measurable:

- https://cdn.rosegriffon.fr/avatar/catalog.json: HTTP 200, 450,777 bytes
- 20 categories, 502 parts, 470 palette entries, 38 presets, 8 morphologies
- historical source field: chara_edit + chara_edit_parts_type_config + 20_EDIT/{center,texPartsDefaultPose,editCharaMdlParts}

## Migrated into apps/nie-web

apps/nie-web/src/pages/Avatar.tsx now consumes the resolved catalog through the existing same-origin /assets proxy and provides:

- category tabs and bounded part grids;
- atlas thumbnail URLs derived from the catalog icon names;
- selection state keyed by the game's faceSettingType and part IDs;
- morphology and size controls;
- a bounded GLB assembly URL derived from selected .g4md resources;
- a fallback to the existing chara_edit API when the resolved catalog is unavailable.

No part names, hashes, palette values, or resource paths were copied into the frontend.

## Deliberately not migrated

- Next.js server components, Metadata, revalidate, and runtime;
- Azalée-only UI package, Tailwind classes, and model-viewer loader;
- local avatar project import/export and share-code mutation;
- old page actions and direct filesystem/SQLite assumptions.

Those pieces do not match the current Vite + nie-site contracts and would be a false migration without a dedicated adapter and runtime tests.

## Verification

- bun run --filter nie-web typecheck: passed
- bun run --filter nie-web build: passed, 172 modules
- live catalog probe: 20 categories / 502 parts / 470 colors / 38 presets / 8 morphologies
