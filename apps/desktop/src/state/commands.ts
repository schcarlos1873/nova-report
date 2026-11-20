// Comandos do editor: fonte única de comportamento para Ribbon, atalhos de teclado e painéis.
// Cada comando que altera o documento gera exatamente um passo de desfazer.
import type { Draft } from 'immer';
import type { BandType, ElementType, ReportDefinition, ReportElement, Style } from '../domain/types';
import { cloneElements, createBand, createElement, uniqueId } from '../domain/factory';
import { printableWidth, sortBands } from '../domain/geometry';
import { api } from '../infra/api';
import { dialogs } from '../infra/dialogs';
import { draftElement, findElement, useReport } from './reportStore';
import { useUi, ZOOM_MAX, ZOOM_MIN } from './uiStore';

// ---- seleção

export function selectedElements(): ReportElement[] {
  const r = useReport.getState().report;
  return useUi.getState().selectedIds.map((id) => findElement(r, id)?.element).filter((e): e is ReportElement => !!e);
}

/** Tipos com propriedades de texto (fonte, cor, alinhamento). */
export const TEXT_TYPES: ElementType[] = ['label', 'expression', 'table'];
export const hasText = (e: ReportElement) => TEXT_TYPES.includes(e.type);

export const MIXED = Symbol('misto');
export type Mixed<T> = T | typeof MIXED | undefined;

/** Valor comum a todos os elementos; `MIXED` quando divergem; `undefined` sem elementos. */
export function common<T>(els: ReportElement[], get: (e: ReportElement) => T): Mixed<T> {
  if (!els.length) return undefined;
  const first = get(els[0]);
  return els.every((e) => get(e) === first) ? first : MIXED;
}

/** Aplica uma alteração a todos os selecionados que aceitam o filtro — um único passo de undo. */
export function editSelection(key: string, recipe: (e: Draft<ReportElement>) => void, accepts: (e: ReportElement) => boolean = () => true) {
  const ids = selectedElements().filter(accepts).map((e) => e.id);
  if (!ids.length) return;
  useReport.getState().update((d) => {
    for (const id of ids) {
      const e = draftElement(d, id);
      if (e) recipe(e);
    }
  }, `sel:${ids.join(',')}:${key}`);
}

export function setStyle<K extends keyof Style>(k: K, v: Style[K]) {
  const textOnly = ['fontFamily', 'fontSize', 'bold', 'italic', 'underline', 'color', 'align', 'verticalAlign'].includes(k);
  editSelection(`style.${k}`, (e) => void (e.style[k] = v as never), textOnly ? hasText : undefined);
}

export function stepFontSize(delta: number) {
  editSelection('style.fontSize', (e) => void (e.style.fontSize = Math.max(4, Math.min(96, e.style.fontSize + delta))), hasText);
}

export function setFormat(format: string | undefined) {
  editSelection('format', (e) => {
    if (e.type === 'expression') e.format = format || undefined;
  }, (e) => e.type === 'expression');
}

// ---- área de transferência

export function deleteSelection() {
  const ids = useUi.getState().selectedIds;
  if (!ids.length) return;
  useReport.getState().update((d) => {
    for (const b of d.bands) b.elements = b.elements.filter((x) => !ids.includes(x.id));
  });
  useUi.getState().select([], useUi.getState().selectedBandId);
}

export function copySelection() {
  const els = selectedElements();
  if (els.length) useUi.getState().set({ clipboard: structuredClone(els) });
  return els.length;
}

export function cutSelection() {
  if (copySelection()) deleteSelection();
}

function insertClones(source: ReportElement[]) {
  if (!source.length) return;
  const r = useReport.getState().report;
  const { selectedIds, selectedBandId } = useUi.getState();
  const bandId = selectedBandId ?? (selectedIds[0] && findElement(r, selectedIds[0])?.band.id) ?? targetBand(r)?.id;
  if (!bandId) return;
  const clones = cloneElements(r, source);
  useReport.getState().update((d) => void d.bands.find((b) => b.id === bandId)?.elements.push(...clones));
  useUi.getState().select(clones.map((c) => c.id), bandId);
}

export const paste = () => insertClones(useUi.getState().clipboard);
export const duplicate = () => insertClones(selectedElements());

export function selectAll() {
  const r = useReport.getState().report;
  const { selectedBandId } = useUi.getState();
  const band = r.bands.find((b) => b.id === selectedBandId);
  const ids = (band ? band.elements : r.bands.flatMap((b) => b.elements)).map((e) => e.id);
  useUi.getState().select(ids, selectedBandId);
}

export function nudge(dx: number, dy: number) {
  const ids = useUi.getState().selectedIds;
  useReport.getState().update((d) => {
    for (const id of ids) {
      const el = draftElement(d, id);
      if (el) {
        el.x = Math.max(0, Math.round((el.x + dx) * 10) / 10);
        el.y = Math.max(0, Math.round((el.y + dy) * 10) / 10);
      }
    }
  }, 'nudge');
}

// ---- organizar (≥ 2 elementos)

export type ArrangeKind = 'left' | 'right' | 'top' | 'bottom' | 'centerH' | 'sameWidth' | 'sameHeight' | 'distributeH' | 'distributeV';

export function arrange(kind: ArrangeKind) {
  const els = selectedElements();
  if (els.length < 2) return;
  const patch = new Map<string, Partial<ReportElement>>();
  const minX = Math.min(...els.map((e) => e.x));
  const maxR = Math.max(...els.map((e) => e.x + e.width));
  const minY = Math.min(...els.map((e) => e.y));
  const maxB = Math.max(...els.map((e) => e.y + e.height));
  const distribute = (axis: 'x' | 'y') => {
    const size = axis === 'x' ? 'width' : 'height';
    const sorted = [...els].sort((a, b) => a[axis] - b[axis]);
    const first = sorted[0];
    const last = sorted[sorted.length - 1];
    const total = sorted.reduce((s, e) => s + e[size], 0);
    const gap = (last[axis] + last[size] - first[axis] - total) / (sorted.length - 1);
    let p = first[axis];
    for (const e of sorted) {
      patch.set(e.id, { [axis]: Math.round(p * 10) / 10 });
      p += e[size] + gap;
    }
  };
  for (const e of els) {
    switch (kind) {
      case 'left': patch.set(e.id, { x: minX }); break;
      case 'right': patch.set(e.id, { x: maxR - e.width }); break;
      case 'top': patch.set(e.id, { y: minY }); break;
      case 'bottom': patch.set(e.id, { y: maxB - e.height }); break;
      case 'centerH': patch.set(e.id, { x: Math.round(((minX + maxR) / 2 - e.width / 2) * 10) / 10 }); break;
      case 'sameWidth': patch.set(e.id, { width: Math.max(...els.map((x) => x.width)) }); break;
      case 'sameHeight': patch.set(e.id, { height: Math.max(...els.map((x) => x.height)) }); break;
    }
  }
  if (kind === 'distributeH') distribute('x');
  if (kind === 'distributeV') distribute('y');
  useReport.getState().update((d) => {
    for (const [id, p] of patch) {
      const e = draftElement(d, id);
      if (e) Object.assign(e, p);
    }
  });
}

// ---- inserir

/** Banda destino de inserções por clique: a selecionada, a do elemento selecionado ou o detalhe. */
export function targetBand(r: ReportDefinition) {
  const { selectedBandId, selectedIds } = useUi.getState();
  return (
    r.bands.find((b) => b.id === selectedBandId) ??
    (selectedIds[0] ? findElement(r, selectedIds[0])?.band : undefined) ??
    sortBands(r).find((b) => b.type === 'detail') ??
    r.bands[0]
  );
}

export function insertElement(type: ElementType) {
  const r = useReport.getState().report;
  const band = targetBand(r);
  if (!band) return;
  // cascata: evita empilhar elementos novos exatamente no mesmo ponto
  let x = 0;
  let y = 0;
  while (band.elements.some((e) => e.x === x && e.y === y)) {
    x += 2;
    y = Math.min(y + 2, Math.max(0, band.height - 4));
  }
  const el = createElement(r, type, Math.min(x, printableWidth(r) - 10), y);
  useReport.getState().update((d) => void d.bands.find((b) => b.id === band.id)?.elements.push(el));
  useUi.getState().select([el.id], band.id);
}

export const SINGLE_BANDS: BandType[] = ['reportHeader', 'pageHeader', 'detail', 'pageFooter', 'reportFooter'];

export function addBand(type: BandType) {
  const r = useReport.getState().report;
  if (SINGLE_BANDS.includes(type) && r.bands.some((b) => b.type === type)) return;
  const band = createBand(r, type);
  useReport.getState().update((d) => void d.bands.push(band));
  useUi.getState().select([], band.id);
}

export function addGroup() {
  useReport.getState().update((d) => {
    d.groups ??= [];
    let n = d.groups.length + 1;
    while (d.groups.some((g) => g.name === `grupo${n}`)) n++;
    const name = `grupo${n}`;
    d.groups.push({ name, expression: 'Fields.campo' });
    const header = createBand(d as ReportDefinition, 'groupHeader', name);
    d.bands.push(header);
    d.bands.push(createBand(d as ReportDefinition, 'groupFooter', name));
  });
  useUi.getState().set({ leftTab: 'data', showLeft: true });
}

export const ZEBRA_ID_PREFIX = 'zebra';

/**
 * Linhas zebradas na banda de detalhe: um retângulo de fundo com cor alternada por registro
 * (`RECORD_NUMBER()` reinicia a cada grupo). Fica atrás dos demais elementos da banda.
 */
export function zebraDetail(color = '#F1F5F9') {
  const r = useReport.getState().report;
  const band = sortBands(r).find((b) => b.type === 'detail');
  if (!band) return useUi.getState().notify('O relatório não tem banda de detalhe', 'error');
  const existing = band.elements.find((e) => e.id.startsWith(`${ZEBRA_ID_PREFIX}_`));
  if (existing) {
    useUi.getState().select([existing.id], band.id);
    return useUi.getState().notify('O detalhe já está zebrado — ajuste a cor nas propriedades do retângulo');
  }
  const el = createElement(r, 'rectangle', 0, 0, {
    id: uniqueId(r, ZEBRA_ID_PREFIX),
    width: printableWidth(r),
    height: band.height,
    backgroundExpression: `IIF(RECORD_NUMBER() % 2 == 0, "${color}", "#FFFFFF")`,
  });
  el.style = { ...el.style, borderWidth: 0 };
  useReport.getState().update((d) => void d.bands.find((b) => b.id === band.id)?.elements.unshift(el));
  useUi.getState().select([el.id], band.id);
}

// ---- dados

export function addDataSource() {
  useReport.getState().update((d) => {
    let n = d.dataSources.length + 1;
    while (d.dataSources.some((x) => x.id === `ds_${n}`)) n++;
    const taken = new Set(d.dataSources.flatMap((x) => x.queries.map((q) => q.name)));
    let q = 1;
    while (taken.has(`Query${q}`)) q++;
    d.dataSources.push({ id: `ds_${n}`, provider: 'sqlite', options: { path: '' }, queries: [{ name: `Query${q}`, sql: 'SELECT 1 AS exemplo' }] });
  });
  useUi.getState().set({ leftTab: 'data', showLeft: true, mode: 'design' });
}

export function newParameter() {
  useReport.getState().update((d) => {
    let n = d.parameters.length + 1;
    while (d.parameters.some((p) => p.name === `param${n}`)) n++;
    d.parameters.push({ name: `param${n}`, type: 'string', label: `Parâmetro ${n}`, required: false });
  });
  useUi.getState().set({ leftTab: 'data', showLeft: true, mode: 'design' });
}

/** Descobre os campos de todas as queries (1 linha cada) para o painel Dados e o arrastar. */
export async function refreshAllFields() {
  const { report, filePath } = useReport.getState();
  const queries = report.dataSources.flatMap((d) => d.queries.map((q) => q.name));
  if (!queries.length) return useUi.getState().notify('Nenhuma query no relatório', 'error');
  const fields = { ...useUi.getState().fields };
  const failed: string[] = [];
  await Promise.all(
    queries.map(async (q) => {
      try {
        fields[q] = (await api.previewReportQuery(report, q, useUi.getState().paramValues, filePath, 1)).columns;
      } catch (e) {
        failed.push(`${q}: ${(e as Error).message}`);
      }
    }),
  );
  useUi.getState().set({ fields, leftTab: 'data', showLeft: true });
  if (failed.length) useUi.getState().notify(failed.join('\n'), 'error');
  else useUi.getState().notify(`Campos atualizados (${queries.length} quer${queries.length > 1 ? 'ies' : 'y'})`, 'success');
}

// ---- exibir

export function setZoom(z: number) {
  useUi.getState().set({ zoom: Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, +z.toFixed(2))) });
}
export const zoomIn = () => setZoom(useUi.getState().zoom + 0.25);
export const zoomOut = () => setZoom(useUi.getState().zoom - 0.25);

export function toggleMode() {
  const { mode, set } = useUi.getState();
  set({ mode: mode === 'design' ? 'preview' : 'design' });
}

// ---- arquivo

const RECENT_KEY = 'novareport.recent';

export function recentFiles(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]');
    return Array.isArray(v) ? v.filter((x) => typeof x === 'string') : [];
  } catch {
    return [];
  }
}

function pushRecent(path: string) {
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify([path, ...recentFiles().filter((p) => p !== path)].slice(0, 8)));
  } catch {
    /* armazenamento indisponível: lista de recentes é apenas conveniência */
  }
}

const confirmDiscard = async () => !useReport.getState().dirty || (await dialogs.confirm('Descartar alterações não salvas?'));

const resetSession = () =>
  useUi.getState().set({ fields: {}, paramValues: {}, selectedIds: [], selectedBandId: undefined, issues: [] });

export async function newReport() {
  if (!(await confirmDiscard())) return;
  useReport.getState().load(await api.newReport('Novo relatório'));
  resetSession();
}

export async function openReport(path?: string) {
  if (!(await confirmDiscard())) return;
  path ??= (await dialogs.openReport()) ?? undefined;
  if (!path) return;
  try {
    const { report, issues } = await api.openReport(path);
    useReport.getState().load(report, path);
    resetSession();
    useUi.getState().set({ issues });
    pushRecent(path);
  } catch (e) {
    dialogs.error((e as Error).message);
  }
}

export async function save(saveAs = false) {
  const st = useReport.getState();
  let path = st.filePath;
  if (!path || saveAs) {
    path = (await dialogs.saveReport(`${st.report.meta.title || 'relatorio'}.nrep`)) ?? undefined;
    if (!path) return;
    if (!path.toLowerCase().endsWith('.nrep')) path += '.nrep';
  }
  try {
    useUi.getState().set({ issues: await api.saveReport(path, st.report) });
    useReport.getState().markSaved(path);
    pushRecent(path);
    useUi.getState().notify('Relatório salvo', 'success');
  } catch (e) {
    dialogs.error((e as Error).message);
  }
}
