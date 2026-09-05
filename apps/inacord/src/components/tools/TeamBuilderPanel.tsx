// **Constructeur d'équipe** — le terrain, les 20 créneaux, les stats de la composition.
//
// Portage de `apps/azalee/components/tools/my-team/` (6 fichiers, 2 966 lignes) et de
// `app/tools/my-team/page.tsx`.
//
// ## Sans session, et c'est mieux
//
// Le constructeur du wiki enregistre côté serveur (`app/actions/teams.ts`, derrière
// `getServerSession`) : sans compte connecté le bouton « Créer » n'existe pas, et il ne reste
// qu'UN brouillon en `localStorage`. L'explorateur n'a ni compte ni session — mais il a un
// disque : `teamsDb` (table `teams` de `mods.db`, migration v4) garde autant de compositions
// NOMMÉES qu'on veut, hors ligne. Et le pont avec le wiki n'est pas perdu pour autant : le code
// de partage est celui de `@rosegriffon/azalee/game/team-code`, donc une composition faite ici se
// colle dans l'URL du site, et réciproquement.
//
// ## Glisser-déposer sans dépendance
//
// Le wiki utilise `@dnd-kit/core` + `@dnd-kit/sortable`. Ils ne sont pas des dépendances de
// l'explorateur, et les ajouter demanderait de toucher au verrou d'installation partagé. Le
// glisser-déposer natif HTML5 (`draggable` + `dragover`/`drop`) suffit ici : il n'y a ni liste
// triable ni capteur tactile à gérer, seulement des cases. Le geste « taper pour placer », déjà
// présent sur le wiki, reste la voie principale.
//
// Les règles de jeu — facteur de poste, synergies d'élément, recalcul par niveau — sont IMPORTÉES
// de `@rosegriffon/azalee/game/team-rules`, jamais recopiées.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { toast } from "sonner";

import {
  calculateElementSynergies,
  recalculateMemberStats,
} from "@rosegriffon/azalee/game/team-rules";
import { ROLE_COLORS, ROLE_LABELS } from "@rosegriffon/azalee/game/formations";
import { decodeTeamCode, encodeTeamCode } from "@rosegriffon/azalee/game/team-code";

import {
  FORMATIONS,
  NB_RESERVES,
  NB_SUPPORTS,
  autoRemplir,
  cheminVisage,
  versMembre,
  type Joueur,
  type TeamMember,
} from "@/lib/equipe";
import { useFiltered } from "@/lib/filtrage";
import { useSettings } from "@/lib/settings";
import { teamsDb, type EquipeEnregistree } from "@/lib/teamsDb";
import { useThumbnail } from "@/lib/thumbs";
import { Badge } from "@/components/ui/badge";
import { Icon } from "@/components/ui/Icon";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";

/** Les sept stats agrégées de la composition, dans l'ordre du jeu. */
const STATS: { cle: keyof ReturnType<typeof recalculateMemberStats>; libelle: string }[] = [
  { cle: "kick", libelle: "Frappe" },
  { cle: "control", libelle: "Contrôle" },
  { cle: "technique", libelle: "Technique" },
  { cle: "pressure", libelle: "Pression" },
  { cle: "physical", libelle: "Physique" },
  { cle: "agility", libelle: "Agilité" },
  { cle: "intelligence", libelle: "Intelligence" },
];

/** Portrait d'un membre — décodé du VFS, jamais chargé du réseau. */
function Portrait({ membre, gameDir }: { membre: TeamMember; gameDir?: string }) {
  const { ref, src } = useThumbnail(cheminVisage(membre.internalCode ?? null) ?? "", "g4tx", gameDir);
  return (
    <div ref={ref} className="h-full w-full overflow-hidden">
      {src ? (
        <img src={src} alt="" className="h-full w-full object-contain" />
      ) : (
        <div className="flex h-full items-center justify-center">
          <Icon name="person" size={16} className="text-on-surface-variant/40" />
        </div>
      )}
    </div>
  );
}

/** Une case de la composition — vide ou occupée, cible de dépôt, source de glisser. */
function Case({
  creneau,
  role,
  membre,
  actif,
  onTaper,
  onRetirer,
  onDeposer,
  style,
  gameDir,
}: {
  creneau: string;
  role: string;
  membre: TeamMember | null;
  actif: boolean;
  onTaper: () => void;
  onRetirer: () => void;
  onDeposer: (depuis: string) => void;
  style?: React.CSSProperties;
  gameDir?: string;
}) {
  return (
    <div
      style={style}
      className={`group relative flex flex-col overflow-hidden rounded-lg border ${
        actif ? "border-accent ring-1 ring-accent" : "border-app-line"
      } bg-app-dark-box`}
      onDragOver={(e) => e.preventDefault()}
      onDrop={(e) => {
        e.preventDefault();
        const depuis = e.dataTransfer.getData("text/plain");
        if (depuis) onDeposer(depuis);
      }}
    >
      <button
        type="button"
        draggable={!!membre}
        onDragStart={(e) => e.dataTransfer.setData("text/plain", `slot:${creneau}`)}
        onClick={onTaper}
        className="flex min-h-0 flex-1 flex-col items-stretch text-left"
        title={membre ? membre.name : `Créneau ${creneau}`}
      >
        <div
          className="flex min-h-0 flex-1 items-center justify-center"
          style={{ backgroundColor: membre ? undefined : `${ROLE_COLORS[role] ?? "#555"}22` }}
        >
          {membre ? (
            <Portrait membre={membre} gameDir={gameDir} />
          ) : (
            <span className="type-label-small text-on-surface-variant">
              {ROLE_LABELS[role] ?? role}
            </span>
          )}
        </div>
        {membre && (
          <span className="truncate px-1 py-0.5 type-label-small text-on-surface">
            {membre.name}
          </span>
        )}
      </button>
      {membre && (
        <button
          type="button"
          aria-label={`Retirer ${membre.name}`}
          className="absolute right-0.5 top-0.5 rounded-full bg-app/70 p-0.5 text-on-surface-variant opacity-0 group-hover:opacity-100"
          onClick={onRetirer}
        >
          <Icon name="close" size={12} />
        </button>
      )}
    </div>
  );
}

export function TeamBuilderPanel({ roster }: { roster: Joueur[] }) {
  const settings = useSettings();
  const [indexFormation, setIndexFormation] = useState(0);
  const [membres, setMembres] = useState<Record<string, TeamMember>>({});
  const [nom, setNom] = useState("");
  const [idEquipe, setIdEquipe] = useState<string | null>(null);
  const [enregistrees, setEnregistrees] = useState<EquipeEnregistree[]>([]);
  const [creneauActif, setCreneauActif] = useState<string | null>(null);
  const [requete, setRequete] = useState("");
  const [niveau, setNiveau] = useState(99);
  const historique = useRef<Record<string, TeamMember>[]>([]);

  const formation = FORMATIONS[indexFormation] ?? FORMATIONS[0];
  const parId = useMemo(() => new Map(roster.map((j) => [j.id, j])), [roster]);
  const filtres = useFiltered(roster, requete, (j) => [j.nom, j.poste, j.element, j.rarete]);

  const rafraichirListe = useCallback(() => {
    teamsDb
      .lister()
      .then(setEnregistrees)
      .catch(() => setEnregistrees([]));
  }, []);
  useEffect(rafraichirListe, [rafraichirListe]);

  const empiler = useCallback(() => {
    historique.current = [...historique.current.slice(-19), { ...membres }];
  }, [membres]);

  const annuler = useCallback(() => {
    const dernier = historique.current.pop();
    if (dernier) setMembres(dernier);
  }, []);

  useEffect(() => {
    function onTouche(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") {
        e.preventDefault();
        annuler();
      }
    }
    window.addEventListener("keydown", onTouche);
    return () => window.removeEventListener("keydown", onTouche);
  }, [annuler]);

  function placer(joueur: Joueur, creneau: string) {
    empiler();
    setMembres((prec) => ({ ...prec, [creneau]: versMembre(joueur, creneau) }));
    setCreneauActif(null);
  }

  function retirer(creneau: string) {
    empiler();
    setMembres((prec) => {
      const suivant = { ...prec };
      delete suivant[creneau];
      return suivant;
    });
  }

  /** Dépôt : depuis le roster (`chara:<id>`) ou depuis une autre case (`slot:<créneau>`, échange). */
  function deposer(cible: string, charge: string) {
    if (charge.startsWith("chara:")) {
      const j = parId.get(charge.slice(6));
      if (j) placer(j, cible);
      return;
    }
    if (!charge.startsWith("slot:")) return;
    const source = charge.slice(5);
    if (source === cible) return;
    empiler();
    setMembres((prec) => {
      const suivant = { ...prec };
      const a = prec[source];
      const b = prec[cible];
      if (a) suivant[cible] = { ...a, slot: cible };
      else delete suivant[cible];
      if (b) suivant[source] = { ...b, slot: source };
      else delete suivant[source];
      return suivant;
    });
  }

  function taperCase(creneau: string) {
    setCreneauActif((prec) => (prec === creneau ? null : creneau));
  }

  function taperJoueur(j: Joueur) {
    if (creneauActif) placer(j, creneauActif);
  }

  function changerFormation(idx: number) {
    empiler();
    setIndexFormation(idx);
    // Les créneaux de terrain d'une formation ne désignent pas les mêmes postes dans une autre :
    // on garde les remplaçants et l'encadrement, on vide le terrain — comme le wiki.
    setMembres((prec) =>
      Object.fromEntries(Object.entries(prec).filter(([c]) => !c.startsWith("field-"))),
    );
  }

  const synergies = useMemo(
    () => calculateElementSynergies(membres, formation),
    [membres, formation],
  );

  const surTerrain = useMemo(
    () => Object.values(membres).filter((m) => m.slot.startsWith("field-")),
    [membres],
  );

  const agregat = useMemo(() => {
    if (surTerrain.length === 0) return null;
    const totaux = {
      kick: 0, control: 0, technique: 0, pressure: 0,
      physical: 0, agility: 0, intelligence: 0, combatPower: 0,
    };
    for (const m of surTerrain) {
      const r = recalculateMemberStats(
        m,
        niveau,
        m.slot,
        formation,
        synergies.dominantElement,
        synergies.hasHarmony,
      );
      for (const cle of Object.keys(totaux) as (keyof typeof totaux)[]) totaux[cle] += r[cle];
    }
    const n = surTerrain.length;
    return {
      moyennes: Object.fromEntries(
        STATS.map(({ cle }) => [cle, Math.round(totaux[cle] / n)]),
      ) as Record<string, number>,
      puissance: totaux.combatPower,
    };
  }, [surTerrain, niveau, formation, synergies]);

  async function enregistrer() {
    const titre = nom.trim() || "Mon équipe";
    try {
      if (idEquipe) {
        await teamsDb.mettreAJour(idEquipe, titre, formation.id, membres);
        toast.success("Composition mise à jour");
      } else {
        const id = await teamsDb.creer(titre, formation.id, membres);
        setIdEquipe(id);
        toast.success("Composition enregistrée");
      }
      rafraichirListe();
    } catch (e) {
      toast.error(String(e));
    }
  }

  function charger(eq: EquipeEnregistree) {
    empiler();
    const idx = FORMATIONS.findIndex((f) => f.id === eq.formationId);
    setIndexFormation(idx >= 0 ? idx : 0);
    setMembres(eq.membres);
    setNom(eq.nom);
    setIdEquipe(eq.id);
  }

  async function supprimer(eq: EquipeEnregistree) {
    try {
      await teamsDb.supprimer(eq.id);
      if (idEquipe === eq.id) setIdEquipe(null);
      rafraichirListe();
      toast.success("Composition supprimée");
    } catch (e) {
      toast.error(String(e));
    }
  }

  async function copierCode() {
    const slots = Object.values(membres).map((m) => ({ slot: m.slot, charaId: m.charaId }));
    try {
      await writeText(encodeTeamCode(formation.id, slots));
      toast.success("Code d'équipe copié");
    } catch (e) {
      toast.error(String(e));
    }
  }

  async function collerCode() {
    try {
      const texte = (await readText())?.trim();
      if (!texte) return;
      const decode = decodeTeamCode(texte);
      const idx = FORMATIONS.findIndex((f) => f.id === decode.formationId);
      if (idx < 0) {
        toast.error(`Formation inconnue : ${decode.formationId}`);
        return;
      }
      empiler();
      setIndexFormation(idx);
      const suivant: Record<string, TeamMember> = {};
      let manquants = 0;
      for (const s of decode.slots) {
        const j = parId.get(s.charaId);
        if (j) suivant[s.slot] = versMembre(j, s.slot);
        else manquants++;
      }
      setMembres(suivant);
      setIdEquipe(null);
      toast.success(
        manquants > 0
          ? `Code importé — ${manquants} joueur(s) introuvable(s) dans le miroir`
          : "Code importé",
      );
    } catch (e) {
      toast.error(String(e));
    }
  }

  const creneauxBanc = [
    ...Array.from({ length: NB_RESERVES }, (_, i) => ({ creneau: `reserve-${i}`, role: "MF" })),
    { creneau: "manager-0", role: "GK" },
    ...Array.from({ length: NB_SUPPORTS }, (_, i) => ({ creneau: `support-${i}`, role: "DF" })),
  ];

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(240px,300px)_1fr_minmax(220px,280px)] gap-3">
      {/* Roster */}
      <div className="flex min-h-0 flex-col gap-2">
        <Input
          placeholder="Rechercher un joueur…"
          value={requete}
          onChange={(e) => setRequete(e.target.value)}
        />
        <p className="type-label-small text-on-surface-variant">
          {creneauActif
            ? `Créneau ${creneauActif} sélectionné — cliquez un joueur`
            : "Cliquez une case, puis un joueur (ou glissez-déposez)"}
        </p>
        <ScrollArea className="min-h-0 flex-1 rounded-2xl border border-app-line bg-app-dark-box">
          <div className="divide-y divide-app-line">
            {filtres.slice(0, 400).map((j) => (
              <button
                key={j.id}
                type="button"
                draggable
                onDragStart={(e) => e.dataTransfer.setData("text/plain", `chara:${j.id}`)}
                onClick={() => taperJoueur(j)}
                className="state-layer flex w-full items-center gap-2 px-2 py-1.5 text-left type-body-small text-on-surface"
              >
                <span className="min-w-0 flex-1 truncate">{j.nom}</span>
                <Badge variant="outline">{ROLE_LABELS[j.poste] ?? j.poste.slice(0, 3)}</Badge>
                <span className="w-14 shrink-0 truncate type-label-small text-on-surface-variant">
                  {j.element}
                </span>
              </button>
            ))}
            {filtres.length === 0 && (
              <p className="p-3 type-body-small text-on-surface-variant">
                Roster vide — le miroir wiki est-il configuré ?
              </p>
            )}
          </div>
        </ScrollArea>
      </div>

      {/* Terrain */}
      <div className="flex min-h-0 flex-col gap-2">
        <div className="flex flex-wrap items-center gap-2">
          <Input
            className="w-48"
            placeholder="Nom de l'équipe"
            value={nom}
            onChange={(e) => setNom(e.target.value)}
          />
          <Select
            value={String(indexFormation)}
            onValueChange={(v) => v && changerFormation(Number(v))}
          >
            <SelectTrigger className="w-52">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {FORMATIONS.map((f, i) => (
                <SelectItem key={f.id} value={String(i)}>
                  {f.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <button
            type="button"
            className="state-layer rounded-lg px-2 py-1 type-label-medium text-on-surface-variant"
            onClick={() => {
              empiler();
              setMembres(autoRemplir(roster, formation, membres));
            }}
          >
            <Icon name="auto_fix_high" size={15} /> Auto-remplir
          </button>
          <button
            type="button"
            className="state-layer rounded-lg px-2 py-1 type-label-medium text-on-surface-variant"
            onClick={() => {
              empiler();
              setMembres({});
            }}
          >
            <Icon name="delete_sweep" size={15} /> Vider
          </button>
          <button
            type="button"
            className="state-layer rounded-lg px-2 py-1 type-label-medium text-on-surface-variant"
            onClick={annuler}
          >
            <Icon name="undo" size={15} /> Annuler
          </button>
        </div>

        {/* Le terrain reprend les coordonnées `top`/`left` en pourcentage de la formation —
            `top` court de 0 à ~50 (moitié de terrain), d'où le facteur deux. */}
        <div className="relative min-h-0 flex-1 overflow-hidden rounded-2xl border border-app-line bg-app-dark-box">
          <div className="absolute inset-0" aria-hidden>
            <div className="absolute inset-x-0 top-1/2 h-px bg-app-line" />
            <div className="absolute left-1/2 top-1/2 size-24 -translate-x-1/2 -translate-y-1/2 rounded-full border border-app-line" />
          </div>
          {formation.positions.map((p) => (
            <Case
              key={p.index}
              creneau={`field-${p.index}`}
              role={p.role}
              membre={membres[`field-${p.index}`] ?? null}
              actif={creneauActif === `field-${p.index}`}
              onTaper={() => taperCase(`field-${p.index}`)}
              onRetirer={() => retirer(`field-${p.index}`)}
              onDeposer={(charge) => deposer(`field-${p.index}`, charge)}
              gameDir={settings.gameDir}
              style={{
                position: "absolute",
                top: `${p.top * 2}%`,
                left: `${p.left}%`,
                width: "17%",
                height: "17%",
              }}
            />
          ))}
        </div>

        <div className="grid grid-cols-9 gap-1">
          {creneauxBanc.map(({ creneau, role }) => (
            <Case
              key={creneau}
              creneau={creneau}
              role={role}
              membre={membres[creneau] ?? null}
              actif={creneauActif === creneau}
              onTaper={() => taperCase(creneau)}
              onRetirer={() => retirer(creneau)}
              onDeposer={(charge) => deposer(creneau, charge)}
              gameDir={settings.gameDir}
              style={{ height: "72px" }}
            />
          ))}
        </div>
      </div>

      {/* Stats + compositions enregistrées */}
      <div className="flex min-h-0 flex-col gap-2">
        <div className="flex items-center gap-2">
          <Slider
            className="flex-1"
            min={1}
            max={99}
            step={1}
            value={[niveau]}
            onValueChange={(v) => setNiveau((Array.isArray(v) ? v[0] : v) ?? niveau)}
          />
          <Badge variant="secondary">Lv {niveau}</Badge>
        </div>

        <div className="space-y-1 rounded-2xl border border-app-line bg-app-dark-box p-3">
          <div className="flex items-center justify-between">
            <span className="type-label-medium text-on-surface">Composition</span>
            <span className="tabular-nums type-label-small text-on-surface-variant">
              {Object.keys(membres).length} / {formation.positions.length + NB_RESERVES + 1 + NB_SUPPORTS}
            </span>
          </div>
          {synergies.dominantElement && (
            <Badge variant="secondary">Élément dominant : {synergies.dominantElement}</Badge>
          )}
          {synergies.hasHarmony && <Badge variant="outline">Harmonie (4 éléments)</Badge>}
          {agregat ? (
            <>
              {STATS.map(({ cle, libelle }) => (
                <div key={String(cle)} className="flex items-center gap-2 type-label-small">
                  <span className="w-24 shrink-0 text-on-surface-variant">{libelle}</span>
                  <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-surface-container-highest">
                    <div
                      className="h-full rounded-full bg-primary"
                      style={{ width: `${Math.min(100, (agregat.moyennes[cle] / 350) * 100)}%` }}
                    />
                  </div>
                  <span className="w-9 shrink-0 text-right tabular-nums text-on-surface">
                    {agregat.moyennes[cle]}
                  </span>
                </div>
              ))}
              <div className="flex items-center justify-between border-t border-app-line pt-1.5 type-label-medium">
                <span className="text-on-surface">Puissance</span>
                <span className="tabular-nums text-on-surface">{agregat.puissance}</span>
              </div>
            </>
          ) : (
            <p className="type-body-small text-on-surface-variant">Aucun joueur sur le terrain.</p>
          )}
        </div>

        <div className="flex flex-wrap gap-1.5">
          <button
            type="button"
            className="state-layer rounded-lg bg-primary px-2.5 py-1 type-label-medium text-on-primary"
            onClick={enregistrer}
          >
            <Icon name="save" size={15} /> {idEquipe ? "Mettre à jour" : "Enregistrer"}
          </button>
          <button
            type="button"
            className="state-layer rounded-lg px-2.5 py-1 type-label-medium text-on-surface-variant"
            onClick={copierCode}
          >
            <Icon name="content_copy" size={15} /> Copier le code
          </button>
          <button
            type="button"
            className="state-layer rounded-lg px-2.5 py-1 type-label-medium text-on-surface-variant"
            onClick={collerCode}
          >
            <Icon name="content_paste" size={15} /> Coller un code
          </button>
        </div>

        <ScrollArea className="min-h-0 flex-1 rounded-2xl border border-app-line bg-app-dark-box">
          <div className="divide-y divide-app-line">
            {enregistrees.map((eq) => (
              <div key={eq.id} className="flex items-center gap-1 px-2 py-1.5">
                <button
                  type="button"
                  className="min-w-0 flex-1 text-left type-body-small text-on-surface"
                  onClick={() => charger(eq)}
                >
                  <span className="block truncate">{eq.nom}</span>
                  <span className="block truncate type-label-small text-on-surface-variant">
                    {eq.formationId} · {Object.keys(eq.membres).length} membre(s)
                  </span>
                </button>
                <button
                  type="button"
                  aria-label={`Supprimer ${eq.nom}`}
                  className="state-layer shrink-0 rounded-full p-1 text-on-surface-variant"
                  onClick={() => supprimer(eq)}
                >
                  <Icon name="delete" size={14} />
                </button>
              </div>
            ))}
            {enregistrees.length === 0 && (
              <p className="p-3 type-body-small text-on-surface-variant">
                Aucune composition enregistrée. Elles vivent dans `mods.db`, sur cette machine.
              </p>
            )}
          </div>
        </ScrollArea>
      </div>
    </div>
  );
}
