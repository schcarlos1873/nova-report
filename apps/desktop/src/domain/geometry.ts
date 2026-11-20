import type { Band, BandType, ReportDefinition } from './types';

/** DPI de referência da tela (CSS px). 1 mm = DPI / 25.4 px no zoom 100%. */
export const SCREEN_DPI = 96;
export const PT_TO_MM = 25.4 / 72;

export const pxPerMm = (zoom: number) => (SCREEN_DPI / 25.4) * zoom;
export const mmToPx = (mm: number, zoom: number) => mm * pxPerMm(zoom);
/** Conversão inversa exata: mm = px · 25.4 / (DPI · zoom). */
export const pxToMm = (px: number, zoom: number) => px / pxPerMm(zoom);

export const round = (v: number, step = 0.1) => Math.round(v / step) * step;
export const snap = (v: number, grid: number) => (grid > 0 ? Math.round(v / grid) * grid : v);
export const clamp = (v: number, min: number, max: number) => Math.min(max, Math.max(min, v));

const PAPER: Record<string, [number, number]> = {
  A3: [297, 420],
  A4: [210, 297],
  A5: [148, 210],
  Letter: [215.9, 279.4],
  Legal: [215.9, 355.6],
};

export function pageSize(report: ReportDefinition): { width: number; height: number } {
  const [w, h] = PAPER[report.meta.paperSize] ?? [report.meta.customSize?.width ?? 210, report.meta.customSize?.height ?? 297];
  const [a, b] = [Math.min(w, h), Math.max(w, h)];
  return report.meta.orientation === 'landscape' ? { width: b, height: a } : { width: a, height: b };
}

export function printableWidth(report: ReportDefinition): number {
  const { width } = pageSize(report);
  return width - report.meta.margins.left - report.meta.margins.right;
}

export const BAND_LABELS: Record<BandType, string> = {
  reportHeader: 'Cabeçalho do relatório',
  pageHeader: 'Cabeçalho de página',
  groupHeader: 'Cabeçalho de grupo',
  detail: 'Detalhe',
  groupFooter: 'Rodapé de grupo',
  pageFooter: 'Rodapé de página',
  reportFooter: 'Rodapé do relatório',
};

export const BAND_COLORS: Record<BandType, string> = {
  reportHeader: '#7C3AED',
  pageHeader: '#2563EB',
  groupHeader: '#0D9488',
  detail: '#16A34A',
  groupFooter: '#0D9488',
  pageFooter: '#2563EB',
  reportFooter: '#7C3AED',
};

export function bandTitle(b: Band): string {
  const base = BAND_LABELS[b.type];
  if (b.group) return `${base}: ${b.group}`;
  if (b.dataSource) return `${base} — ${b.dataSource}`;
  return base;
}

const ORDER: BandType[] = ['reportHeader', 'pageHeader', 'groupHeader', 'detail', 'groupFooter', 'pageFooter', 'reportFooter'];

/** Mesma ordenação canônica do domínio Rust (headers externos→internos, footers internos→externos). */
export function sortBands(report: ReportDefinition): Band[] {
  const groups = (report.groups ?? []).map((g) => g.name);
  const gpos = (b: Band) => (b.group ? groups.indexOf(b.group) : 0);
  return [...report.bands].sort((a, b) => {
    const d = ORDER.indexOf(a.type) - ORDER.indexOf(b.type);
    if (d !== 0) return d;
    if (a.type === 'groupHeader') return gpos(a) - gpos(b);
    if (a.type === 'groupFooter') return gpos(b) - gpos(a);
    return 0;
  });
}
