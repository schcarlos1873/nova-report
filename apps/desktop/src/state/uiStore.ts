import { create } from 'zustand';
import type { AppInfo, ConnectionProfile, ReportElement, ValidationIssue } from '../domain/types';

export type LeftTab = 'toolbox' | 'data' | 'connections';
export type Mode = 'design' | 'preview';
export type RibbonTab = 'home' | 'insert' | 'data' | 'layout' | 'view';

export interface FieldInfo {
  name: string;
  dataType: string;
}

interface UiState {
  info?: AppInfo;
  mode: Mode;
  leftTab: LeftTab;
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
  leftTab: 'toolbox',
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
