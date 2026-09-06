"""Build a bounded, auditable inventory of data/oc.

The catalog describes OC metadata and source files without pretending that a source image is a
game asset. Game-facing paths and unresolved identifiers live in each character's
``game/character-contract.json``.

    uv run scripts/donnees/oc-catalog.py --write
    uv run scripts/donnees/oc-catalog.py --check
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OC_ROOT = ROOT / "data" / "oc"
OUTPUT = OC_ROOT / "catalog.json"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def classify(relative: Path) -> str:
    parts = relative.parts
    if "provenance" in parts:
        return "provenance"
    if "source" in parts:
        return "original"
    if "game" in parts:
        return "game_metadata"
    if parts[-1].endswith(".md") or parts[-1].endswith(".json"):
        return "metadata"
    return "unclassified"


def inspect_files() -> list[dict[str, object]]:
    rows = []
    for path in sorted(p for p in OC_ROOT.rglob("*") if p.is_file()):
        if path == OUTPUT:
            continue
        relative = path.relative_to(ROOT)
        rows.append(
            {
                "path": relative.as_posix(),
                "bytes": path.stat().st_size,
                "sha256": sha256(path),
                "class": classify(path.relative_to(OC_ROOT)),
            }
        )
    return rows


def validate_character_layout(character_dir: Path, contract_path: Path) -> None:
    """Require the complete source/metadata boundary for every OC directory."""
    required_files = (
        "README.md",
        "manifest.json",
        "game/README.md",
        "game/character-contract.json",
        "provenance/SHA256SUMS",
    )
    for relative in required_files:
        candidate = character_dir / relative
        if not candidate.is_file():
            raise ValueError(f"{character_dir}: required OC file is missing: {relative}")
    for relative in ("source", "provenance", "game"):
        candidate = character_dir / relative
        if not candidate.is_dir():
            raise ValueError(f"{character_dir}: required OC directory is missing: {relative}")
    contract = json.loads(contract_path.read_text(encoding="utf-8"))
    if contract.get("character_slug") != character_dir.name:
        raise ValueError(
            f"{contract_path}: character_slug does not match directory {character_dir.name!r}"
        )
    manifest_path = character_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    expected_contract = contract_path.relative_to(ROOT).as_posix()
    if manifest.get("game_contract") != expected_contract:
        raise ValueError(
            f"{manifest_path}: game_contract must point to {expected_contract!r}"
        )


def audit_source_manifest(character_dir: Path) -> dict[str, object]:
    """Check every available original against its versioned SHA256 manifest."""
    manifest_path = character_dir / "provenance" / "SHA256SUMS"
    present: list[str] = []
    missing: list[str] = []
    for line_number, line in enumerate(manifest_path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        match = re.fullmatch(r"([0-9a-fA-F]{64})  (.+)", line)
        if match is None:
            raise ValueError(f"{manifest_path}:{line_number}: invalid SHA256SUMS entry")
        expected, relative = match.groups()
        relative_path = Path(relative)
        if relative_path.is_absolute() or ".." in relative_path.parts:
            raise ValueError(f"{manifest_path}:{line_number}: source path escapes source/")
        source_path = character_dir / "source" / Path(relative.replace("/", "\\"))
        if not source_path.is_file():
            missing.append(relative)
            continue
        actual = sha256(source_path)
        if actual.lower() != expected.lower():
            raise ValueError(f"{source_path}: SHA-256 mismatch")
        present.append(relative)
    return {
        "manifest_entries": len(present) + len(missing),
        "present": len(present),
        "missing": missing,
    }


def load_contracts() -> list[dict[str, object]]:
    contracts = []
    character_dirs = sorted(path for path in OC_ROOT.iterdir() if path.is_dir())
    for character_dir in character_dirs:
        path = character_dir / "game" / "character-contract.json"
        if not path.is_file():
            raise ValueError(f"{character_dir}: game/character-contract.json is missing")
        validate_character_layout(character_dir, path)
        source_audit = audit_source_manifest(character_dir)
        contract = json.loads(path.read_text(encoding="utf-8"))
        validate_contract(contract, path)
        codes = [variant["internal_code"] for variant in contract["variants"]]
        provenance_files = sum(1 for candidate in (character_dir / "provenance").iterdir() if candidate.is_file())
        source_files = sum(1 for candidate in (character_dir / "source").rglob("*") if candidate.is_file())
        contracts.append(
            {
                "path": path.relative_to(ROOT).as_posix(),
                "schema": contract.get("schema"),
                "character_slug": contract.get("character_slug"),
                "variants": codes,
                "provenance_files": provenance_files,
                "source_files": source_files,
                "source_audit": source_audit,
                "required_cfgbin_tables": sorted(
                    {
                        table["table"]
                        for variant in contract["variants"]
                        for table in variant["tables"]
                        if table.get("required")
                    }
                ),
                "lua": contract.get("lua_runtime", {}).get("status"),
            }
        )
    return contracts


def walk_json(value: object):
    if isinstance(value, dict):
        yield value
        for child in value.values():
            yield from walk_json(child)
    elif isinstance(value, list):
        for child in value:
            yield from walk_json(child)


def signed_i32(hex_value: str) -> str:
    value = int(hex_value, 16)
    if value >= 2**31:
        value -= 2**32
    return str(value)


def validate_reference_evidence(contract: dict[str, object], path: Path) -> None:
    evidence_ref = contract.get("reference_evidence")
    if not isinstance(evidence_ref, str):
        raise ValueError(f"{path}: reference_evidence is required")
    evidence_path = ROOT / evidence_ref
    evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    base = evidence["measured"]["chara_base"]
    base_path = ROOT / base["path"]
    base_json = json.loads(base_path.read_text(encoding="utf-8"))
    base_nodes = [n for n in walk_json(base_json) if n.get("name") == base["node"]]
    if len(base_nodes) != 1:
        raise ValueError(f"{path}: expected one evidence node {base['node']}")
    variables = base_nodes[0].get("variables", [])
    if variables[1].get("value") != base["internal_code"]:
        raise ValueError(f"{path}: reference code does not match evidence")
    if variables[0].get("value") != signed_i32(base["chara_id"]):
        raise ValueError(f"{path}: reference chara_id does not match evidence")
    param = evidence["measured"]["chara_param"]
    param_json = json.loads((ROOT / param["path"]).read_text(encoding="utf-8"))
    for candidate in param["candidates"]:
        nodes = [n for n in walk_json(param_json) if n.get("name") == candidate["node"]]
        if len(nodes) != 1:
            raise ValueError(f"{path}: expected one evidence node {candidate['node']}")
        values = nodes[0].get("variables", [])
        if values[0].get("value") != signed_i32(candidate["chara_param_id"]):
            raise ValueError(f"{path}: candidate param id does not match evidence")
        if values[1].get("value") != signed_i32(param["join_value"]):
            raise ValueError(f"{path}: candidate join value does not match evidence")
    live_vfs = evidence.get("live_vfs_probe", {})
    live_tables = live_vfs.get("required_cfgbin_tables", [])
    expected_tables = {
        "chara_base", "chara_param", "chara_model", "chara_parts", "chara_scale",
        "chara_motion", "chara_face", "chara_name_tag", "chara_costume",
    }
    if (
        live_vfs.get("available") is not True
        or len(live_tables) != len(expected_tables)
        or {table.get("table") for table in live_tables} != expected_tables
        or any(
            not isinstance(table.get("path"), str)
            or not table["path"].startswith("data/common/gamedata/character/")
            or table.get("format") not in {"T2B", "RDBN"}
            or not isinstance(table.get("bytes"), int)
            for table in live_tables
        )
    ):
        raise ValueError(f"{path}: live VFS cfg.bin inventory is incomplete or invalid")
    if live_vfs.get("lua_reference_present") is not True:
        raise ValueError(f"{path}: live VFS Lua reference is missing")
    game_probe = evidence.get("game_installation_probe", {})
    if (
        game_probe.get("present") is not True
        or game_probe.get("binary_relative_path") != "nie.exe"
        or game_probe.get("image_base") != "0x140000000"
        or not isinstance(game_probe.get("bytes"), int)
        or not re.fullmatch(r"[0-9a-f]{64}", game_probe.get("sha256", ""))
    ):
        raise ValueError(f"{path}: nie.exe installation probe is incomplete or invalid")
    visual = evidence.get("measured", {}).get("visual_assets")
    if not isinstance(visual, dict):
        raise ValueError(f"{path}: visual asset evidence is required")
    face_model = visual.get("face_model", {})
    metadata = face_model.get("metadata", {})
    mesh = face_model.get("mesh", {})
    atlases = face_model.get("texture_atlases", [])
    if metadata.get("format") != "G4MD" or metadata.get("submeshes") != 4:
        raise ValueError(f"{path}: invalid G4MD reference measurement")
    if mesh.get("format") != "G4MG" or not isinstance(mesh.get("bytes"), int):
        raise ValueError(f"{path}: invalid G4MG reference measurement")
    if not isinstance(atlases, list) or len(atlases) != 2 or any(
        atlas.get("format") != "G4TX" or atlas.get("subtextures") != 12 for atlas in atlases
    ):
        raise ValueError(f"{path}: expected two 12-subtexture face atlases")
    portrait = visual.get("portrait_icon", {})
    portrait_textures = portrait.get("subtextures", [])
    if portrait.get("format") != "G4TX" or len(portrait_textures) != 2 or any(
        texture.get("width") != 256 or texture.get("height") != 256
        for texture in portrait_textures
    ):
        raise ValueError(f"{path}: expected two 256x256 portrait subtextures")
    card = visual.get("mode_change_card", {})
    layout = card.get("layout", {})
    localized = card.get("localized_images", {})
    locales = localized.get("locales", [])
    if (
        layout.get("format") != "OBJBIN"
        or layout.get("components") != 3
        or localized.get("format") != "G4TX"
        or localized.get("width") != 1728
        or localized.get("height") != 352
        or localized.get("subtextures") != 1
        or locales != ["de", "en", "es", "fr", "it", "ja", "pt", "zh_hans", "zh_hant"]
    ):
        raise ValueError(f"{path}: invalid localized mode-change card measurement")
    lua_evidence = evidence.get("measured", {}).get("lua_runtime", {})
    if (
        lua_evidence.get("format") != "Lua 5.2 bytecode"
        or lua_evidence.get("magic_hex") != "1B4C7561"
        or lua_evidence.get("version_hex") != "52"
    ):
        raise ValueError(f"{path}: invalid Lua reference measurement")


def validate_contract(contract: dict[str, object], path: Path) -> None:
    """Reject contracts that cannot be mapped to the game's VFS naming contract."""
    if contract.get("schema") != "niers.oc.character-contract/v1":
        raise ValueError(f"{path}: unsupported contract schema")
    game = contract.get("game", {})
    if (
        game.get("binary") != "nie.exe"
        or game.get("binary_path") != "nie.exe"
        or game.get("image_base") != "0x140000000"
    ):
        raise ValueError(f"{path}: game binary metadata must target nie.exe at 0x140000000")
    format_contract = contract.get("formats", {})
    cfgbin = format_contract.get("cfg.bin", {})
    if set(cfgbin.get("container_variants", [])) != {"T2B", "RDBN"}:
        raise ValueError(f"{path}: cfg.bin must declare both T2B and RDBN")
    lua_format = format_contract.get("lua.bin", {})
    if (
        lua_format.get("format") != "Lua 5.2 bytecode"
        or lua_format.get("magic_hex") != "1B4C7561"
        or lua_format.get("version_hex") != "52"
    ):
        raise ValueError(f"{path}: lua.bin format metadata is not Lua 5.2")
    for name, magic in (("G4MD", "G4MD"), ("G4MG", "G4MG"), ("G4TX", "G4TX")):
        if format_contract.get(name, {}).get("magic") != magic:
            raise ValueError(f"{path}: {name} magic metadata is missing or incorrect")
    if format_contract.get("AWB", {}).get("magic") != "AFS2":
        raise ValueError(f"{path}: AWB magic metadata must be AFS2")
    namespace = contract.get("namespace", {})
    groups = set(namespace.get("allowed_series_groups", []))
    formats = {"T2B", "RDBN", "G4MD", "G4MG", "G4TX", "ACB", "AWB"}
    variants = contract.get("variants", [])
    if not variants:
        raise ValueError(f"{path}: no character variant")
    seen: set[str] = set()
    for variant in variants:
        code = variant.get("internal_code")
        if not isinstance(code, str) or not re.fullmatch(r"c[0-9]{8}", code) or code in seen:
            raise ValueError(f"{path}: duplicate or invalid internal_code: {code!r}")
        seen.add(code)
        if variant.get("series_group") not in groups:
            raise ValueError(f"{path}: unknown series_group for {code}: {variant.get('series_group')!r}")
        for table in variant.get("tables", []):
            vfs_path = table.get("vfs_path", "")
            if not (vfs_path.startswith("data/common/gamedata/character/") and vfs_path.endswith(".cfg.bin")):
                raise ValueError(f"{path}: invalid cfg.bin path: {vfs_path!r}")
            if table.get("format") not in {"T2B", "RDBN"}:
                raise ValueError(f"{path}: invalid cfg.bin format: {table.get('format')!r}")
        for asset in variant.get("assets", []):
            vfs_path = asset.get("vfs_path", "")
            filename = Path(vfs_path).name
            if not vfs_path.startswith("data/") or not filename.startswith(code):
                raise ValueError(f"{path}: asset is not code-prefixed: {vfs_path!r}")
            if asset.get("format") not in formats:
                raise ValueError(f"{path}: unknown asset format: {asset.get('format')!r}")
    lua = contract.get("lua_runtime", {})
    if lua.get("magic_hex") != "1B4C7561" or lua.get("version_hex") != "52":
        raise ValueError(f"{path}: Lua runtime must declare Lua 5.2 bytecode")
    lua_reference = lua.get("reference_path")
    if lua_reference is not None and (
        not isinstance(lua_reference, str)
        or not lua_reference.startswith("data/common/script/lua/")
        or not lua_reference.endswith(".lua.bin")
    ):
        raise ValueError(f"{path}: invalid Lua reference path: {lua_reference!r}")
    lua_evidence = lua.get("reference_evidence")
    if lua_evidence is not None and (
        not isinstance(lua_evidence, str) or not lua_evidence.startswith("data/oc/")
    ):
        raise ValueError(f"{path}: invalid Lua reference evidence")
    visual = contract.get("visual_contract")
    if not isinstance(visual, dict):
        raise ValueError(f"{path}: visual_contract is required")
    reference = visual.get("reference")
    if not isinstance(reference, str) or not reference.startswith("data/oc/"):
        raise ValueError(f"{path}: visual reference must point inside data/oc")
    visual_formats = formats | {"OBJBIN"}
    per_variant = visual.get("per_variant_templates")
    if not isinstance(per_variant, list) or not per_variant:
        raise ValueError(f"{path}: visual per_variant_templates is empty")
    for template in per_variant:
        if not isinstance(template, dict):
            raise ValueError(f"{path}: visual template is not an object")
        role = template.get("role")
        fmt = template.get("format")
        template_path = template.get("path_template")
        if not isinstance(role, str) or not isinstance(fmt, str) or fmt not in visual_formats:
            raise ValueError(f"{path}: invalid visual template format: {fmt!r}")
        if not isinstance(template_path, str) or "<code>" not in template_path:
            raise ValueError(f"{path}: visual template lacks <code>: {role!r}")
        if "<series_group>" in template_path and "<series_group>" not in template_path.split("<code>", 1)[0]:
            raise ValueError(f"{path}: series template must place <series_group> before <code>: {role!r}")
        if "expected_subtextures" in template and (
            not isinstance(template["expected_subtextures"], int)
            or template["expected_subtextures"] < 1
        ):
            raise ValueError(f"{path}: invalid expected_subtextures: {role!r}")
    code_keyed = visual.get("code_keyed_menu_templates")
    if not isinstance(code_keyed, list) or not code_keyed:
        raise ValueError(f"{path}: visual code_keyed_menu_templates is empty")
    for template in code_keyed:
        if not isinstance(template, dict) or not isinstance(template.get("path_template"), str):
            raise ValueError(f"{path}: invalid code-keyed visual template")
        if "<code>" not in template["path_template"]:
            raise ValueError(f"{path}: code-keyed visual template lacks <code>")
        if template.get("format") not in visual_formats:
            raise ValueError(f"{path}: invalid code-keyed visual format: {template.get('format')!r}")
        locales = template.get("locales")
        if locales is not None and (
            not isinstance(locales, list)
            or not locales
            or len(locales) != len(set(locales))
            or any(not isinstance(locale, str) or not re.fullmatch(r"[a-z]{2}(?:_[a-z]+)?", locale) for locale in locales)
        ):
            raise ValueError(f"{path}: invalid visual locales")
    validate_reference_evidence(contract, path)


def build_catalog() -> dict[str, object]:
    return {
        "schema": "niers.oc.catalog/v1",
        "root": "data/oc",
        "policy": {
            "game_assets_are_not_copied_here": True,
            "game_paths_must_start_with_data": True,
            "unresolved_ids_are_null": True,
            "source_files_are_originals": True,
        },
        "contracts": load_contracts(),
        "files": inspect_files(),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true", help="write data/oc/catalog.json")
    parser.add_argument("--check", action="store_true", help="validate and print the catalog")
    args = parser.parse_args()
    catalog = build_catalog()
    if args.write:
        OUTPUT.write_text(json.dumps(catalog, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if args.check:
        if not OUTPUT.is_file():
            raise ValueError(f"catalog missing: {OUTPUT}")
        current = json.loads(OUTPUT.read_text(encoding="utf-8"))
        if current != catalog:
            raise ValueError(f"catalog is stale: regenerate with --write: {OUTPUT}")
    print(json.dumps({"files": len(catalog["files"]), "contracts": len(catalog["contracts"]), "output": str(OUTPUT.relative_to(ROOT))}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
