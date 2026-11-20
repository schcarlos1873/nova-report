// Arrastar e soltar do toolbox / painel de dados para o canvas.
// O payload também fica em memória durante o arraste: `dragover` não pode ler `dataTransfer`
// (necessário para o destaque do destino) e alguns WebViews descartam tipos MIME customizados.
import type { Band, ElementType, ReportDefinition, ReportElement, TableColumn } from '../../domain/types';
import { createElement } from '../../domain/factory';

export interface DropPayload {
  elementType?: ElementType;
  field?: string;
  /** Query de origem do campo e seu tipo (dtype do Polars). */
  query?: string;
  dataType?: string;
}

export const DND_MIME = 'application/x-novareport';

let current: DropPayload | undefined;

export function beginDrag(e: React.DragEvent, payload: DropPayload) {
  current = payload;
  e.dataTransfer.setData(DND_MIME, JSON.stringify(payload));
  e.dataTransfer.setData('text/plain', payload.field ? `Fields.${payload.field}` : payload.elementType ?? '');
  e.dataTransfer.effectAllowed = 'copy';
}

export function endDrag() {
  current = undefined;
}

export function draggedPayload(e?: React.DragEvent): DropPayload | undefined {
  if (current) return current;
  const raw = e?.dataTransfer.getData(DND_MIME);
  try {
    return raw ? (JSON.parse(raw) as DropPayload) : undefined;
  } catch {
    return undefined;
  }
}

// ---- semântica de soltar um campo

export const isNumeric = (t?: string) => !!t && /^(i|u|f)\d+$|^int|^float|^decimal/i.test(t);
export const isDate = (t?: string) => !!t && /^date/i.test(t);

/** `valor_total` → `Valor total`. */
export function humanize(field: string) {
  const s = field.replace(/[_\-.]+/g, ' ').replace(/([a-z])([A-Z])/g, '$1 $2').trim();
  return s.charAt(0).toUpperCase() + s.slice(1).toLowerCase();
}

function defaultFormat(dataType?: string) {
  if (isNumeric(dataType)) return /^(i|u)\d+$|^int/i.test(dataType!) ? 'number:0' : 'number:2';
  if (isDate(dataType)) return /time/i.test(dataType!) ? 'date:%d/%m/%Y %H:%M' : 'date:%d/%m/%Y';
  return undefined;
}

export interface FieldDrop {
  /** Elementos a inserir, por banda. */
  inserts: { bandId: string; element: ReportElement }[];
  /** Coluna a anexar numa tabela existente. */
  tableColumn?: { tableId: string; column: TableColumn };
  /** Query a associar à banda de detalhe (quando ainda não tem). */
  bindDetail?: { bandId: string; query: string };
  /** Descrição curta para o destaque durante o arraste. */
  hint: string;
}

/** Tabela sob o ponto (mm relativos à banda). */
export function tableAt(band: Band, x: number, y: number) {
  return [...band.elements]
    .reverse()
    .find((e): e is Extract<ReportElement, { type: 'table' }> => e.type === 'table' && x >= e.x && x <= e.x + e.width && y >= e.y && y <= e.y + e.height);
}

/**
 * Decide o que soltar um campo cria, conforme o destino:
 * - tabela: nova coluna (se a tabela usa a mesma query ou ainda não tem);
 * - cabeçalho de relatório/página: rótulo com o nome do campo (com Alt: também o valor no detalhe);
 * - cabeçalho de grupo: valor do campo em negrito;
 * - detalhe: valor do campo, formatado pelo tipo (com Alt: também o rótulo no cabeçalho de página);
 * - rodapé de grupo/relatório: `SUM` para numéricos, valor para os demais.
 */
export function resolveFieldDrop(
  report: ReportDefinition,
  band: Band,
  p: DropPayload & { field: string },
  x: number,
  y: number,
  widthMm: number,
  withCaption = false,
): FieldDrop {
  const { field, query, dataType } = p;
  const numeric = isNumeric(dataType);
  const caption = humanize(field);
  const table = tableAt(band, x, y);
  if (table) {
    const compatible = !table.dataSource || !query || table.dataSource === query;
    if (compatible) {
      return {
        inserts: [],
        tableColumn: {
          tableId: table.id,
          column: {
            header: caption,
            expression: `Fields.${field}`,
            width: numeric ? 25 : 35,
            align: numeric ? 'right' : undefined,
            format: defaultFormat(dataType),
            footer: numeric ? `SUM("${field}")` : undefined,
          },
        },
        hint: `Nova coluna “${caption}” em ${table.id}`,
      };
    }
  }

  const width = numeric ? 30 : 40;
  const at = { x: Math.max(0, Math.min(x, widthMm - width)), y };
  const style = createElement(report, 'label', 0, 0).style;
  const valueEl = (extra: Partial<ReportElement> = {}) =>
    createElement(report, 'expression', at.x, at.y, {
      width,
      expression: `Fields.${field}`,
      format: defaultFormat(dataType),
      style: { ...style, align: numeric ? 'right' : 'left' },
      ...extra,
    } as Partial<ReportElement>);
  const captionEl = (yy: number) =>
    createElement(report, 'label', at.x, yy, {
      width,
      text: caption,
      style: { ...style, bold: true, align: numeric ? 'right' : 'left' },
    } as Partial<ReportElement>);
  const pageHeader = report.bands.find((b) => b.type === 'pageHeader');
  const detail = report.bands.find((b) => b.type === 'detail');

  switch (band.type) {
    case 'reportHeader':
    case 'pageHeader': {
      const inserts = [{ bandId: band.id, element: captionEl(at.y) }];
      if (withCaption && detail) {
        inserts.push({ bandId: detail.id, element: valueEl({ y: 0 } as Partial<ReportElement>) });
      }
      return { inserts, hint: withCaption ? `Rótulo “${caption}” + valor no detalhe` : `Rótulo “${caption}”` };
    }
    case 'groupHeader': {
      const el = valueEl();
      el.style = { ...el.style, bold: true };
      return { inserts: [{ bandId: band.id, element: el }], hint: `Valor {${field}} do grupo` };
    }
    case 'detail': {
      const inserts = [{ bandId: band.id, element: valueEl() }];
      if (withCaption && pageHeader) {
        inserts.push({ bandId: pageHeader.id, element: captionEl(Math.max(0, pageHeader.height - 6)) });
      }
      const bindDetail = !band.dataSource && query ? { bandId: band.id, query } : undefined;
      return { inserts, bindDetail, hint: withCaption ? `Valor {${field}} + rótulo no cabeçalho de página` : `Valor {${field}}` };
    }
    case 'groupFooter':
    case 'reportFooter':
      if (numeric) {
        const el = valueEl({ expression: `SUM("${field}")`, format: defaultFormat('f64') } as Partial<ReportElement>);
        el.style = { ...el.style, bold: true };
        return { inserts: [{ bandId: band.id, element: el }], hint: `Total SUM("${field}")` };
      }
      return { inserts: [{ bandId: band.id, element: valueEl() }], hint: `Valor {${field}}` };
    default:
      return { inserts: [{ bandId: band.id, element: valueEl() }], hint: `Valor {${field}}` };
  }
}
