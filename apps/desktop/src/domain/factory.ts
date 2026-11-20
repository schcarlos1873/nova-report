import type { Band, BandType, ElementType, ReportDefinition, ReportElement, Style } from './types';

export const DEFAULT_STYLE: Style = {
  fontFamily: 'Arial',
  fontSize: 10,
  bold: false,
  italic: false,
  underline: false,
  color: '#000000',
  align: 'left',
  verticalAlign: 'top',
  borderWidth: 0,
  borderColor: '#000000',
  padding: 0,
};

export function uniqueId(report: ReportDefinition, prefix: string): string {
  const used = new Set(report.bands.flatMap((b) => [b.id, ...b.elements.map((e) => e.id)]));
  let n = 1;
  while (used.has(`${prefix}_${n}`)) n++;
  return `${prefix}_${n}`;
}

const PREFIX: Record<ElementType, string> = {
  label: 'txt',
  expression: 'fld',
  image: 'img',
  line: 'lin',
  rectangle: 'rct',
  table: 'tbl',
};

export function createElement(
  report: ReportDefinition,
  type: ElementType,
  x: number,
  y: number,
  extra: Partial<ReportElement> = {},
): ReportElement {
  const id = uniqueId(report, PREFIX[type]);
  const base = { id, x, y, style: { ...DEFAULT_STYLE } };
  const queries = report.dataSources.flatMap((d) => d.queries.map((q) => q.name));
  let el: ReportElement;
  switch (type) {
    case 'label':
      el = { ...base, type, text: 'Texto', width: 40, height: 6 };
      break;
    case 'expression':
      el = { ...base, type, expression: 'Fields.campo', width: 40, height: 6 };
      break;
    case 'image':
      el = { ...base, type, source: '', fit: 'contain', width: 30, height: 15 };
      break;
    case 'line':
      el = { ...base, type, direction: 'horizontal', width: 60, height: 0, style: { ...DEFAULT_STYLE, borderWidth: 0.5 } };
      break;
    case 'rectangle':
      el = { ...base, type, radius: 0, width: 40, height: 15, style: { ...DEFAULT_STYLE, borderWidth: 0.5 } };
      break;
    case 'table':
      el = {
        ...base,
        type,
        dataSource: queries[0] ?? '',
        columns: [
          { header: 'Coluna 1', expression: 'Fields.campo', width: 60 },
          { header: 'Coluna 2', expression: 'Fields.valor', width: 40, align: 'right' },
        ],
        repeatHeader: true,
        headerBackground: '#E2E8F0',
        width: 100,
        height: 20,
        style: { ...DEFAULT_STYLE, fontSize: 9 },
      };
      break;
  }
  return { ...el, ...extra } as ReportElement;
}

export function createBand(report: ReportDefinition, type: BandType, group?: string): Band {
  const slug = type.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
  return { id: uniqueId(report, slug), type, height: type === 'detail' ? 8 : 12, group, elements: [] };
}

export function blankReport(title = 'Novo relatório'): ReportDefinition {
  return {
    $schema: 'https://novareport.io/schemas/v1/report.json',
    schemaVersion: '1.0',
    meta: { title, paperSize: 'A4', orientation: 'portrait', margins: { top: 10, bottom: 10, left: 15, right: 15 } },
    dataSources: [],
    parameters: [],
    groups: [],
    bands: [
      { id: 'report_header_1', type: 'reportHeader', height: 20, elements: [] },
      { id: 'page_header_1', type: 'pageHeader', height: 15, elements: [] },
      { id: 'detail_1', type: 'detail', height: 8, elements: [] },
      { id: 'page_footer_1', type: 'pageFooter', height: 10, elements: [] },
      { id: 'report_footer_1', type: 'reportFooter', height: 15, elements: [] },
    ],
  };
}

/** Clona elementos com novos ids e deslocamento (copiar/colar, duplicar). */
export function cloneElements(report: ReportDefinition, els: ReportElement[], offset = 2): ReportElement[] {
  const taken = new Set(report.bands.flatMap((b) => b.elements.map((e) => e.id)));
  return els.map((e) => {
    const prefix = e.id.replace(/_\d+$/, '');
    let n = 1;
    while (taken.has(`${prefix}_${n}`)) n++;
    const id = `${prefix}_${n}`;
    taken.add(id);
    return { ...structuredClone(e), id, x: e.x + offset, y: e.y + offset };
  });
}
