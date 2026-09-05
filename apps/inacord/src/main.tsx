import React from "react";
import ReactDOM from "react-dom/client";
import { ThemeProvider } from "next-themes";
import App from "@/App";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import "./styles.css";

// Barrière RACINE : sans elle, la moindre exception non rattrapée (rendu OU effet) vide
// `#root` et la fenêtre devient blanche — indiscernable d'un crash du processus. Les zones à
// risque ont en plus leur propre barrière, pour que la panne y reste locale.
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary zone="Application">
      <ThemeProvider attribute="class" defaultTheme="dark" enableSystem>
        <App />
      </ThemeProvider>
    </ErrorBoundary>
  </React.StrictMode>,
);
