// Gestionnaire de jobs — portage du `JobManagerPopover`/`JobCard` de
// `var/spacedrive/packages/interface/src/components/JobManager/` : un `CircleButton` en pied de
// barre latérale (icône qui tourne tant qu'un job tourne), un popover listant les jobs récents
// avec statut, barre de progression et erreur.
//
// La source de vérité est la table `jobs` de `mods.db` (cf. `lib/jobsDb.ts`), pas un état React :
// c'est ce qui rend le suivi survivant à un changement d'onglet et à un redémarrage de l'app.
import { CircleButton } from "@/components/ui/circle-button";
import { Icon } from "@/components/ui/Icon";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Progress } from "@/components/ui/progress";
import { jobsDb, useJobs, type JobRow, type JobStatus } from "@/lib/jobsDb";
import { cn } from "@/lib/utils";

const STATUS_LABEL: Record<JobStatus, string> = {
  running: "en cours",
  done: "terminé",
  error: "échec",
  canceled: "annulé",
  interrupted: "interrompu",
};

const STATUS_CLASS: Record<JobStatus, string> = {
  running: "text-accent",
  done: "text-status-success",
  error: "text-status-error",
  canceled: "text-ink-faint",
  interrupted: "text-status-warning",
};

const STATUS_ICON: Record<JobStatus, string> = {
  running: "update",
  done: "check_circle",
  error: "error",
  canceled: "block",
  interrupted: "warning",
};

function JobCard({ job }: { job: JobRow }) {
  const pct = job.total > 0 ? Math.min(100, Math.round((job.progress / job.total) * 100)) : 0;
  return (
    <div className="rounded-lg border border-app-line/60 bg-app-box p-2.5">
      <div className="flex items-start gap-2">
        <span className={cn("mt-0.5 shrink-0", STATUS_CLASS[job.status])}>
          <Icon name={STATUS_ICON[job.status]} size={15} className={job.status === "running" ? "animate-spin" : ""} />
        </span>
        <div className="min-w-0 flex-1">
          <p className="truncate text-xs font-medium text-ink">{job.label || job.kind}</p>
          <p className={cn("text-tiny", STATUS_CLASS[job.status])}>
            {STATUS_LABEL[job.status]}
            {job.total > 0 && ` · ${job.progress.toLocaleString("fr-FR")} / ${job.total.toLocaleString("fr-FR")}`}
          </p>
          {job.status === "running" && job.total > 0 && <Progress value={pct} className="mt-1.5" />}
          {job.error && <p className="mt-1 text-tiny text-status-error">{job.error}</p>}
        </div>
      </div>
    </div>
  );
}

export function JobManagerButton() {
  const jobs = useJobs();
  const running = jobs.filter((j) => j.status === "running").length;

  return (
    <Popover>
      <PopoverTrigger
        render={
          <CircleButton
            icon={running > 0 ? "update" : "task_alt"}
            size="sm"
            variant={running > 0 ? "accent" : "default"}
            title={running > 0 ? `${running} opération(s) en cours` : "Gestionnaire d'opérations"}
            aria-label="Gestionnaire d'opérations"
          />
        }
      />
      <PopoverContent className="w-80">
        <div className="flex items-center justify-between pb-1">
          <span className="text-xs font-semibold text-ink">Opérations</span>
          <button
            type="button"
            className="text-tiny text-ink-faint transition-colors hover:text-ink-dull"
            onClick={() => void jobsDb.clearFinished()}
          >
            Vider les terminées
          </button>
        </div>
        <div className="no-scrollbar flex max-h-80 flex-col gap-1.5 overflow-y-auto">
          {jobs.length === 0 ? (
            <p className="py-4 text-center text-xs text-ink-faint">Aucune opération enregistrée.</p>
          ) : (
            jobs.map((j) => <JobCard key={j.id} job={j} />)
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}
