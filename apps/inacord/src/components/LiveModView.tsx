// Onglet **Live mod** — modifier `nie.exe` pendant qu'il tourne, et lancer l'éditeur de
// sauvegarde livré avec le dépôt.
//
// Deux surfaces distinctes, volontairement séparées :
//
//  - **Live** : lit et écrit la mémoire du process en cours. La structure éditée est l'équipe
//    active, un tableau de `CraftResidentsStatusP` (0x38 octets par slot) dont les 24 premiers
//    octets forment un `CraftResidentsCharaInfo`. Les noms de champs viennent de la table de
//    réflexion embarquée dans le binaire (chaque champ y est enregistré avec son nom, son offset
//    et sa taille) — ils ne sont pas devinés.
//  - **Save Editor** : lance `InazumaElevenVRSaveEditor.exe` (racine du dépôt), et
//    optionnellement le jeu derrière, en direct (`nie.exe`, sans EACLauncher) pour que le live
//    mod puisse s'y attacher.
//
// L'adresse du tableau change à chaque lancement : `liveFindTeam` la retrouve en scannant un
// `charaParamId` connu puis en validant la forme du tableau (voisins à ±0x38 partageant le même
// `uniformId`). Tant qu'aucune adresse n'est trouvée, rien n'est modifiable.
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import type { LiveHit, LiveMember, LiveStatus } from "@/lib/bindings";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

/// Les champs éditables d'un membre, avec leur libellé et leur largeur en octets.
const CHAMPS: { cle: string; libelle: string; octets: number; aide: string }[] = [
  { cle: "charaParamId", libelle: "Personnage", octets: 4, aide: "charaParamId — la variante jouable du slot" },
  { cle: "uniformNo", libelle: "Numéro", octets: 2, aide: "uniformNo — numéro de maillot" },
  { cle: "scPosNo", libelle: "Position", octets: 1, aide: "scPosNo — poste sur le terrain" },
  { cle: "isCaptain", libelle: "Capitaine", octets: 1, aide: "isCaptain — brassard (0 ou 1)" },
  { cle: "uniformId", libelle: "Maillot", octets: 4, aide: "uniformId — tenue de l'équipe" },
  { cle: "shoesId", libelle: "Chaussures", octets: 4, aide: "shoesId" },
  { cle: "gloveId", libelle: "Gants", octets: 4, aide: "gloveId — 0 si aucun" },
  { cle: "emblemId", libelle: "Emblème", octets: 4, aide: "emblemId" },
];

function hex(n: number): string {
  return `0x${(n >>> 0).toString(16).toUpperCase().padStart(8, "0")}`;
}

/// Parse `0x…` ou un décimal ; `null` si illisible.
function parseU32(s: string): number | null {
  const t = s.trim();
  const v = t.startsWith("0x") || t.startsWith("0X")
    ? Number.parseInt(t.slice(2), 16)
    : Number.parseInt(t, 10);
  return Number.isFinite(v) ? v >>> 0 : null;
}

/// Les trois paliers de l'aura « Mode Aphrody » (`Lord Aphrody Mode`, アフロディモード), relevés
/// dans `skill/aura_skill_config_1.04.09.00.cfg.bin` → `AURA_CMD_INFO`. Leur nom interne
/// (`mode_change_c11150120`) porte le code de son propriétaire : Archon Aphrodite Teita Tanji.
/// Elles sont proposées d'un clic parce que c'est le cas d'usage qui a motivé ce panneau ; le
/// champ reste libre pour n'importe quel autre identifiant.
const AURAS_CONNUES: { id: number; nom: string }[] = [
  { id: 0x03d98821, nom: "Mode Aphrody — base" },
  { id: 0x0438cbd2, nom: "Mode Aphrody — exst" },
  { id: 0x3220427e, nom: "Mode Aphrody — legend" },
];

export function LiveModView() {
  const [status, setStatus] = useState<LiveStatus | null>(null);
  const [adresse, setAdresse] = useState("");
  const [ancre, setAncre] = useState("");
  const [membres, setMembres] = useState<LiveMember[]>([]);
  const [occupe, setOccupe] = useState(false);
  const [lancerJeu, setLancerJeu] = useState(true);

  const rafraichirStatus = useCallback(async () => {
    try {
      setStatus(await api.liveStatus());
    } catch (e) {
      toast.error(String(e));
    }
  }, []);

  useEffect(() => {
    void rafraichirStatus();
    const t = setInterval(() => void rafraichirStatus(), 4000);
    return () => clearInterval(t);
  }, [rafraichirStatus]);

  const chercher = useCallback(async () => {
    const brut = ancre.trim();
    const valeur = brut.startsWith("0x") || brut.startsWith("0X")
      ? Number.parseInt(brut.slice(2), 16)
      : Number.parseInt(brut, 10);
    if (!Number.isFinite(valeur) || valeur === 0) {
      toast.error("Donne un charaParamId présent dans ton équipe (hex 0x… ou décimal)");
      return;
    }
    setOccupe(true);
    try {
      const addr = await api.liveFindTeam(valeur >>> 0);
      setAdresse(addr);
      setMembres(await api.liveReadTeam(addr));
      toast.success(`Équipe trouvée à ${addr}`);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setOccupe(false);
    }
  }, [ancre]);

  const relire = useCallback(async () => {
    if (!adresse) return;
    setOccupe(true);
    try {
      setMembres(await api.liveReadTeam(adresse));
    } catch (e) {
      toast.error(String(e));
    } finally {
      setOccupe(false);
    }
  }, [adresse]);

  const ecrire = useCallback(
    async (slot: number, champ: string, brut: string) => {
      if (!adresse) return;
      const v = brut.trim().startsWith("0x")
        ? Number.parseInt(brut.trim().slice(2), 16)
        : Number.parseInt(brut.trim(), 10);
      if (!Number.isFinite(v)) {
        toast.error(`Valeur illisible pour ${champ}`);
        return;
      }
      try {
        const maj = await api.liveWriteMember(adresse, slot, champ, v >>> 0);
        setMembres((prev) => prev.map((m) => (m.slot === maj.slot ? maj : m)));
        toast.success(`slot ${slot} · ${champ} = ${hex(v)}`);
      } catch (e) {
        toast.error(String(e));
      }
    },
    [adresse],
  );

  // ── Scanner / poser une valeur (auras, slots de compétence) ─────────────────────────────
  const [valeurScan, setValeurScan] = useState("0x03D98821");
  const [hits, setHits] = useState<LiveHit[]>([]);
  const [aPoser, setAPoser] = useState("0x03D98821");

  const scanner = useCallback(async () => {
    const v = parseU32(valeurScan);
    if (v === null) {
      toast.error("Valeur illisible");
      return;
    }
    setOccupe(true);
    try {
      const r = await api.liveScanU32(v, 60);
      setHits(r);
      toast.success(`${r.length} occurrence(s) de ${hex(v)}`);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setOccupe(false);
    }
  }, [valeurScan]);

  const poser = useCallback(
    async (adr: string) => {
      const v = parseU32(aPoser);
      if (v === null) {
        toast.error("Valeur à poser illisible");
        return;
      }
      try {
        const relu = await api.liveWriteU32(adr, v);
        toast.success(`${adr} ← ${hex(relu)}`);
        setHits((prev) =>
          prev.map((h) => (h.address === adr ? { ...h, context_hex: h.context_hex } : h)),
        );
      } catch (e) {
        toast.error(String(e));
      }
    },
    [aPoser],
  );

  const lancerEditeur = useCallback(async () => {
    setOccupe(true);
    try {
      const r = await api.launchSaveEditor(lancerJeu);
      if (r.launched.length > 0) toast.success(`Lancé : ${r.launched.join(", ")}`);
      if (r.missing.length > 0) toast.error(`Introuvable : ${r.missing.join(", ")}`);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setOccupe(false);
    }
  }, [lancerJeu]);

  return (
    <div className="flex flex-col gap-4 p-4">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            Process
            {status?.running ? (
              <Badge variant="default">{status.process} · pid {status.pid}</Badge>
            ) : (
              <Badge variant="secondary">jeu fermé</Badge>
            )}
          </CardTitle>
          <CardDescription>
            {status?.running
              ? `base ${status.module_base} · slide ASLR ${status.aslr_slide}`
              : "Lance le jeu pour attacher le live mod."}
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap items-end gap-3">
          <div className="flex items-center gap-2">
            <Switch id="aussi-jeu" checked={lancerJeu} onCheckedChange={setLancerJeu} />
            <Label htmlFor="aussi-jeu">lancer aussi le jeu</Label>
          </div>
          <Button onClick={() => void lancerEditeur()} disabled={occupe}>
            Save Editor
          </Button>
          <Button variant="outline" onClick={() => void rafraichirStatus()} disabled={occupe}>
            Rafraîchir
          </Button>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Équipe active</CardTitle>
          <CardDescription>
            L'adresse change à chaque lancement. Donne un <code>charaParamId</code> que tu sais
            présent dans ton onze : le scan valide ensuite la forme du tableau (pas 0x38, même{" "}
            <code>uniformId</code> entre voisins) avant de rendre une adresse.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <div className="flex flex-wrap items-end gap-2">
            <div className="flex flex-col gap-1">
              <Label htmlFor="ancre">charaParamId d'ancrage</Label>
              <Input
                id="ancre"
                value={ancre}
                onChange={(e) => setAncre(e.target.value)}
                placeholder="0x209B996D"
                className="w-52 font-mono"
              />
            </div>
            <Button onClick={() => void chercher()} disabled={occupe || !status?.running}>
              Trouver l'équipe
            </Button>
            <Button variant="outline" onClick={() => void relire()} disabled={occupe || !adresse}>
              Relire
            </Button>
            {adresse ? <Badge variant="outline" className="font-mono">{adresse}</Badge> : null}
          </div>

          {!status?.running ? (
            <Alert>
              <AlertTitle>Jeu fermé</AlertTitle>
              <AlertDescription>
                Le live mod écrit dans la mémoire du process : il faut que le jeu tourne.
              </AlertDescription>
            </Alert>
          ) : null}

          {membres.length > 0 ? (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b text-left">
                    <th className="p-2">Slot</th>
                    {CHAMPS.map((c) => (
                      <th key={c.cle} className="p-2" title={c.aide}>
                        {c.libelle}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {membres.map((m) => (
                    <tr key={m.slot} className="border-b last:border-0">
                      <td className="p-2 font-mono text-xs">
                        {m.slot}
                        <div className="text-muted-foreground">{m.address}</div>
                      </td>
                      {CHAMPS.map((c) => {
                        const valeur =
                          c.cle === "charaParamId" ? hex(m.chara_param_id)
                          : c.cle === "uniformId" ? hex(m.uniform_id)
                          : c.cle === "shoesId" ? hex(m.shoes_id)
                          : c.cle === "gloveId" ? hex(m.glove_id)
                          : c.cle === "emblemId" ? hex(m.emblem_id)
                          : c.cle === "uniformNo" ? String(m.uniform_no)
                          : c.cle === "scPosNo" ? String(m.sc_pos_no)
                          : m.is_captain ? "1" : "0";
                        return (
                          <td key={c.cle} className="p-1">
                            <Input
                              defaultValue={valeur}
                              className={c.octets === 4 ? "w-32 font-mono text-xs" : "w-20 font-mono text-xs"}
                              onKeyDown={(e) => {
                                if (e.key === "Enter") {
                                  void ecrire(m.slot, c.cle, (e.target as HTMLInputElement).value);
                                }
                              }}
                              onBlur={(e) => {
                                if (e.target.value !== valeur) {
                                  void ecrire(m.slot, c.cle, e.target.value);
                                }
                              }}
                            />
                          </td>
                        );
                      })}
                    </tr>
                  ))}
                </tbody>
              </table>
              <p className="mt-2 text-xs text-muted-foreground">
                Entrée ou perte de focus écrit la valeur dans le process. Une écriture ne touche
                que les octets du champ, à sa largeur déclarée.
              </p>
            </div>
          ) : null}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Auras et compétences</CardTitle>
          <CardDescription>
            Cherche un identifiant dans la mémoire du jeu, puis pose-le où tu veux. C'est ce qui
            permet de donner une aura à n'importe quel personnage : on repère un emplacement de
            compétence, on y écrit l'identifiant. Le voisinage de chaque occurrence est affiché —
            un pas régulier entre deux adresses signale un tableau, une occurrence isolée non.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center gap-2">
            {AURAS_CONNUES.map((a) => (
              <Button
                key={a.id}
                size="sm"
                variant="outline"
                onClick={() => {
                  setValeurScan(hex(a.id));
                  setAPoser(hex(a.id));
                }}
              >
                {a.nom}
              </Button>
            ))}
          </div>
          <div className="flex flex-wrap items-end gap-2">
            <div className="flex flex-col gap-1">
              <Label htmlFor="scan">Chercher</Label>
              <Input
                id="scan"
                value={valeurScan}
                onChange={(e) => setValeurScan(e.target.value)}
                className="w-44 font-mono"
              />
            </div>
            <Button onClick={() => void scanner()} disabled={occupe || !status?.running}>
              Scanner
            </Button>
            <div className="flex flex-col gap-1">
              <Label htmlFor="poser">Valeur à poser</Label>
              <Input
                id="poser"
                value={aPoser}
                onChange={(e) => setAPoser(e.target.value)}
                className="w-44 font-mono"
              />
            </div>
          </div>

          {hits.length > 0 ? (
            <div className="max-h-96 overflow-auto">
              <table className="w-full text-xs">
                <thead>
                  <tr className="border-b text-left">
                    <th className="p-2">Adresse</th>
                    <th className="p-2">Voisinage</th>
                    <th className="p-2" />
                  </tr>
                </thead>
                <tbody>
                  {hits.map((h) => (
                    <tr key={h.address} className="border-b last:border-0">
                      <td className="p-2 font-mono">{h.address}</td>
                      <td className="p-2 font-mono break-all text-muted-foreground">
                        {h.context_hex.slice(0, h.context_offset * 2)}
                        <span className="text-foreground font-bold">
                          {h.context_hex.slice(h.context_offset * 2, h.context_offset * 2 + 8)}
                        </span>
                        {h.context_hex.slice(h.context_offset * 2 + 8)}
                      </td>
                      <td className="p-2">
                        <Button size="sm" variant="secondary" onClick={() => void poser(h.address)}>
                          Poser ici
                        </Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : null}
        </CardContent>
      </Card>
    </div>
  );
}

export default LiveModView;
