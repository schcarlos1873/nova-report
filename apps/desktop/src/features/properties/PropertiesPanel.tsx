import type { Draft } from 'immer';
import { IMAGE_REF_PREFIX, type Align, type Band, type PaperSize, type ReportElement, type Style, type TableColumn, type VerticalAlign } from '../../domain/types';
import { BAND_LABELS, pageSize, printableWidth } from '../../domain/geometry';
import { ColorInput, Field, NumberInput, Section, Select, TextInput, Toggle } from '../../components/ui';
import { draftElement, findElement, useReport } from '../../state/reportStore';
import { useUi } from '../../state/uiStore';
import { dialogs } from '../../infra/dialogs';
import { ExpressionField } from './ExpressionEditor';
import { FORMATS } from '../../domain/formats';
import { MIXED, SINGLE_BANDS, addBand, arrange, common, editSelection, hasText, setFormat, setStyle } from '../../state/commands';

const ALIGN: { value: Align; label: string }[] = [
  { value: 'left', label: 'Esquerda' },
  { value: 'center', label: 'Centro' },
  { value: 'right', label: 'Direita' },
  { value: 'justify', label: 'Justificado' },
];
const TYPE_NAMES: Record<ReportElement['type'], string> = {
  label: 'Texto',
  expression: 'Expressão',
  image: 'Imagem',
  line: 'Linha',
  rectangle: 'Retângulo',
  table: 'Tabela',
};
const VALIGN: { value: VerticalAlign; label: string }[] = [
  { value: 'top', label: 'Topo' },
  { value: 'middle', label: 'Meio' },
  { value: 'bottom', label: 'Base' },
];

export function PropertiesPanel() {
  const report = useReport((s) => s.report);
  const { selectedIds, selectedBandId } = useUi();

  if (selectedIds.length === 1) {
    const found = findElement(report, selectedIds[0]);
    if (found) return <ElementProps el={found.element} band={found.band} />;
  }
  if (selectedIds.length > 1) return <MultiProps ids={selectedIds} />;
  const band = report.bands.find((b) => b.id === selectedBandId);
  if (band) return <BandProps band={band} />;
  return <ReportProps />;
}

function useElementEditor(id: string) {
  const update = useReport((s) => s.update);
  return (field: string, recipe: (el: Draft<ReportElement>) => void) =>
    update((d) => {
      const el = draftElement(d, id);
      if (el) recipe(el);
    }, `${id}.${field}`);
}

function ElementProps({ el, band }: { el: ReportElement; band: Band }) {
  const edit = useElementEditor(el.id);
  const fonts = useUi((s) => s.info?.fonts ?? []);
  const report = useReport((s) => s.report);
  const queries = report.dataSources.flatMap((d) => d.queries.map((q) => q.name));
  const setStyle = <K extends keyof Style>(k: K, v: Style[K]) => edit(`style.${k}`, (e) => void (e.style[k] = v as never));
  const textual = hasText(el);

  return (
    <div className="props">
      <Section title={`${TYPE_NAMES[el.type]} · ${el.id}`}>
        <p className="muted small">Banda: {BAND_LABELS[band.type]}{band.group ? ` (${band.group})` : ''}</p>
        {el.type === 'label' && (
          <Field label="Texto">
            <TextInput value={el.text} onCommit={(v) => edit('text', (e) => void (e.type === 'label' && (e.text = v)))} />
          </Field>
        )}
        {el.type === 'expression' && (
          <>
            <Field label="Expressão">
              <ExpressionField value={el.expression} onCommit={(v) => edit('expr', (e) => void (e.type === 'expression' && (e.expression = v ?? '')))} />
            </Field>
            <Field label="Formato">
              <Select value={el.format ?? ''} options={FORMATS} onChange={(v) => edit('format', (e) => void (e.type === 'expression' && (e.format = v || undefined)))} />
            </Field>
          </>
        )}
        {el.type === 'image' && (
          <>
            <Field label="Imagem do relatório" hint="Recursos de Dados do Relatório › Imagens">
              <Select
                value={el.source.startsWith(IMAGE_REF_PREFIX) ? el.source.slice(IMAGE_REF_PREFIX.length) : ''}
                options={[{ value: '', label: '(origem direta)' }, ...(useReport.getState().report.images ?? []).map((i) => ({ value: i.name, label: i.name }))]}
                onChange={(v) => edit('src', (e) => void (e.type === 'image' && (e.source = v ? IMAGE_REF_PREFIX + v : '')))}
              />
            </Field>
            <Field label="Origem" hint="image:nome, caminho relativo ao .nrep, absoluto ou data: URI">
              <span className="row">
                <TextInput value={el.source} mono onCommit={(v) => edit('src', (e) => void (e.type === 'image' && (e.source = v)))} />
                <button
                  onClick={async () => {
                    const p = await dialogs.pickFile(['png', 'jpg', 'jpeg', 'svg', 'gif']);
                    if (p) edit('src', (e) => void (e.type === 'image' && (e.source = p)));
                  }}
                >
                  …
                </button>
              </span>
            </Field>
            <Field label="Ajuste">
              <Select value={el.fit ?? 'contain'} options={['contain', 'cover', 'stretch'] as const} onChange={(v) => edit('fit', (e) => void (e.type === 'image' && (e.fit = v)))} />
            </Field>
          </>
        )}
        {el.type === 'line' && (
          <Field label="Direção">
            <Select
              value={el.direction ?? 'horizontal'}
              options={[{ value: 'horizontal', label: 'Horizontal' }, { value: 'vertical', label: 'Vertical' }, { value: 'diagonal', label: 'Diagonal' }]}
              onChange={(v) => edit('dir', (e) => {
                if (e.type !== 'line') return;
                e.direction = v;
                if (v === 'horizontal') e.height = 0;
                if (v === 'vertical') { e.height = Math.max(e.height, 10); e.width = 0; }
              })}
            />
          </Field>
        )}
        {el.type === 'rectangle' && (
          <Field label="Raio (mm)">
            <NumberInput value={el.radius ?? 0} min={0} onCommit={(v) => edit('radius', (e) => void (e.type === 'rectangle' && (e.radius = v)))} />
          </Field>
        )}
        {el.type === 'table' && <TableProps el={el} queries={queries} edit={edit} />}
      </Section>

      <Section title="Posição e tamanho (mm)">
        <div className="grid2">
          <Field label="X"><NumberInput value={el.x} min={0} onCommit={(v) => edit('x', (e) => void (e.x = v))} /></Field>
          <Field label="Y"><NumberInput value={el.y} min={0} onCommit={(v) => edit('y', (e) => void (e.y = v))} /></Field>
          <Field label="Largura"><NumberInput value={el.width} min={0} onCommit={(v) => edit('w', (e) => void (e.width = v))} /></Field>
          <Field label="Altura"><NumberInput value={el.height} min={0} onCommit={(v) => edit('h', (e) => void (e.height = v))} /></Field>
        </div>
      </Section>

      {textual && (
        <Section title="Texto">
          <Field label="Fonte">
            <input list="font-list" value={el.style.fontFamily} onChange={(e) => setStyle('fontFamily', e.target.value)} />
            <datalist id="font-list">{fonts.map((f) => <option key={f} value={f} />)}</datalist>
          </Field>
          <div className="grid2">
            <Field label="Tamanho (pt)"><NumberInput value={el.style.fontSize} step={0.5} min={4} onCommit={(v) => setStyle('fontSize', v)} /></Field>
            <Field label="Cor"><ColorInput value={el.style.color} onCommit={(v) => setStyle('color', v ?? '#000000')} /></Field>
          </div>
          <div className="row gap">
            <Toggle label="Negrito" checked={el.style.bold} onChange={(v) => setStyle('bold', v)} />
            <Toggle label="Itálico" checked={el.style.italic} onChange={(v) => setStyle('italic', v)} />
            <Toggle label="Sublinhado" checked={el.style.underline} onChange={(v) => setStyle('underline', v)} />
          </div>
          <div className="grid2">
            <Field label="Alinhamento"><Select value={el.style.align} options={ALIGN} onChange={(v) => setStyle('align', v)} /></Field>
            <Field label="Vertical"><Select value={el.style.verticalAlign} options={VALIGN} onChange={(v) => setStyle('verticalAlign', v)} /></Field>
          </div>
          {el.type !== 'table' && (
            <Toggle label="Pode crescer (quebra de linha)" checked={!!el.canGrow} onChange={(v) => edit('grow', (e) => void (e.canGrow = v || undefined))} />
          )}
        </Section>
      )}

      <Section title="Aparência">
        <Field label="Fundo"><ColorInput value={el.style.background} allowEmpty onCommit={(v) => setStyle('background', v)} /></Field>
        <div className="grid2">
          <Field label="Borda (pt)"><NumberInput value={el.style.borderWidth} step={0.25} min={0} onCommit={(v) => setStyle('borderWidth', v)} /></Field>
          <Field label="Espaçamento (mm)"><NumberInput value={el.style.padding} step={0.5} min={0} onCommit={(v) => setStyle('padding', v)} /></Field>
        </div>
        <Field label="Cor da borda"><ColorInput value={el.style.borderColor} onCommit={(v) => setStyle('borderColor', v ?? '#000000')} /></Field>
      </Section>

      <Section title="Comportamento condicional">
        <Field label="Visível quando" hint="Expressão booleana, ex.: Fields.valor > 0">
          <ExpressionField value={el.visible} placeholder="sempre" onCommit={(v) => edit('visible', (e) => void (e.visible = v))} />
        </Field>
        <Field label="Cor do texto (expr.)" hint='Ex.: IIF(Fields.valor < 0, "#DC2626", "#000000")'>
          <ExpressionField value={el.colorExpression} placeholder="fixa" onCommit={(v) => edit('colorExpr', (e) => void (e.colorExpression = v))} />
        </Field>
        <Field label="Cor de fundo (expr.)">
          <ExpressionField value={el.backgroundExpression} placeholder="fixa" onCommit={(v) => edit('bgExpr', (e) => void (e.backgroundExpression = v))} />
        </Field>
      </Section>
    </div>
  );
}

function TableProps({
  el,
  queries,
  edit,
}: {
  el: Extract<ReportElement, { type: 'table' }>;
  queries: string[];
  edit: (field: string, recipe: (el: Draft<ReportElement>) => void) => void;
}) {
  const editCol = (i: number, field: string, recipe: (c: Draft<TableColumn>) => void) =>
    edit(`col${i}.${field}`, (e) => {
      if (e.type === 'table' && e.columns[i]) recipe(e.columns[i]);
    });
  const total = el.columns.reduce((s, c) => s + c.width, 0);
  return (
    <>
      <Field label="Dados (query)">
        <Select value={el.dataSource || undefined} options={queries} onChange={(v) => edit('ds', (e) => void (e.type === 'table' && (e.dataSource = v)))} />
      </Field>
      <div className="row gap">
        <Toggle label="Repetir cabeçalho" checked={el.repeatHeader !== false} onChange={(v) => edit('rep', (e) => void (e.type === 'table' && (e.repeatHeader = v)))} />
      </div>
      <div className="grid2">
        <Field label="Fundo cabeçalho"><ColorInput value={el.headerBackground} onCommit={(v) => edit('hbg', (e) => void (e.type === 'table' && (e.headerBackground = v)))} /></Field>
        <Field label="Zebra"><ColorInput value={el.zebraBackground} allowEmpty onCommit={(v) => edit('zebra', (e) => void (e.type === 'table' && (e.zebraBackground = v)))} /></Field>
      </div>
      <h4 className="sub">Colunas <small className="muted">({total.toFixed(1)} mm)</small></h4>
      {el.columns.map((c, i) => (
        <div key={i} className="table-col">
          <div className="row">
            <TextInput value={c.header} onCommit={(v) => editCol(i, 'h', (x) => void (x.header = v))} />
            <NumberInput value={c.width} min={1} onCommit={(v) => editCol(i, 'w', (x) => void (x.width = v))} />
            <button className="icon-btn" title="Remover coluna" onClick={() => edit('cols', (e) => void (e.type === 'table' && e.columns.splice(i, 1)))}>×</button>
          </div>
          <ExpressionField value={c.expression} onCommit={(v) => editCol(i, 'e', (x) => void (x.expression = v ?? ''))} />
          <div className="row">
            <Select value={c.align ?? 'left'} options={ALIGN} onChange={(v) => editCol(i, 'a', (x) => void (x.align = v))} />
            <Select value={c.format ?? ''} options={FORMATS} onChange={(v) => editCol(i, 'f', (x) => void (x.format = v || undefined))} />
          </div>
          <ExpressionField value={c.footer} placeholder="rodapé (ex.: SUM(&quot;valor&quot;))" onCommit={(v) => editCol(i, 'ft', (x) => void (x.footer = v))} />
        </div>
      ))}
      <button
        onClick={() =>
          edit('cols', (e) => {
            if (e.type === 'table') e.columns.push({ header: `Coluna ${e.columns.length + 1}`, expression: 'Fields.campo', width: 30 });
          })
        }
      >
        + Coluna
      </button>
    </>
  );
}

function MultiProps({ ids }: { ids: string[] }) {
  const report = useReport((s) => s.report);
  const fonts = useUi((s) => s.info?.fonts ?? []);
  const els = ids.map((id) => findElement(report, id)?.element).filter((e): e is ReportElement => !!e);
  const textEls = els.filter(hasText);
  const allText = textEls.length === els.length;
  const exprEls = els.filter((e) => e.type === 'expression');
  const bands = new Set(ids.map((id) => findElement(report, id)?.band.id));
  const counts = Object.entries(
    els.reduce<Record<string, number>>((acc, e) => ({ ...acc, [TYPE_NAMES[e.type]]: (acc[TYPE_NAMES[e.type]] ?? 0) + 1 }), {}),
  ).map(([t, n]) => `${t}: ${n}`);

  /** Valor comum ou estado misto de uma propriedade sobre um conjunto. */
  const val = <T,>(set: ReportElement[], get: (e: ReportElement) => T) => {
    const v = common(set, get);
    return { value: v === MIXED || v === undefined ? undefined : v, mixed: v === MIXED };
  };
  const geom = (k: 'x' | 'y' | 'width' | 'height') => val(els, (e) => e[k]);
  const st = <K extends keyof Style>(k: K, set = els) => val(set, (e) => e.style[k]);
  const setGeom = (k: 'x' | 'y' | 'width' | 'height', v: number) =>
    editSelection(k, (e) => {
      e[k] = v;
      if (e.type === 'line' && k === 'height' && e.direction !== 'vertical' && e.direction !== 'diagonal') e.height = 0;
    });

  const font = st('fontFamily', textEls);
  const size = st('fontSize', textEls);
  const color = st('color', textEls);
  const bold = st('bold', textEls);
  const italic = st('italic', textEls);
  const underline = st('underline', textEls);
  const align = st('align', textEls);
  const valign = st('verticalAlign', textEls);
  const bg = st('background');
  const bw = st('borderWidth');
  const bc = st('borderColor');
  const pad = st('padding');
  const fmt = val(exprEls, (e) => (e.type === 'expression' ? e.format ?? '' : ''));

  return (
    <div className="props">
      <Section title={`${ids.length} elementos selecionados`}>
        <p className="muted small">
          {counts.join(', ')} · {bands.size > 1 ? `${bands.size} bandas` : '1 banda'}. Campos com “—” têm valores diferentes; só o que você alterar é aplicado a todos.
        </p>
        <div className="btn-grid">
          <button onClick={() => arrange('left')}>⇤ Esquerda</button>
          <button onClick={() => arrange('right')}>Direita ⇥</button>
          <button onClick={() => arrange('top')}>⤒ Topo</button>
          <button onClick={() => arrange('bottom')}>Base ⤓</button>
          <button onClick={() => arrange('sameWidth')}>Mesma largura</button>
          <button onClick={() => arrange('sameHeight')}>Mesma altura</button>
          <button disabled={els.length < 3} title={els.length < 3 ? 'Selecione 3 ou mais elementos' : undefined} onClick={() => arrange('distributeH')}>⇹ Distribuir</button>
          <button disabled={els.length < 3} title={els.length < 3 ? 'Selecione 3 ou mais elementos' : undefined} onClick={() => arrange('distributeV')}>↕ Distribuir vert.</button>
        </div>
      </Section>

      <Section title="Posição e tamanho (mm)">
        <div className="grid2">
          {(['x', 'y', 'width', 'height'] as const).map((k) => {
            const g = geom(k);
            return (
              <Field key={k} label={{ x: 'X', y: 'Y', width: 'Largura', height: 'Altura' }[k]}>
                <NumberInput value={g.value ?? 0} mixed={g.mixed} min={0} onCommit={(v) => setGeom(k, v)} />
              </Field>
            );
          })}
        </div>
        {bands.size > 1 && <p className="muted small">Y é relativo ao topo de cada banda.</p>}
      </Section>

      {textEls.length > 0 && (
        <Section title={allText ? 'Texto' : `Texto (${textEls.length} de ${els.length})`}>
          <Field label="Fonte">
            <input
              list="font-list-multi"
              defaultValue={font.value ?? ''}
              key={font.value ?? 'mixed'}
              placeholder={font.mixed ? '— (várias)' : ''}
              onBlur={(e) => e.target.value && e.target.value !== font.value && setStyle('fontFamily', e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && (e.target as HTMLInputElement).blur()}
            />
            <datalist id="font-list-multi">{fonts.map((f) => <option key={f} value={f} />)}</datalist>
          </Field>
          <div className="grid2">
            <Field label="Tamanho (pt)"><NumberInput value={size.value ?? 10} mixed={size.mixed} step={0.5} min={4} onCommit={(v) => setStyle('fontSize', v)} /></Field>
            <Field label="Cor"><ColorInput value={color.value} mixed={color.mixed} onCommit={(v) => setStyle('color', v ?? '#000000')} /></Field>
          </div>
          <div className="row gap">
            <Toggle label="Negrito" checked={!!bold.value} mixed={bold.mixed} onChange={(v) => setStyle('bold', v)} />
            <Toggle label="Itálico" checked={!!italic.value} mixed={italic.mixed} onChange={(v) => setStyle('italic', v)} />
            <Toggle label="Sublinhado" checked={!!underline.value} mixed={underline.mixed} onChange={(v) => setStyle('underline', v)} />
          </div>
          <div className="grid2">
            <Field label="Alinhamento"><Select value={align.value} options={ALIGN} onChange={(v) => setStyle('align', v)} /></Field>
            <Field label="Vertical"><Select value={valign.value} options={VALIGN} onChange={(v) => setStyle('verticalAlign', v)} /></Field>
          </div>
          {exprEls.length > 0 && (
            <Field label={exprEls.length === els.length ? 'Formato' : `Formato (${exprEls.length} campo(s))`}>
              <Select value={fmt.mixed ? undefined : fmt.value ?? ''} options={FORMATS} onChange={(v) => setFormat(v)} />
            </Field>
          )}
        </Section>
      )}

      <Section title="Aparência">
        <Field label="Fundo"><ColorInput value={bg.value} mixed={bg.mixed} allowEmpty onCommit={(v) => setStyle('background', v)} /></Field>
        <div className="grid2">
          <Field label="Borda (pt)"><NumberInput value={bw.value ?? 0} mixed={bw.mixed} step={0.25} min={0} onCommit={(v) => setStyle('borderWidth', v)} /></Field>
          <Field label="Espaçamento (mm)"><NumberInput value={pad.value ?? 0} mixed={pad.mixed} step={0.5} min={0} onCommit={(v) => setStyle('padding', v)} /></Field>
        </div>
        <Field label="Cor da borda"><ColorInput value={bc.value} mixed={bc.mixed} onCommit={(v) => setStyle('borderColor', v ?? '#000000')} /></Field>
      </Section>
    </div>
  );
}

function BandProps({ band }: { band: Band }) {
  const update = useReport((s) => s.update);
  const report = useReport((s) => s.report);
  const queries = report.dataSources.flatMap((d) => d.queries.map((q) => q.name));
  const edit = (field: string, recipe: (b: Draft<Band>) => void) =>
    update((d) => {
      const b = d.bands.find((x) => x.id === band.id);
      if (b) recipe(b);
    }, `${band.id}.${field}`);
  return (
    <div className="props">
      <Section title={`Banda · ${BAND_LABELS[band.type]}`}>
        <Field label="Altura (mm)"><NumberInput value={band.height} min={1} onCommit={(v) => edit('h', (b) => void (b.height = v))} /></Field>
        {band.type === 'detail' && (
          <Field label="Dados (query)">
            <Select value={band.dataSource} options={queries} onChange={(v) => edit('ds', (b) => void (b.dataSource = v))} />
          </Field>
        )}
        {(band.type === 'groupHeader' || band.type === 'groupFooter') && (
          <Field label="Grupo">
            <Select value={band.group} options={(report.groups ?? []).map((g) => g.name)} onChange={(v) => edit('g', (b) => void (b.group = v))} />
          </Field>
        )}
        <Toggle label="Manter junto" checked={!!band.keepTogether} onChange={(v) => edit('kt', (b) => void (b.keepTogether = v || undefined))} />
        <Toggle label="Quebra de página após" checked={!!band.pageBreakAfter} onChange={(v) => edit('pb', (b) => void (b.pageBreakAfter = v || undefined))} />
        <Field label="Visível quando">
          <ExpressionField value={band.visible} placeholder="sempre" onCommit={(v) => edit('vis', (b) => void (b.visible = v))} />
        </Field>
        <p className="muted small">{band.elements.length} elemento(s)</p>
        <button
          className="danger"
          onClick={async () => {
            if (band.elements.length && !(await dialogs.confirm(`Excluir a banda e seus ${band.elements.length} elementos?`))) return;
            update((d) => void (d.bands = d.bands.filter((b) => b.id !== band.id)));
            useUi.getState().select([]);
          }}
        >
          Excluir banda
        </button>
      </Section>
    </div>
  );
}

function ReportProps() {
  const update = useReport((s) => s.update);
  const report = useReport((s) => s.report);
  const m = report.meta;
  const edit = (field: string, recipe: (d: Draft<typeof report>) => void) => update(recipe, `meta.${field}`);
  const size = pageSize(report);
  const existing = new Set(report.bands.map((b) => b.type));
  return (
    <div className="props">
      <Section title="Relatório">
        <Field label="Título"><TextInput value={m.title} onCommit={(v) => edit('title', (d) => void (d.meta.title = v))} /></Field>
        <Field label="Descrição"><TextInput value={m.description ?? ''} onCommit={(v) => edit('desc', (d) => void (d.meta.description = v || undefined))} /></Field>
        <Field label="Autor"><TextInput value={m.author ?? ''} onCommit={(v) => edit('author', (d) => void (d.meta.author = v || undefined))} /></Field>
      </Section>
      <Section title="Página">
        <div className="grid2">
          <Field label="Papel">
            <Select<PaperSize> value={m.paperSize} options={['A4', 'A3', 'A5', 'Letter', 'Legal', 'Custom']}
              onChange={(v) => edit('paper', (d) => { d.meta.paperSize = v; if (v === 'Custom' && !d.meta.customSize) d.meta.customSize = { width: 210, height: 297 }; })} />
          </Field>
          <Field label="Orientação">
            <Select value={m.orientation} options={[{ value: 'portrait', label: 'Retrato' }, { value: 'landscape', label: 'Paisagem' }]}
              onChange={(v) => edit('orient', (d) => void (d.meta.orientation = v))} />
          </Field>
        </div>
        {m.paperSize === 'Custom' && (
          <div className="grid2">
            <Field label="Largura (mm)"><NumberInput value={m.customSize?.width ?? 210} min={20} onCommit={(v) => edit('cw', (d) => void (d.meta.customSize = { width: v, height: d.meta.customSize?.height ?? 297 }))} /></Field>
            <Field label="Altura (mm)"><NumberInput value={m.customSize?.height ?? 297} min={20} onCommit={(v) => edit('ch', (d) => void (d.meta.customSize = { width: d.meta.customSize?.width ?? 210, height: v }))} /></Field>
          </div>
        )}
        <div className="grid2">
          {(['top', 'bottom', 'left', 'right'] as const).map((k) => (
            <Field key={k} label={{ top: 'Margem sup.', bottom: 'Margem inf.', left: 'Margem esq.', right: 'Margem dir.' }[k]}>
              <NumberInput value={m.margins[k]} min={0} onCommit={(v) => edit(`m.${k}`, (d) => void (d.meta.margins[k] = v))} />
            </Field>
          ))}
        </div>
        <p className="muted small">
          {size.width} × {size.height} mm · área útil {printableWidth(report).toFixed(1)} mm
        </p>
      </Section>
      <Section title="Bandas">
        <div className="btn-grid">
          {SINGLE_BANDS.map((t) => (
            <button key={t} disabled={existing.has(t)} title={existing.has(t) ? 'O relatório já tem esta banda' : undefined} onClick={() => addBand(t)}>
              + {BAND_LABELS[t]}
            </button>
          ))}
        </div>
        <p className="muted small">Bandas de grupo são criadas em Dados do Relatório › Grupos.</p>
      </Section>
    </div>
  );
}
