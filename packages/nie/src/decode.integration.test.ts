/**
 * Test d'INTÉGRATION du décodage de vrais fichiers du jeu via la FFI + VFS.
 *
 * Gated sur `NIE_GAME_DIR` (comme les golden tests Rust) : si le jeu n'est pas
 * monté (`<dir>/data/cpk_list.cfg.bin` absent), tous les cas sont `skip`. Sinon,
 * on prouve end-to-end que :
 *   - le VFS s'ouvre (cpk_list.cfg.bin AES-256-CBC déchiffré),
 *   - un `.g4tx` BC7 réel se décode en PNG aux bonnes dimensions,
 *   - un `.cfg.bin` réel se décode en JSON structuré (menu_setting T2b),
 *   - `detectFormat` reconnaît les magies.
 *
 * Lancer :  NIE_GAME_DIR=/home/aphrody/niers bun test packages/nie/
 */

import { test, expect, describe } from "bun:test";
import { existsSync } from "node:fs";
import { vfsOpen, decode, decodeToPng, detectFormat } from "./index.ts";

const GAME_DIR = process.env["NIE_GAME_DIR"] ?? "/home/aphrody/niers";
const DATA_DIR = `${GAME_DIR}/data`;
const HAS_GAME = existsSync(`${DATA_DIR}/cpk_list.cfg.bin`);

const BG = "data/dx11/menu/100_mainmenu/mainmenu90/mainmenu90_00/mainmenu90_00.g4tx";
const SETTING = "data/common/gamedata/menu/cfg/main_menu_setting.cfg.bin";

describe("décodage intégration (VFS + FFI)", () => {
  test.skipIf(!HAS_GAME)("VFS s'ouvre et lit un g4tx réel", () => {
    using vfs = vfsOpen(DATA_DIR)!;
    expect(vfs).not.toBeNull();
    const g4tx = vfs.read(BG);
    expect(g4tx).not.toBeNull();
    // magic "G4TX" (0x47 0x34 0x54 0x58)
    expect(Array.from(g4tx!.slice(0, 4))).toEqual([0x47, 0x34, 0x54, 0x58]);
    expect(detectFormat(g4tx!).name).toBe("G4TX");
  });

  // Budget explicite : décoder 2 904 000 pixels en BC7 à travers le FFI prend
  // ~2 s à vide, mais dépasse les 5 000 ms par défaut quand la suite complète
  // fait tourner quinze workspaces en parallèle. Sans ce budget, le verdict du
  // test dépend de la CHARGE de la machine et non du code : vert lancé seul,
  // rouge dans `bun run test`, et rien dans le message n'indique pourquoi.
  test.skipIf(!HAS_GAME)("g4tx BC7 → PNG 2640×1100", () => {
    using vfs = vfsOpen(DATA_DIR)!;
    const g4tx = vfs.read(BG)!;
    const png = decodeToPng(g4tx);
    expect(png).not.toBeNull();
    // magic PNG
    expect(Array.from(png!.slice(0, 4))).toEqual([0x89, 0x50, 0x4e, 0x47]);
    // IHDR width/height (big-endian, octets 16..24)
    const dv = new DataView(png!.buffer, png!.byteOffset, png!.byteLength);
    expect(dv.getUint32(16, false)).toBe(2640);
    expect(dv.getUint32(20, false)).toBe(1100);
  }, 30_000);

  test.skipIf(!HAS_GAME)("cfg.bin réel → JSON menu_setting (T2b)", () => {
    using vfs = vfsOpen(DATA_DIR)!;
    const bytes = vfs.read(SETTING)!;
    expect(bytes).not.toBeNull();
    const obj = decode(bytes) as {
      format: string;
      entries: Array<{ name: string }>;
    } | null;
    expect(obj).not.toBeNull();
    expect(obj!.format).toBe("T2b");
    expect(Array.isArray(obj!.entries)).toBe(true);
    // la liste des layers de menu commence par MENU_LAYER_INFO_LIST_BEG
    expect(obj!.entries[0]!.name).toBe("MENU_LAYER_INFO_LIST_BEG");
  });

  test.skipIf(!HAS_GAME)("police : renderText('A') → PNG 39×71", () => {
    using vfs = vfsOpen(DATA_DIR)!;
    using font = vfs.openFont()!;
    expect(font).not.toBeNull();
    const png = font.renderText("A");
    expect(png).not.toBeNull();
    // magic PNG
    expect(Array.from(png!.slice(0, 4))).toEqual([0x89, 0x50, 0x4e, 0x47]);
    // IHDR : largeur = avance de 'A' (39), hauteur = cell_height (71).
    const dv = new DataView(png!.buffer, png!.byteOffset, png!.byteLength);
    expect(dv.getUint32(16, false)).toBe(39);
    expect(dv.getUint32(20, false)).toBe(71);
  });

  test.skipIf(!HAS_GAME)("police : 'AW' accumule l'avance (39 + 48 = 87)", () => {
    using vfs = vfsOpen(DATA_DIR)!;
    using font = vfs.openFont()!;
    const png = font.renderText("AW")!;
    expect(png).not.toBeNull();
    const dv = new DataView(png.buffer, png.byteOffset, png.byteLength);
    // avance(A)=39 + avance(W)=48 → 87 ; hauteur inchangée.
    expect(dv.getUint32(16, false)).toBe(87);
    expect(dv.getUint32(20, false)).toBe(71);
  });

  test.skipIf(HAS_GAME)("skip si jeu absent (placeholder de gate)", () => {
    expect(HAS_GAME).toBe(false);
  });
});
