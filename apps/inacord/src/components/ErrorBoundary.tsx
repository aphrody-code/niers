// Barrière d'erreur — ce qui sépare « un panneau affiche une erreur » de « l'application a
// crashé ».
//
// React 19 démonte l'ARBRE ENTIER quand une exception traverse un rendu **ou un effet** sans
// rencontrer de barrière. Sans elle, un `new THREE.WebGLRenderer()` qui échoue (WebView2 sans
// accélération matérielle), un GLB refusé par `GLTFLoader.parse` ou n'importe quel accès à une
// propriété d'un objet libéré vide le `<div id="root">` : la fenêtre devient blanche, sans
// message, sans journal, sans rien à rapporter. C'est exactement ce que l'utilisatrice décrit
// quand elle dit que l'aperçu « crashe ».
//
// Deux emplois, tous deux nécessaires :
//   * autour de l'application entière (`main.tsx`) — dernier filet, rien ne doit pouvoir blanchir
//     la fenêtre ;
//   * autour des zones à risque (viewport 3D, panneau de détail) — la panne reste LOCALE, le
//     reste de l'interface continue de fonctionner.
//
// `resetKeys` permet de repartir tout seul quand la cause change (autre asset sélectionné) : sans
// ça, une erreur sur un fichier condamnerait le panneau jusqu'au redémarrage.
import { Component, type ErrorInfo, type ReactNode } from "react";

export interface ErrorBoundaryProps {
  children: ReactNode;
  /** Nom de la zone protégée, affiché dans le message (« Aperçu 3D », « Panneau de détail »…). */
  zone?: string;
  /** Change une de ces valeurs et la barrière se réarme — typiquement le chemin de l'asset. */
  resetKeys?: unknown[];
  /** Rendu de repli personnalisé. Par défaut : la carte d'erreur ci-dessous. */
  fallback?: (erreur: Error, reessayer: () => void) => ReactNode;
}

interface ErrorBoundaryState {
  erreur: Error | null;
  /** Signature des `resetKeys` au moment de l'erreur, pour détecter un changement. */
  cles: string;
}

function signature(keys: unknown[] | undefined): string {
  try {
    return JSON.stringify(keys ?? []);
  } catch {
    return String(keys);
  }
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { erreur: null, cles: signature(props.resetKeys) };
  }

  static getDerivedStateFromError(erreur: Error): Partial<ErrorBoundaryState> {
    return { erreur };
  }

  static getDerivedStateFromProps(
    props: ErrorBoundaryProps,
    state: ErrorBoundaryState,
  ): Partial<ErrorBoundaryState> | null {
    const cles = signature(props.resetKeys);
    if (cles !== state.cles) return { erreur: null, cles };
    return null;
  }

  componentDidCatch(erreur: Error, info: ErrorInfo) {
    // La console du WebView est le SEUL endroit où la pile survit : la carte ci-dessous n'affiche
    // que le message, et une erreur non journalisée est une erreur non diagnosticable.
    console.error(`[${this.props.zone ?? "app"}]`, erreur, info.componentStack);
  }

  render() {
    const { erreur } = this.state;
    if (!erreur) return this.props.children;

    const reessayer = () => this.setState({ erreur: null });
    if (this.props.fallback) return this.props.fallback(erreur, reessayer);

    return (
      <div className="flex h-full min-h-0 items-center justify-center p-4">
        <div className="max-w-lg rounded-lg border border-app-line bg-app-box p-4 text-xs">
          <p className="mb-1 font-semibold text-status-error">
            {this.props.zone ? `${this.props.zone} : panne` : "Panne de l'interface"}
          </p>
          <p className="mb-2 text-ink-dull">
            Cette zone s&apos;est arrêtée sur une erreur. Le reste de l&apos;application continue de
            fonctionner.
          </p>
          <pre className="mb-3 max-h-40 overflow-auto whitespace-pre-wrap rounded border border-app-line bg-app-dark-box p-2 font-mono text-[11px] text-ink">
            {erreur.message || String(erreur)}
          </pre>
          <div className="flex gap-2">
            <button
              type="button"
              className="rounded border border-app-line px-2 py-1 text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
              onClick={reessayer}
            >
              Réessayer
            </button>
            <button
              type="button"
              className="rounded border border-app-line px-2 py-1 text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
              onClick={() => {
                navigator.clipboard?.writeText(`${erreur.message}\n${erreur.stack ?? ""}`).catch(() => {});
              }}
            >
              Copier le détail
            </button>
          </div>
        </div>
      </div>
    );
  }
}
