/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** « 1 » dans la version Mac App Store (`scripts/mas.sh`), absent ailleurs. */
  readonly VITE_COLOPHON_MAS?: string;
}
