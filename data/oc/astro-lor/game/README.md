# Game integration contract

This directory contains machine-readable integration metadata for the OC. It is not a copy of
the game data and it is not an installable mod yet.

The contract keeps three namespaces separate:

- `internal_code` is the character code used by the VFS (`c...`), model folders and textures;
- `chara_param_id` is the hash joining the character parameter row to the other game tables;
- every versioned `*.cfg.bin` path is copied from the measured VFS inventory and must be refreshed
  when the target game build changes.

Lua is described as a runtime contract, not as a per-character asset. A character does not need a
new `.lua.bin` to be selectable or playable. If a future story/menu behavior needs one, it must be
compiled Lua 5.2 bytecode under the game's `data/common/script/lua/<category>/` tree and carry the
versioned filename convention recorded in `character-contract.json`.

`character-contract.json` is deliberately explicit about unresolved hashes and pending rows. A
`null` value means “measure from the target VFS/config”, never “use zero”.
