// Estado do documento: definição `.nrep` imutável + histórico ilimitado de desfazer/refazer.
// Imutabilidade com compartilhamento estrutural (immer) mantém o custo de cada snapshot baixo.
import { create } from 'zustand';
import { produce, type Draft } from 'immer';
import type { Band, ReportDefinition, ReportElement } from '../domain/types';
import { DEFAULT_STYLE, blankReport } from '../domain/factory';

interface ReportState {
  report: ReportDefinition;
  filePath?: string;
  dirty: boolean;
  past: ReportDefinition[];
  future: ReportDefinition[];
  /** Último agrupamento de edição (digitação contínua vira um único passo de undo). */
  lastKey?: string;
  lastAt: number;
  update: (recipe: (draft: Draft<ReportDefinition>) => void, coalesceKey?: string) => void;
  undo: () => void;
  redo: () => void;
  load: (report: ReportDefinition, filePath?: string) => void;
  markSaved: (filePath: string) => void;
}

const COALESCE_MS = 800;

export const useReport = create<ReportState>((set, get) => ({
  report: blankReport(),
  dirty: false,
  past: [],
  future: [],
  lastAt: 0,
  update: (recipe, coalesceKey) => {
    const { report, past, lastKey, lastAt } = get();
    const next = produce(report, recipe);
    if (next === report) return;
    const now = Date.now();
    const coalesce = coalesceKey !== undefined && coalesceKey === lastKey && now - lastAt < COALESCE_MS;
    set({
      report: next,
      past: coalesce ? past : [...past, report],
      future: [],
      dirty: true,
      lastKey: coalesceKey,
      lastAt: now,
    });
  },
  undo: () => {
    const { past, report, future } = get();
    const prev = past[past.length - 1];
    if (!prev) return;
    set({ report: prev, past: past.slice(0, -1), future: [report, ...future], dirty: true, lastKey: undefined });
  },
  redo: () => {
    const { past, report, future } = get();
    const next = future[0];
    if (!next) return;
    set({ report: next, past: [...past, report], future: future.slice(1), dirty: true, lastKey: undefined });
  },
  load: (report, filePath) => set({ report: normalize(report), filePath, dirty: false, past: [], future: [], lastKey: undefined }),
  markSaved: (filePath) => set({ filePath, dirty: false }),
}));

/** Completa campos opcionais (estilos parciais, listas ausentes) — espelha os defaults do serde. */
export function normalize(report: ReportDefinition): ReportDefinition {
  return produce(report, (d) => {
    d.dataSources ??= [];
    d.parameters ??= [];
    d.groups ??= [];
    d.bands ??= [];
    for (const b of d.bands) {
      b.elements ??= [];
      for (const e of b.elements) e.style = { ...DEFAULT_STYLE, ...(e.style ?? {}) };
    }
  });
}

// ---- helpers de leitura/edição

export function findElement(report: ReportDefinition, id: string): { band: Band; element: ReportElement } | undefined {
  for (const band of report.bands) {
    const element = band.elements.find((e) => e.id === id);
    if (element) return { band, element };
  }
  return undefined;
}

export function draftElement(draft: Draft<ReportDefinition>, id: string): Draft<ReportElement> | undefined {
  for (const b of draft.bands) {
    const e = b.elements.find((x) => x.id === id);
    if (e) return e;
  }
  return undefined;
}

/** Move um elemento para outra banda (regra da Skill 1.3: `parentBand` atualizado no drop). */
export function moveElementToBand(draft: Draft<ReportDefinition>, id: string, bandId: string, x: number, y: number) {
  const from = draft.bands.find((b) => b.elements.some((e) => e.id === id));
  const to = draft.bands.find((b) => b.id === bandId);
  if (!from || !to) return;
  const idx = from.elements.findIndex((e) => e.id === id);
  const [el] = from.elements.splice(idx, 1);
  el.x = x;
  el.y = y;
  to.elements.push(el);
}
