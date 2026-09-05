/**
 * Préchargement de test : pose le miroir SQLite comme source de secours.
 *
 * Chargé par le `bunfig.toml` de `packages/azalee` ET par celui de `packages/azalee-outils`.
 * Comme il pose un DÉFAUT et non une fabrique explicite, il ne perturbe pas les suites qui
 * testent l'injection elle-même.
 */
import { poserMiroirParDefaut } from "./mirror-source";

poserMiroirParDefaut();
