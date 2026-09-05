// « Qui regarde ? » — l'écran de sélection de profil de la médiathèque.
//
// C'est la première chose que montrent Netflix et Disney+, et pour une raison qui vaut ici : la
// progression, les reprises et « ma liste » n'ont de sens que rapportées à quelqu'un. Le choix se
// fait une fois par session (`lireProfilActif` mémorise le dernier), pas à chaque ouverture de la
// vue — l'écran ne réapparaît que si personne n'a encore choisi, ou si l'on demande à changer.
//
// L'avatar n'est pas une photo : c'est un dégradé et un emblème. Un portrait de personnage
// viendrait du VFS, donc coûterait un décodage de texture par profil au tout premier écran de la
// vue — pour une image que personne ne regarde plus après trois secondes. Le dégradé se rend en
// une passe CSS et reste lisible dans les deux thèmes.
import { useState, type ReactNode } from "react";

import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/Icon";
import { Input } from "@/components/ui/input";
import {
  degrade,
  ecrireProfilActif,
  ecrireProfils,
  EMBLEMES,
  nouvelId,
  oublierProfil,
  PALETTE,
  PROFIL_PRINCIPAL,
  profilPrincipal,
  type Profil,
} from "@/lib/profils";
import { cn } from "@/lib/utils";

/** L'avatar seul — réemployé par la barre de la vue Cinéma, en petit. */
export function AvatarProfil({
  profil,
  taille = 128,
  className,
}: {
  profil: Profil;
  taille?: number;
  className?: string;
}) {
  return (
    <div
      className={cn("flex shrink-0 items-center justify-center overflow-hidden rounded-lg", className)}
      style={{ width: taille, height: taille, backgroundImage: degrade(profil) }}
    >
      <Icon name={profil.embleme} size={Math.round(taille * 0.46)} className="text-white/90" />
    </div>
  );
}

export function ChoixProfil({
  profils,
  onProfils,
  onChoisir,
  onAnnuler,
}: {
  profils: Profil[];
  onProfils: (p: Profil[]) => void;
  onChoisir: (id: string) => void;
  /** Présent seulement quand un profil est déjà actif : on peut alors renoncer à en changer. */
  onAnnuler?: () => void;
}) {
  const [gestion, setGestion] = useState(false);
  const [edite, setEdite] = useState<Profil | null>(null);

  const enregistrer = (p: Profil) => {
    const existe = profils.some((x) => x.id === p.id);
    const suivant = existe ? profils.map((x) => (x.id === p.id ? p : x)) : [...profils, p];
    ecrireProfils(suivant);
    onProfils(suivant);
    setEdite(null);
  };

  const supprimer = (id: string) => {
    const suivant = profils.filter((p) => p.id !== id);
    ecrireProfils(suivant);
    oublierProfil(id);
    onProfils(suivant);
    setEdite(null);
    if (suivant.length === 0) ecrireProfilActif(null);
  };

  // Aucun profil enregistré : plutôt qu'un écran vide, on en propose un tout fait. Demander de
  // remplir un formulaire avant de pouvoir regarder quoi que ce soit serait une porte fermée là
  // où les deux plateformes de référence en ouvrent une.
  if (profils.length === 0 && !edite) {
    return (
      <Cadre>
        <h1 className="text-3xl font-semibold text-ink">Qui regarde&nbsp;?</h1>
        <p className="mt-2 max-w-md text-center text-sm text-ink-dull">
          Chaque profil garde sa progression, ses reprises de lecture et sa liste. Rien n'est
          partagé entre eux, rien ne sort de cette machine.
        </p>
        <div className="mt-8 flex gap-6">
          <BoutonAjout onClick={() => setEdite(profilPrincipal())} />
        </div>
      </Cadre>
    );
  }

  if (edite) {
    return (
      <Cadre>
        <EditeurProfil
          profil={edite}
          suppressible={edite.id !== PROFIL_PRINCIPAL && profils.some((p) => p.id === edite.id)}
          onEnregistrer={enregistrer}
          onSupprimer={() => supprimer(edite.id)}
          onAnnuler={() => setEdite(null)}
        />
      </Cadre>
    );
  }

  return (
    <Cadre>
      <h1 className="text-3xl font-semibold text-ink">Qui regarde&nbsp;?</h1>
      <div className="mt-10 flex flex-wrap items-start justify-center gap-6">
        {profils.map((p) => (
          <div key={p.id} className="flex w-32 flex-col items-center gap-2">
            <button
              type="button"
              onClick={() => (gestion ? setEdite(p) : onChoisir(p.id))}
              className="group relative rounded-lg outline-none ring-offset-4 ring-offset-app focus-visible:ring-2 focus-visible:ring-accent"
              title={gestion ? `Modifier ${p.nom}` : `Regarder en tant que ${p.nom}`}
            >
              <AvatarProfil
                profil={p}
                className="transition-all duration-150 group-hover:scale-105 group-hover:ring-2 group-hover:ring-ink/70"
              />
              {gestion && (
                <span className="absolute inset-0 flex items-center justify-center rounded-lg bg-black/55">
                  <Icon name="edit" size={32} className="text-white" />
                </span>
              )}
            </button>
            <span className="truncate text-center text-sm text-ink-dull" title={p.nom}>
              {p.nom}
            </span>
            {p.jeunesse && <span className="text-tiny uppercase text-ink-faint">jeunesse</span>}
          </div>
        ))}
        {profils.length < 6 && (
          <div className="flex w-32 flex-col items-center gap-2">
            <BoutonAjout
              onClick={() =>
                setEdite({
                  id: nouvelId(profils),
                  nom: "",
                  couleur: profils.length % PALETTE.length,
                  embleme: EMBLEMES[profils.length % EMBLEMES.length] ?? "person",
                })
              }
            />
            <span className="text-sm text-ink-faint">Ajouter</span>
          </div>
        )}
      </div>
      <div className="mt-12 flex items-center gap-2">
        <Button variant="outline" onClick={() => setGestion((v) => !v)}>
          <Icon name={gestion ? "check" : "edit"} size={16} />
          {gestion ? "Terminé" : "Gérer les profils"}
        </Button>
        {onAnnuler && (
          <Button variant="ghost" onClick={onAnnuler}>
            Revenir
          </Button>
        )}
      </div>
    </Cadre>
  );
}

function Cadre({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full min-h-0 flex-col items-center justify-center overflow-y-auto bg-app px-6 py-10">
      {children}
    </div>
  );
}

function BoutonAjout({ onClick }: { onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label="Ajouter un profil"
      className="flex size-32 items-center justify-center rounded-lg border-2 border-dashed border-app-line text-ink-faint transition-colors hover:border-accent hover:text-accent"
    >
      <Icon name="add" size={40} />
    </button>
  );
}

// ── Édition d'un profil ───────────────────────────────────────────────────────

function EditeurProfil({
  profil,
  suppressible,
  onEnregistrer,
  onSupprimer,
  onAnnuler,
}: {
  profil: Profil;
  suppressible: boolean;
  onEnregistrer: (p: Profil) => void;
  onSupprimer: () => void;
  onAnnuler: () => void;
}) {
  const [brouillon, setBrouillon] = useState<Profil>(profil);
  const nom = brouillon.nom.trim();

  return (
    <div className="w-full max-w-lg">
      <h1 className="text-center text-2xl font-semibold text-ink">
        {suppressible ? "Modifier le profil" : "Nouveau profil"}
      </h1>

      <div className="mt-8 flex items-center gap-5">
        <AvatarProfil profil={brouillon} taille={96} />
        <div className="min-w-0 flex-1">
          <label className="text-tiny uppercase tracking-wider text-ink-faint" htmlFor="profil-nom">
            Nom
          </label>
          <Input
            id="profil-nom"
            value={brouillon.nom}
            autoFocus
            maxLength={24}
            placeholder="Mark, Axel, Jude…"
            onChange={(e) => setBrouillon({ ...brouillon, nom: e.target.value })}
            onKeyDown={(e) => {
              if (e.key === "Enter" && nom) onEnregistrer({ ...brouillon, nom });
            }}
            className="mt-1"
          />
          {/* Le profil jeunesse masque la fiche technique — codec, octets, chemin VFS. Il ne
              restreint aucun contenu et ne prétend pas le faire : ce serait un verrou sans
              serrure, puisque tout est déjà sur le disque de celui qui l'ouvre. */}
          <label className="mt-3 flex items-center gap-2 text-xs text-ink-dull">
            <input
              type="checkbox"
              checked={brouillon.jeunesse ?? false}
              onChange={(e) => setBrouillon({ ...brouillon, jeunesse: e.target.checked })}
              className="accent-[var(--color-accent,#3b82f6)]"
            />
            Profil jeunesse — masquer la fiche technique
          </label>
        </div>
      </div>

      <div className="mt-6">
        <div className="text-tiny uppercase tracking-wider text-ink-faint">Couleur</div>
        <div className="mt-2 flex flex-wrap gap-2">
          {PALETTE.map((p, i) => (
            <button
              key={p.nom}
              type="button"
              title={p.nom}
              aria-label={p.nom}
              aria-pressed={brouillon.couleur === i}
              onClick={() => setBrouillon({ ...brouillon, couleur: i })}
              className={cn(
                "size-9 rounded-full transition-transform",
                brouillon.couleur === i ? "scale-110 ring-2 ring-ink ring-offset-2 ring-offset-app" : "hover:scale-105",
              )}
              style={{ backgroundImage: `linear-gradient(135deg, ${p.de} 0%, ${p.vers} 100%)` }}
            />
          ))}
        </div>
      </div>

      <div className="mt-5">
        <div className="text-tiny uppercase tracking-wider text-ink-faint">Emblème</div>
        <div className="mt-2 flex flex-wrap gap-2">
          {EMBLEMES.map((nomIcone) => (
            <button
              key={nomIcone}
              type="button"
              aria-label={nomIcone}
              aria-pressed={brouillon.embleme === nomIcone}
              onClick={() => setBrouillon({ ...brouillon, embleme: nomIcone })}
              className={cn(
                "flex size-9 items-center justify-center rounded-md border transition-colors",
                brouillon.embleme === nomIcone
                  ? "border-accent bg-accent/15 text-accent"
                  : "border-app-line text-ink-dull hover:border-ink-faint hover:text-ink",
              )}
            >
              <Icon name={nomIcone} size={18} />
            </button>
          ))}
        </div>
      </div>

      <div className="mt-8 flex items-center gap-2">
        <Button disabled={!nom} onClick={() => onEnregistrer({ ...brouillon, nom })}>
          Enregistrer
        </Button>
        <Button variant="ghost" onClick={onAnnuler}>
          Annuler
        </Button>
        <div className="flex-1" />
        {suppressible && (
          <Button variant="outline" onClick={onSupprimer} title="Supprime aussi sa progression">
            <Icon name="delete" size={16} />
            Supprimer
          </Button>
        )}
      </div>
    </div>
  );
}
