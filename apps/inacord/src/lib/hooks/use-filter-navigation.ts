// Navigation par filtres — portage desktop du hook du wiki (`apps/azalee/lib/hooks/
// use-filter-navigation.ts`), qui écrivait les filtres dans la QUERY STRING de l'URL via
// `useRouter`/`useSearchParams` de Next.
//
// Une application Tauri n'a pas d'URL de page : la WebView ne doit jamais naviguer (elle
// quitterait l'application). L'état des filtres vit donc en mémoire, dans un contexte, et l'API
// reste STRICTEMENT la même — `{ isPending, navigate, navigateWithPage, searchParams }` — pour
// que `FilterChips` et les autres composants portés fonctionnent sans une ligne de changement.
//
// `searchParams` est un vrai `URLSearchParams` : les composants portés le lisent avec `.get()`,
// `.getAll()` et le comparent — la sémantique est identique, seule la persistance change.
import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  useTransition,
} from "react";

interface FilterPendingCtx {
  isPending: boolean;
  startTransition: (cb: () => void) => void;
}

export const FilterTransitionContext = createContext<FilterPendingCtx | null>(null);

/** État des filtres partagé par les composants montés sous le même écran. */
interface FilterStateCtx {
  params: URLSearchParams;
  setParams: (p: URLSearchParams) => void;
}

export const FilterStateContext = createContext<FilterStateCtx | null>(null);

/**
 * À appeler une fois dans la vue qui porte les filtres, et à passer aux deux fournisseurs
 * (`FilterTransitionContext` et `FilterStateContext`). Sans fournisseur, chaque composant garde
 * son propre état local — le comportement de repli du hook amont.
 */
export function useFilterState(initial?: string) {
  const [params, setParams] = useState(() => new URLSearchParams(initial ?? ""));
  const [isPending, startTransition] = useTransition();
  return useMemo(
    () => ({
      etat: { params, setParams } satisfies FilterStateCtx,
      transition: { isPending, startTransition } satisfies FilterPendingCtx,
    }),
    [params, isPending],
  );
}

/** Compat amont : le wiki appelait ceci pour partager la transition entre les puces. */
export function useFilterTransition() {
  const [isPending, startTransition] = useTransition();
  return useMemo<FilterPendingCtx>(() => ({ isPending, startTransition }), [isPending]);
}

/**
 * Hook consommé par chaque puce de filtre — même signature que la version web.
 *
 * `navigate` retire la pagination (`page`) comme l'amont : changer un filtre remet à la première
 * page. `navigateWithPage` la conserve.
 */
export function useFilterNavigation() {
  const partage = useContext(FilterTransitionContext);
  const etat = useContext(FilterStateContext);
  const [localPending, localStartTransition] = useTransition();
  const [locaux, setLocaux] = useState(() => new URLSearchParams());

  const isPending = partage ? partage.isPending : localPending;
  const startTransition = partage ? partage.startTransition : localStartTransition;
  const searchParams = etat ? etat.params : locaux;
  const appliquer = etat ? etat.setParams : setLocaux;

  const majuscule = useCallback(
    (updater: (params: URLSearchParams) => void, retirerPage: boolean) => {
      const params = new URLSearchParams(searchParams.toString());
      updater(params);
      if (retirerPage) params.delete("page");
      startTransition(() => appliquer(params));
    },
    [searchParams, appliquer, startTransition],
  );

  const navigate = useCallback(
    (updater: (params: URLSearchParams) => void) => majuscule(updater, true),
    [majuscule],
  );
  const navigateWithPage = useCallback(
    (updater: (params: URLSearchParams) => void) => majuscule(updater, false),
    [majuscule],
  );

  return { isPending, navigate, navigateWithPage, searchParams };
}
