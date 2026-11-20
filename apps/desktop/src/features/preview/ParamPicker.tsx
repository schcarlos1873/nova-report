// Seletor de valores de parâmetro com lista (simples ou múltiplo), com busca e estados de cascata.
import { useEffect, useRef, useState } from 'react';
import type { ParameterDef, ParameterOption } from '../../domain/types';

export interface ListState {
  status: 'loading' | 'ready' | 'waiting' | 'error';
  options: ParameterOption[];
  truncated?: boolean;
  waitingFor?: string[];
  error?: string;
}

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

export function ParamPicker({ p, value, list, onChange }: {
  p: ParameterDef;
  value: unknown;
  list?: ListState;
  onChange: (v: unknown) => void;
}) {
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState('');
  const ref = useRef<HTMLDivElement>(null);
  const options = list?.options ?? [];
  const selected: unknown[] = p.multiValue ? (Array.isArray(value) ? value : []) : value === null || value === undefined ? [] : [value];
  const isSel = (v: unknown) => selected.some((s) => same(s, v));

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && setOpen(false);
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && setOpen(false);
    window.addEventListener('mousedown', onDown);
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('mousedown', onDown);
      window.removeEventListener('keydown', onKey);
    };
  }, [open]);

  const waiting = list?.status === 'waiting';
  const disabled = !list || waiting || list.status === 'loading';
  let summary: string;
  if (!list || list.status === 'loading') summary = 'Carregando…';
  else if (waiting) summary = `Selecione ${list.waitingFor?.join(', ')} primeiro`;
  else if (list.status === 'error') summary = 'Erro ao carregar';
  else if (!selected.length) summary = p.multiValue ? 'Nenhum selecionado' : '(selecione)';
  else if (selected.length === 1) summary = options.find((o) => same(o.value, selected[0]))?.label ?? String(selected[0]);
  else if (selected.length === options.length) summary = `Todos (${selected.length})`;
  else summary = `${selected.length} selecionados`;

  const q = filter.trim().toLowerCase();
  const visible = q ? options.filter((o) => o.label.toLowerCase().includes(q) || String(o.value).toLowerCase().includes(q)) : options;

  const pick = (v: unknown) => {
    if (!p.multiValue) {
      onChange(v);
      setOpen(false);
      return;
    }
    onChange(isSel(v) ? selected.filter((s) => !same(s, v)) : [...selected, v]);
  };

  return (
    <div className="param-picker" ref={ref}>
      <button
        type="button"
        className={`picker-btn${list?.status === 'error' ? ' error' : ''}`}
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        title={list?.error ?? summary}
        onClick={() => setOpen((o) => !o)}
      >
        <span className="picker-summary">{summary}</span>
        <span aria-hidden>▾</span>
      </button>
      {open && (
        <div className="picker-pop">
          {options.length > 8 && (
            <input autoFocus className="picker-search" placeholder="Buscar…" value={filter} onChange={(e) => setFilter(e.target.value)} />
          )}
          {p.multiValue && (
            <div className="picker-actions">
              <button type="button" className="link" onClick={() => onChange(Array.from(new Set([...selected, ...visible.map((o) => o.value)].map((v) => JSON.stringify(v)))).map((s) => JSON.parse(s)))}>
                Selecionar {q ? 'filtrados' : 'tudo'}
              </button>
              <button type="button" className="link" onClick={() => onChange([])}>Limpar</button>
            </div>
          )}
          <ul role="listbox" aria-multiselectable={!!p.multiValue}>
            {!p.required && !p.multiValue && (
              <li role="option" tabIndex={0} aria-selected={!selected.length} className={!selected.length ? 'sel' : ''}
                onClick={() => pick(null)} onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), pick(null))}>
                <span className="muted">(nenhum)</span>
              </li>
            )}
            {visible.map((o) => (
              <li key={JSON.stringify(o.value)} role="option" tabIndex={0} aria-selected={isSel(o.value)} className={isSel(o.value) ? 'sel' : ''}
                onClick={() => pick(o.value)} onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), pick(o.value))}>
                {p.multiValue && <input type="checkbox" readOnly checked={isSel(o.value)} tabIndex={-1} />}
                <span>{o.label}</span>
              </li>
            ))}
            {!visible.length && <li className="muted">Nenhum valor</li>}
          </ul>
          {list?.truncated && <p className="muted small picker-note">Mostrando os primeiros {options.length} valores.</p>}
        </div>
      )}
      {list?.status === 'error' && <span className="picker-error" title={list.error}>⚠ {list.error}</span>}
    </div>
  );
}
