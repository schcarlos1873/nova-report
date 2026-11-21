import { create } from 'zustand';
import type { AppInfo, ConnectionProfile, ReportElement, ValidationIssue } from '../domain/types';

export type Mode = 'design' | 'preview';
export type RibbonTab = 'home' | 'insert' | 'data' | 'layout' | 'view';

/** Diálogo aberto a partir da árvore "Dados do Relatório" (um por vez). `target` é o nome do item editado; ausente = novo. */
export type DataDialog =
  | { kind: 'parameter'; target?: string }
  | { kind: 'dataSource'; target?: string }
  | { kind: 'dataset'; target?: string; dataSource?: string }
  | { kind: 'image'; target?: string }
  | { kind: 'group'; target?: string }
  | { kind: 'connections' };

export const LEFT_MIN = 200;
export const LEFT_MAX = 520;
const LEFT_KEY = 'novareport.leftWidth';

function storedLeftWidth() {
  try {
    const n = Number(localStorage.getItem(LEFT_KEY));
    return n >= LEFT_MIN && n <= LEFT_MAX ? n : 280;
  } catch {
    return 280;
  }
}

export function persistLeftWidth(w: number) {
  try {
    localStorage.setItem(LEFT_KEY, String(Math.round(w)));
  } catch {
    /* preferência apenas local */
  }
}

export interface FieldInfo {
  name: string;
  dataType: string;
}

interface UiState {
  info?: AppInfo;
  mode: Mode;
  /** Largura da barra "Dados do Relatório" (px). */
  leftWidth: number;
  /** Nós expandidos da árvore (id → aberto). Ausente = padrão do nó. */
  treeExpanded: Record<string, boolean>;
  dialog?: DataDialog;
  ribbonTab: RibbonTab;
  ribbonCollapsed: boolean;
  showLeft: boolean;
  showRight: boolean;
  zoom: number;
  gridMm: number;
  snapToGrid: boolean;
  showGrid: boolean;
  selectedIds: string[];
  selectedBandId?: string;
  clipboard: ReportElement[];
  /** Campos conhecidos por query (descobertos via preview). */
  fields: Record<string, FieldInfo[]>;
  /** Valores dos parâmetros usados no preview. */
  paramValues: Record<string, unknown>;
  connections: ConnectionProfile[];
  issues: ValidationIssue[];
  showIssues: boolean;
  toast?: { text: string; kind: 'info' | 'error' | 'success'; at: number };
  set: (patch: Partial<UiState>) => void;
  select: (ids: string[], bandId?: string) => void;
  toggle: (id: string) => void;
  notify: (text: string, kind?: 'info' | 'error' | 'success') => void;
}

export const useUi = create<UiState>((set, get) => ({
  mode: 'design',
  leftWidth: storedLeftWidth(),
  treeExpanded: {},
  ribbonTab: 'home',
  ribbonCollapsed: false,
  showLeft: true,
  showRight: true,
  zoom: 1,
  gridMm: 1,
  snapToGrid: true,
  showGrid: true,
  selectedIds: [],
  clipboard: [],
  fields: {},
  paramValues: {},
  connections: [],
  issues: [],
  showIssues: false,
  set: (patch) => set(patch),
  select: (ids, bandId) => set({ selectedIds: ids, selectedBandId: bandId }),
  toggle: (id) => {
    const cur = get().selectedIds;
    set({ selectedIds: cur.includes(id) ? cur.filter((x) => x !== id) : [...cur, id] });
  },
  notify: (text, kind = 'info') => set({ toast: { text, kind, at: Date.now() } }),
}));

export const ZOOM_MIN = 0.5;
export const ZOOM_MAX = 4;
