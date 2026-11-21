// Ribbon: comandos agrupados por tarefa. Todo comando vem de `state/commands` (o mesmo usado
// pelos atalhos), reflete a seleção atual e explica no tooltip por que está indisponível.
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import type { Align, BandType, ElementType, PaperSize, VerticalAlign } from '../../domain/types';
import { BAND_LABELS } from '../../domain/geometry';
import { FORMATS } from '../../domain/formats';
import { findElement, useReport } from '../../state/reportStore';
import { useUi, type RibbonTab } from '../../state/uiStore';
import * as cmd from '../../state/commands';
import { MIXED, common, hasText, type Mixed } from '../../state/commands';
import { beginDrag, endDrag, type DropPayload } from '../designer/dnd';

const TABS: { id: RibbonTab; label: string }[] = [
  { id: 'home', label: 'Página Inicial' },
  { id: 'insert', label: 'Inserir' },
  { id: 'data', label: 'Dados' },
  { id: 'layout', label: 'Layout' },
  { id: 'view', label: 'Exibir' },
];

const WHY = {
  design: 'Disponível no modo Design (⌘P alterna)',
  noSel: 'Selecione um ou mais elementos',
  noText: 'Selecione um texto, campo ou tabela',
  noExpr: 'Selecione um ou mais campos/expressões',
  two: 'Selecione 2 ou mais elementos',
  three: 'Selecione 3 ou mais elementos',
  clipboard: 'A área de transferência está vazia',
};

const ZOOMS = [0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4];
const FONT_SIZES = [6, 7, 8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 36];
const BORDERS = [0, 0.25, 0.5, 0.75, 1, 1.5, 2, 3];

// ---- primitivas

interface BtnProps {
  icon: ReactNode;
  label: string;
  onClick: () => void;
  /** Motivo da indisponibilidade; `undefined` = habilitado. */
  disabled?: string | false;
  shortcut?: string;
  pressed?: Mixed<boolean>;
  large?: boolean;
  /** Mostra só o ícone (o rótulo continua acessível e no tooltip). */
  compact?: boolean;
  /** Torna o botão arrastável (ex.: elementos de Inserir soltos no canvas). */
  drag?: DropPayload;
}

function Btn({ icon, label, onClick, disabled, shortcut, pressed, large, compact, drag }: BtnProps) {
  const tip = [label, shortcut && `(${shortcut})`, drag && !disabled && '— clique ou arraste para o relatório', disabled && `— ${disabled}`].filter(Boolean).join(' ');
  const draggable = !!drag && !disabled;
  return (
    <button
      type="button"
      className={`rb-btn${large ? ' large' : ''}${compact ? ' compact' : ''}${pressed === true ? ' on' : ''}${pressed === MIXED ? ' mixed' : ''}`}
      onClick={onClick}
      disabled={!!disabled}
      title={tip}
      aria-label={compact ? label : undefined}
      aria-pressed={pressed === undefined ? undefined : pressed === MIXED ? 'mixed' : pressed}
      draggable={draggable || undefined}
      onDragStart={draggable ? (e) => beginDrag(e, drag) : undefined}
      onDragEnd={draggable ? endDrag : undefined}
    >
      <span className="rb-icon" aria-hidden>{icon}</span>
      {!compact && <span className="rb-label">{label}</span>}
    </button>
  );
}

function Group({ label, children, wide, info }: { label: string; children: ReactNode; wide?: boolean; info?: string }) {
  return (
    <div className={`rb-group${wide ? ' wide' : ''}`} role="group" aria-label={label}>
      {info && <InfoTip text={info} />}
      <div className="rb-group-body">{children}</div>
      <div className="rb-group-label">{label}</div>
    </div>
  );
}

/** Ícone (i) com a explicação em tooltip (também ao focar pelo teclado). */
function InfoTip({ text }: { text: string }) {
  return (
    <span className="rb-info" tabIndex={0} role="img" aria-label={text} title={text}>
      i
    </span>
  );
}

function Stack({ children }: { children: ReactNode }) {
  return <div className="rb-stack">{children}</div>;
}

function Row({ children }: { children: ReactNode }) {
  return <div className="rb-row">{children}</div>;
}

/** Select com estado misto: mostra "—" e só aplica quando o usuário escolhe um valor. */
function MixedSelect<T extends string | number>({
  value,
  options,
  onChange,
  disabled,
  label,
  width,
}: {
  value: Mixed<T>;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
  disabled?: string | false;
  label: string;
  width?: number;
}) {
  const known = value !== undefined && value !== MIXED && options.some((o) => o.value === value);
  const current = value === undefined || value === MIXED ? '' : String(value);
  return (
    <select
      className="rb-select"
      style={width ? { width } : undefined}
      aria-label={label}
      title={disabled ? `${label} — ${disabled}` : value === MIXED ? `${label}: valores diferentes na seleção` : label}
      disabled={!!disabled}
      value={current}
      onChange={(e) => {
        const o = options.find((x) => String(x.value) === e.target.value);
        if (o) onChange(o.value);
      }}
    >
      {(value === MIXED || value === undefined) && <option value="">{value === MIXED ? '—' : ''}</option>}
      {!known && current !== '' && <option value={current}>{current}</option>}
      {options.map((o) => <option key={String(o.value)} value={String(o.value)}>{o.label}</option>)}
    </select>
  );
}

function ColorBtn({ value, onChange, disabled, label, icon, allowEmpty }: {
  value: Mixed<string>;
  onChange: (v?: string) => void;
  disabled?: string | false;
  label: string;
  icon: ReactNode;
  allowEmpty?: boolean;
}) {
  const hex = typeof value === 'string' ? value : '#000000';
  const tip = disabled ? `${label} — ${disabled}` : value === MIXED ? `${label}: valores diferentes` : `${label}: ${value ?? 'nenhum'}`;
  return (
    <span className={`rb-color${disabled ? ' disabled' : ''}`} title={tip}>
      <label>
        <span className="rb-icon" aria-hidden>{icon}</span>
        <span className={`rb-swatch${value === MIXED ? ' mixed' : ''}${!value ? ' empty' : ''}`} style={typeof value === 'string' ? { background: value } : undefined} />
        <input type="color" aria-label={label} disabled={!!disabled} value={hex} onChange={(e) => onChange(e.target.value.toUpperCase())} />
      </label>
      {allowEmpty && (
        <button type="button" className="rb-clear" disabled={!!disabled || !value} aria-label={`Remover ${label.toLowerCase()}`} title={`Remover ${label.toLowerCase()}`} onClick={() => onChange(undefined)}>
          ⌀
        </button>
      )}
    </span>
  );
}

function NumberBox({ value, onCommit, label, min = 0, step = 1, disabled }: {
  value: number;
  onCommit: (v: number) => void;
  label: string;
  min?: number;
  step?: number;
  disabled?: string | false;
}) {
  const [v, setV] = useState(String(value));
  useEffect(() => setV(String(value)), [value]);
  const commit = () => {
    const n = Number(v.replace(',', '.'));
    if (Number.isFinite(n) && n >= min && n !== value) onCommit(n);
    else setV(String(value));
  };
  return (
    <label className="rb-num" title={disabled ? `${label} — ${disabled}` : label}>
      <span>{label}</span>
      <input type="number" value={v} min={min} step={step} disabled={!!disabled} onChange={(e) => setV(e.target.value)} onBlur={commit}
        onKeyDown={(e) => e.key === 'Enter' && (e.target as HTMLInputElement).blur()} />
    </label>
  );
}

// ---- menu Arquivo

function FileMenu({ onClose }: { onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const recent = cmd.recentFiles();
  useEffect(() => {
    ref.current?.querySelector('button')?.focus();
    const onDown = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && onClose();
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    window.addEventListener('mousedown', onDown);
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('mousedown', onDown);
      window.removeEventListener('keydown', onKey);
    };
  }, [onClose]);
  const run = (f: () => unknown) => () => {
    onClose();
    f();
  };
  return (
    <div className="file-menu" ref={ref} role="menu" aria-label="Arquivo">
      <div className="file-actions">
        <button role="menuitem" onClick={run(cmd.newReport)}><span aria-hidden>🗋</span> Novo <kbd>⌘N</kbd></button>
        <button role="menuitem" onClick={run(() => cmd.openReport())}><span aria-hidden>📂</span> Abrir… <kbd>⌘O</kbd></button>
        <hr />
        <button role="menuitem" onClick={run(() => cmd.save(false))}><span aria-hidden>💾</span> Salvar <kbd>⌘S</kbd></button>
        <button role="menuitem" onClick={run(() => cmd.save(true))}><span aria-hidden>🖫</span> Salvar como… <kbd>⇧⌘S</kbd></button>
        <hr />
        <button role="menuitem" onClick={run(() => useUi.getState().set({ mode: 'preview' }))}><span aria-hidden>⇪</span> Exportar (PDF, Excel, CSV)…</button>
      </div>
      <div className="file-recent">
        <h4>Documentos recentes</h4>
        {recent.length === 0 && <p className="muted small">Nenhum documento aberto ainda.</p>}
        {recent.map((p) => (
          <button key={p} role="menuitem" className="recent" title={p} onClick={run(() => cmd.openReport(p))}>
            <strong>{p.split(/[\\/]/).pop()}</strong>
            <span className="muted small">{p.replace(/[\\/][^\\/]*$/, '')}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

// ---- Ribbon

export function Ribbon() {
  const report = useReport((s) => s.report);
  const filePath = useReport((s) => s.filePath);
  const dirty = useReport((s) => s.dirty);
  const canUndo = useReport((s) => s.past.length > 0);
  const canRedo = useReport((s) => s.future.length > 0);
  const ui = useUi();
  const [fileOpen, setFileOpen] = useState(false);
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const els = useMemo(
    () => ui.selectedIds.map((id) => findElement(report, id)?.element).filter((e) => !!e),
    [report, ui.selectedIds],
  );
  const textEls = els.filter(hasText);
  const exprEls = els.filter((e) => e.type === 'expression');
  const design = ui.mode === 'design';
  const off = (cond: boolean, why: string) => (!design ? WHY.design : cond ? false : why);
  const sel = off(els.length > 0, WHY.noSel);
  const txt = off(textEls.length > 0, textEls.length || els.length ? WHY.noText : WHY.noSel);
  const two = off(els.length >= 2, WHY.two);
  const s = <T,>(get: (st: (typeof els)[number]['style']) => T) => common(textEls, (e) => get(e.style));
  const sAll = <T,>(get: (st: (typeof els)[number]['style']) => T) => common(els, (e) => get(e.style));
  const fonts = ui.info?.fonts ?? [];
  const fontFamily = s((x) => x.fontFamily);
  const [fontDraft, setFontDraft] = useState('');
  useEffect(() => setFontDraft(typeof fontFamily === 'string' ? fontFamily : ''), [fontFamily]);

  const bandTypes = new Set(report.bands.map((b) => b.type));
  const selectedBand = report.bands.find((b) => b.id === ui.selectedBandId);
  const m = report.meta;
  const fileName = filePath?.split(/[\\/]/).pop() ?? 'Sem título';
  const errors = ui.issues.filter((i) => i.severity === 'error').length;

  const onTabKey = (e: React.KeyboardEvent, i: number) => {
    const d = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
    if (!d) return;
    e.preventDefault();
    const n = (i + d + TABS.length) % TABS.length;
    ui.set({ ribbonTab: TABS[n].id });
    tabRefs.current[n]?.focus();
  };

  const insertTools: { type: ElementType; icon: string; label: string }[] = [
    { type: 'label', icon: 'T', label: 'Texto' },
    { type: 'expression', icon: 'ƒx', label: 'Campo' },
    { type: 'table', icon: '▦', label: 'Tabela' },
    { type: 'image', icon: '🖼', label: 'Imagem' },
    { type: 'line', icon: '╱', label: 'Linha' },
    { type: 'rectangle', icon: '▭', label: 'Retângulo' },
  ];
  const bandBtn = (t: BandType, icon: string) => (
    <Btn key={t} icon={icon} label={BAND_LABELS[t]} onClick={() => cmd.addBand(t)}
      disabled={off(!bandTypes.has(t), 'O relatório já tem esta banda')} />
  );

  return (
    <header className={`ribbon${ui.ribbonCollapsed ? ' collapsed' : ''}`}>
      <div className="rb-top">
        <span className="logo" aria-hidden>N</span>
        <div className="rb-quick" role="toolbar" aria-label="Acesso rápido">
          <Btn compact icon="💾" label="Salvar" shortcut="⌘S" onClick={() => cmd.save(false)} />
          <Btn compact icon="↶" label="Desfazer" shortcut="⌘Z" onClick={useReport.getState().undo} disabled={!canUndo && 'Nada para desfazer'} />
          <Btn compact icon="↷" label="Refazer" shortcut="⇧⌘Z" onClick={useReport.getState().redo} disabled={!canRedo && 'Nada para refazer'} />
        </div>
        <nav className="rb-tabs" role="tablist" aria-label="Ribbon">
          <button
            type="button"
            className={`rb-tab file${fileOpen ? ' active' : ''}`}
            aria-haspopup="menu"
            aria-expanded={fileOpen}
            onClick={() => setFileOpen((v) => !v)}
          >
            Arquivo
          </button>
          {TABS.map((t, i) => (
            <button
              key={t.id}
              ref={(el) => void (tabRefs.current[i] = el)}
              type="button"
              role="tab"
              aria-selected={ui.ribbonTab === t.id}
              tabIndex={ui.ribbonTab === t.id ? 0 : -1}
              className={`rb-tab${ui.ribbonTab === t.id ? ' active' : ''}`}
              onClick={() => ui.set({ ribbonTab: t.id, ribbonCollapsed: false })}
              onDoubleClick={() => ui.set({ ribbonCollapsed: !ui.ribbonCollapsed })}
              onKeyDown={(e) => onTabKey(e, i)}
            >
              {t.label}
            </button>
          ))}
        </nav>
        <span className="rb-doc" title={filePath ?? 'Documento ainda não salvo'}>
          {fileName}
          {dirty && <span className="dirty"> · não salvo</span>}
        </span>
        <div className="mode-switch" role="group" aria-label="Modo">
          <button type="button" className={design ? 'active' : ''} aria-pressed={design} onClick={() => ui.set({ mode: 'design' })}>Design</button>
          <button type="button" className={!design ? 'active' : ''} aria-pressed={!design} onClick={() => ui.set({ mode: 'preview' })} title="Pré-visualizar (⌘P)">Pré-visualizar</button>
        </div>
        <Btn compact icon={ui.ribbonCollapsed ? '⌄' : '⌃'} label={ui.ribbonCollapsed ? 'Expandir Ribbon' : 'Recolher Ribbon'} shortcut="⌃F1"
          onClick={() => ui.set({ ribbonCollapsed: !ui.ribbonCollapsed })} />
        {fileOpen && <FileMenu onClose={() => setFileOpen(false)} />}
      </div>

      {!ui.ribbonCollapsed && (
        <div className="rb-panel" role="tabpanel" aria-label={TABS.find((t) => t.id === ui.ribbonTab)?.label}>
          {ui.ribbonTab === 'home' && (
            <>
              <Group label="Exibições">
                <Btn large icon={design ? '▶' : '✎'} label={design ? 'Executar' : 'Design'} shortcut="⌘P" onClick={cmd.toggleMode} />
              </Group>
              <Group label="Área de Transferência">
                <Btn large icon="📋" label="Colar" shortcut="⌘V" onClick={cmd.paste} disabled={off(ui.clipboard.length > 0, WHY.clipboard)} />
                <Stack>
                  <Btn icon="✂" label="Recortar" shortcut="⌘X" onClick={cmd.cutSelection} disabled={sel} />
                  <Btn icon="⧉" label="Copiar" shortcut="⌘C" onClick={cmd.copySelection} disabled={sel} />
                  <Btn icon="⊕" label="Duplicar" shortcut="⌘D" onClick={cmd.duplicate} disabled={sel} />
                </Stack>
                <Stack>
                  <Btn icon="✕" label="Excluir" shortcut="Del" onClick={cmd.deleteSelection} disabled={sel} />
                  <Btn icon="⬚" label="Selecionar tudo" shortcut="⌘A" onClick={cmd.selectAll} disabled={off(true, '')} />
                </Stack>
              </Group>
              <Group label="Fonte" wide>
                <Row>
                  <input
                    className="rb-font"
                    list="rb-font-list"
                    aria-label="Fonte"
                    placeholder={fontFamily === MIXED ? '— (várias)' : 'Fonte'}
                    title={txt ? `Fonte — ${txt}` : 'Fonte'}
                    disabled={!!txt}
                    value={fontDraft}
                    onChange={(e) => setFontDraft(e.target.value)}
                    onBlur={() => fontDraft && fontDraft !== fontFamily && cmd.setStyle('fontFamily', fontDraft)}
                    onKeyDown={(e) => e.key === 'Enter' && (e.target as HTMLInputElement).blur()}
                  />
                  <datalist id="rb-font-list">{fonts.map((f) => <option key={f} value={f} />)}</datalist>
                  <MixedSelect label="Tamanho (pt)" width={58} value={s((x) => x.fontSize)} disabled={txt}
                    options={FONT_SIZES.map((n) => ({ value: n, label: String(n) }))} onChange={(v) => cmd.setStyle('fontSize', v)} />
                </Row>
                <Row>
                  <Btn compact icon={<b>N</b>} label="Negrito" pressed={s((x) => x.bold)} disabled={txt} onClick={() => cmd.setStyle('bold', s((x) => x.bold) !== true)} />
                  <Btn compact icon={<i>I</i>} label="Itálico" pressed={s((x) => x.italic)} disabled={txt} onClick={() => cmd.setStyle('italic', s((x) => x.italic) !== true)} />
                  <Btn compact icon={<u>S</u>} label="Sublinhado" pressed={s((x) => x.underline)} disabled={txt} onClick={() => cmd.setStyle('underline', s((x) => x.underline) !== true)} />
                  <ColorBtn icon={<span className="rb-a">A</span>} label="Cor da fonte" value={s((x) => x.color)} disabled={txt} onChange={(v) => cmd.setStyle('color', v ?? '#000000')} />
                  <Btn compact icon="A⁺" label="Aumentar fonte" onClick={() => cmd.stepFontSize(1)} disabled={txt} />
                  <Btn compact icon="A⁻" label="Diminuir fonte" onClick={() => cmd.stepFontSize(-1)} disabled={txt} />
                </Row>
              </Group>
              <Group label="Parágrafo">
                <Row>
                  {([['left', '⫷', 'Alinhar à esquerda'], ['center', '≡', 'Centralizar'], ['right', '⫸', 'Alinhar à direita'], ['justify', '☰', 'Justificar']] as [Align, string, string][]).map(([a, icon, label]) => {
                    const v = s((x) => x.align);
                    return <Btn key={a} compact icon={icon} label={label} disabled={txt} pressed={v === MIXED ? MIXED : v === a} onClick={() => cmd.setStyle('align', a)} />;
                  })}
                </Row>
                <Row>
                  {([['top', '⤒', 'Alinhar ao topo'], ['middle', '↕', 'Alinhar ao meio'], ['bottom', '⤓', 'Alinhar à base']] as [VerticalAlign, string, string][]).map(([a, icon, label]) => {
                    const v = s((x) => x.verticalAlign);
                    return <Btn key={a} compact icon={icon} label={label} disabled={txt} pressed={v === MIXED ? MIXED : v === a} onClick={() => cmd.setStyle('verticalAlign', a)} />;
                  })}
                  {(() => {
                    const growable = els.filter((e) => e.type === 'label' || e.type === 'expression');
                    const v = common(growable, (e) => !!e.canGrow);
                    return (
                      <Btn compact icon="↵" label="Quebrar texto (pode crescer)" pressed={v} disabled={off(growable.length > 0, WHY.noText)}
                        onClick={() => cmd.editSelection('grow', (e) => void (e.canGrow = v !== true || undefined), (e) => e.type === 'label' || e.type === 'expression')} />
                    );
                  })()}
                </Row>
              </Group>
              <Group label="Borda e Preenchimento">
                <Row>
                  <MixedSelect label="Espessura da borda (pt)" width={92} value={sAll((x) => x.borderWidth)} disabled={sel}
                    options={BORDERS.map((n) => ({ value: n, label: n ? `${n} pt` : 'Sem borda' }))} onChange={(v) => cmd.setStyle('borderWidth', v)} />
                  <ColorBtn icon="▢" label="Cor da borda" value={sAll((x) => x.borderColor)} disabled={sel} onChange={(v) => cmd.setStyle('borderColor', v ?? '#000000')} />
                </Row>
                <Row>
                  <ColorBtn icon="◧" label="Preenchimento" allowEmpty value={sAll((x) => x.background)} disabled={sel} onChange={(v) => cmd.setStyle('background', v)} />
                </Row>
              </Group>
              <Group label="Número">
                <MixedSelect label="Formato" width={140} value={common(exprEls, (e) => (e.type === 'expression' ? e.format ?? '' : ''))}
                  disabled={off(exprEls.length > 0, WHY.noExpr)} options={FORMATS} onChange={(v) => cmd.setFormat(v)} />
                <Row>
                  <Btn compact icon="R$" label="Moeda" disabled={off(exprEls.length > 0, WHY.noExpr)} onClick={() => cmd.setFormat('currency:BRL')} />
                  <Btn compact icon="%" label="Percentual" disabled={off(exprEls.length > 0, WHY.noExpr)} onClick={() => cmd.setFormat('percent:1')} />
                  <Btn compact icon=",00" label="Número 2 casas" disabled={off(exprEls.length > 0, WHY.noExpr)} onClick={() => cmd.setFormat('number:2')} />
                </Row>
              </Group>
              <Group label="Organizar">
                <Stack>
                  <Btn icon="⇤" label="Esquerdas" onClick={() => cmd.arrange('left')} disabled={two} />
                  <Btn icon="⇥" label="Direitas" onClick={() => cmd.arrange('right')} disabled={two} />
                  <Btn icon="⇹" label="Centros" onClick={() => cmd.arrange('centerH')} disabled={two} />
                </Stack>
                <Stack>
                  <Btn icon="⤒" label="Topos" onClick={() => cmd.arrange('top')} disabled={two} />
                  <Btn icon="⤓" label="Bases" onClick={() => cmd.arrange('bottom')} disabled={two} />
                  <Btn icon="↔" label="Distribuir" onClick={() => cmd.arrange('distributeH')} disabled={off(els.length >= 3, WHY.three)} />
                </Stack>
                <Stack>
                  <Btn icon="⬌" label="Mesma largura" onClick={() => cmd.arrange('sameWidth')} disabled={two} />
                  <Btn icon="⬍" label="Mesma altura" onClick={() => cmd.arrange('sameHeight')} disabled={two} />
                  <Btn icon="↕" label="Distribuir vert." onClick={() => cmd.arrange('distributeV')} disabled={off(els.length >= 3, WHY.three)} />
                </Stack>
              </Group>
            </>
          )}

          {ui.ribbonTab === 'insert' && (
            <>
              <Group
                label="Elementos"
                info={`Inserção por clique vai para ${selectedBand ? BAND_LABELS[selectedBand.type] : 'a banda selecionada (ou o Detalhe)'}. Também é possível arrastar estes elementos, ou campos, parâmetros e imagens de Dados do Relatório, para uma banda.`}
              >
                {insertTools.map((t) => (
                  <Btn key={t.type} large icon={t.icon} label={t.label} onClick={() => cmd.insertElement(t.type)}
                    drag={{ elementType: t.type, label: t.label }}
                    disabled={off(true, '')} />
                ))}
              </Group>
              <Group label="Bandas">
                <Stack>
                  {bandBtn('reportHeader', '▤')}
                  {bandBtn('pageHeader', '▔')}
                  {bandBtn('detail', '☰')}
                </Stack>
                <Stack>
                  {bandBtn('pageFooter', '▁')}
                  {bandBtn('reportFooter', '▥')}
                  <Btn icon="⧉" label="Grupo (cabeçalho + rodapé)" onClick={cmd.addGroup} disabled={off(true, '')} />
                </Stack>
              </Group>
            </>
          )}

          {ui.ribbonTab === 'data' && (
            <>
              <Group label="Fontes">
                <Btn large icon="🗄" label="Nova fonte" onClick={cmd.addDataSource} />
                <Btn large icon="🔌" label="Conexões" onClick={cmd.openConnections} />
              </Group>
              <Group label="Campos">
                <Btn large icon="⟳" label="Atualizar campos" onClick={cmd.refreshAllFields}
                  disabled={report.dataSources.some((d) => d.queries.length) ? false : 'Crie uma fonte com query primeiro'} />
                <Btn large icon="☷" label="Dados do Relatório" onClick={() => ui.set({ showLeft: true, mode: 'design' })} />
              </Group>
              <Group label="Relatório">
                <Stack>
                  <Btn icon="ⓟ" label="Novo parâmetro" onClick={cmd.newParameter} />
                  <Btn icon="⧉" label="Novo grupo" onClick={cmd.addGroup} disabled={off(true, '')} />
                </Stack>
              </Group>
              <Group label="Validação">
                <Btn large icon={errors ? '✖' : '✔'} label={errors ? `${errors} erro(s)` : 'Validar'} pressed={ui.showIssues}
                  onClick={() => ui.set({ showIssues: !ui.showIssues })} />
              </Group>
            </>
          )}

          {ui.ribbonTab === 'layout' && (
            <>
              <Group label="Página">
                <Stack>
                  <MixedSelect<PaperSize> label="Papel" width={92} value={m.paperSize}
                    options={(['A4', 'A3', 'A5', 'Letter', 'Legal', 'Custom'] as PaperSize[]).map((p) => ({ value: p, label: p === 'Custom' ? 'Personalizado' : p }))}
                    onChange={(v) => useReport.getState().update((d) => {
                      d.meta.paperSize = v;
                      if (v === 'Custom' && !d.meta.customSize) d.meta.customSize = { width: 210, height: 297 };
                    })} />
                  <Row>
                    <Btn icon="▯" label="Retrato" pressed={m.orientation === 'portrait'} onClick={() => useReport.getState().update((d) => void (d.meta.orientation = 'portrait'))} />
                    <Btn icon="▭" label="Paisagem" pressed={m.orientation === 'landscape'} onClick={() => useReport.getState().update((d) => void (d.meta.orientation = 'landscape'))} />
                  </Row>
                </Stack>
              </Group>
              <Group label="Margens (mm)">
                <div className="rb-grid2">
                  {(['top', 'bottom', 'left', 'right'] as const).map((k) => (
                    <NumberBox key={k} label={{ top: 'Sup.', bottom: 'Inf.', left: 'Esq.', right: 'Dir.' }[k]} value={m.margins[k]}
                      onCommit={(v) => useReport.getState().update((d) => void (d.meta.margins[k] = v), `meta.m.${k}`)} />
                  ))}
                </div>
              </Group>
              <Group label="Banda">
                <NumberBox label="Altura (mm)" min={1} step={0.5} value={selectedBand?.height ?? 0}
                  disabled={!design ? WHY.design : !selectedBand && 'Clique na faixa de uma banda para selecioná-la'}
                  onCommit={(v) => useReport.getState().update((d) => {
                    const b = d.bands.find((x) => x.id === selectedBand?.id);
                    if (b) b.height = v;
                  }, `${selectedBand?.id}.h`)} />
              </Group>
              <Group label="Estilo de linhas">
                <Btn large icon="▤" label="Linhas zebradas" onClick={() => cmd.zebraDetail()}
                  disabled={off(bandTypes.has('detail'), 'O relatório não tem banda de detalhe')} />
              </Group>
            </>
          )}

          {ui.ribbonTab === 'view' && (
            <>
              <Group label="Zoom">
                <Btn large icon="🔍" label="100%" shortcut="⌘0" onClick={() => cmd.setZoom(1)} disabled={off(true, '')} />
                <Stack>
                  <MixedSelect label="Zoom" width={76} value={ui.zoom} disabled={off(true, '')}
                    options={ZOOMS.map((z) => ({ value: z, label: `${Math.round(z * 100)}%` }))} onChange={cmd.setZoom} />
                  <Row>
                    <Btn compact icon="−" label="Diminuir zoom" shortcut="⌘−" onClick={cmd.zoomOut} disabled={off(true, '')} />
                    <Btn compact icon="+" label="Aumentar zoom" shortcut="⌘+" onClick={cmd.zoomIn} disabled={off(true, '')} />
                  </Row>
                </Stack>
              </Group>
              <Group label="Grade">
                <Stack>
                  <Btn icon="#" label="Mostrar grade" pressed={ui.showGrid} onClick={() => ui.set({ showGrid: !ui.showGrid })} disabled={off(true, '')} />
                  <Btn icon="⌗" label="Encaixar na grade" pressed={ui.snapToGrid} onClick={() => ui.set({ snapToGrid: !ui.snapToGrid })} disabled={off(true, '')} />
                  <MixedSelect label="Tamanho da grade" width={76} value={ui.gridMm} disabled={off(true, '')}
                    options={[0.5, 1, 2, 2.5, 5].map((g) => ({ value: g, label: `${g} mm` }))} onChange={(v) => ui.set({ gridMm: v })} />
                </Stack>
              </Group>
              <Group label="Painéis">
                <Stack>
                  <Btn icon="◧" label="Dados do Relatório" pressed={ui.showLeft} onClick={() => ui.set({ showLeft: !ui.showLeft })} disabled={off(true, '')} />
                  <Btn icon="◨" label="Propriedades" pressed={ui.showRight} onClick={() => ui.set({ showRight: !ui.showRight })} disabled={off(true, '')} />
                </Stack>
              </Group>
            </>
          )}
        </div>
      )}
    </header>
  );
}
