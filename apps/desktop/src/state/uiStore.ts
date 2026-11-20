import { create } from 'zustand';
import type { AppInfo, ConnectionProfile, ReportElement } from '../domain/types';

export type LeftTab = 'toolbox' | 'data' | 'connections';
export type Mode = 'design' | 'preview';

interface UiState {
  info?: AppInfo;
  mode: Mode;
  leftTab: LeftTab;
  zoom: number;
  gridMm: number;
  snapToGrid: boolean;
  showGrid: boolean;
  selectedIds: string[];
  selectedBandId?: string;
  clipboard: ReportElement[];
  /** Campos conhecidos por query (descobertos via preview). */
  fields: Record<string, { name: string; dataType: string }[]>;
  /** Valores dos parâmetros usados no preview. */
  paramValues: Record<string, unknown>;
  connections: ConnectionProfile[];
  toast?: { text: string; kind: 'info' | 'error' | 'success'; at: number };
  set: (patch: Partial<UiState>) => void;
  select: (ids: string[], bandId?: string) => void;
  toggle: (id: string) => void;
  notify: (text: string, kind?: 'info' | 'error' | 'success') => void;
}

export const useUi = create<UiState>((set, get) => ({
  mode: 'design',
  leftTab: 'toolbox',
  zoom: 1,
  gridMm: 1,
  snapToGrid: true,
  showGrid: true,
  selectedIds: [],
  clipboard: [],
  fields: {},
  paramValues: {},
  connections: [],
  set: (patch) => set(patch),
  select: (ids, bandId) => set({ selectedIds: ids, selectedBandId: bandId }),
  toggle: (id) => {
    const cur = get().selectedIds;
    set({ selectedIds: cur.includes(id) ? cur.filter((x) => x !== id) : [...cur, id] });
  },
  notify: (text, kind = 'info') => set({ toast: { text, kind, at: Date.now() } }),
}));
