import { useEffect, useState, type ReactNode } from 'react';

export function Section({ title, children, actions }: { title: string; children: ReactNode; actions?: ReactNode }) {
  return (
    <section className="section">
      <header className="section-title">
        <span>{title}</span>
        {actions && <span className="section-actions">{actions}</span>}
      </header>
      <div className="section-body">{children}</div>
    </section>
  );
}

export function Field({ label, children, hint }: { label: string; children: ReactNode; hint?: string }) {
  return (
    <label className="field" title={hint}>
      <span className="field-label">{label}</span>
      {children}
    </label>
  );
}

/** Input de texto que confirma no blur/Enter (evita um passo de undo por tecla em campos longos). */
export function TextInput({
  value,
  onCommit,
  placeholder,
  mono,
  live,
}: {
  value: string;
  onCommit: (v: string) => void;
  placeholder?: string;
  mono?: boolean;
  live?: boolean;
}) {
  const [v, setV] = useState(value);
  useEffect(() => setV(value), [value]);
  return (
    <input
      className={mono ? 'mono' : undefined}
      value={v}
      placeholder={placeholder}
      onChange={(e) => {
        setV(e.target.value);
        if (live) onCommit(e.target.value);
      }}
      onBlur={() => v !== value && onCommit(v)}
      onKeyDown={(e) => e.key === 'Enter' && (e.target as HTMLInputElement).blur()}
    />
  );
}

export function NumberInput({
  value,
  onCommit,
  step = 0.5,
  min,
  max,
}: {
  value: number;
  onCommit: (v: number) => void;
  step?: number;
  min?: number;
  max?: number;
}) {
  const [v, setV] = useState(String(value));
  useEffect(() => setV(String(Math.round(value * 100) / 100)), [value]);
  const commit = () => {
    const n = Number(v.replace(',', '.'));
    if (Number.isFinite(n) && n !== value) onCommit(min !== undefined ? Math.max(min, max !== undefined ? Math.min(max, n) : n) : n);
    else setV(String(value));
  };
  return (
    <input
      type="number"
      step={step}
      value={v}
      onChange={(e) => setV(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => e.key === 'Enter' && commit()}
    />
  );
}

export function ColorInput({ value, onCommit, allowEmpty }: { value?: string; onCommit: (v?: string) => void; allowEmpty?: boolean }) {
  return (
    <span className="color-input">
      <input type="color" value={value ?? '#ffffff'} onChange={(e) => onCommit(e.target.value)} />
      <TextInput value={value ?? ''} placeholder={allowEmpty ? 'nenhuma' : ''} mono onCommit={(v) => onCommit(v.trim() || undefined)} />
      {allowEmpty && value && (
        <button className="icon-btn" title="Remover cor" onClick={() => onCommit(undefined)}>
          ×
        </button>
      )}
    </span>
  );
}

export function Select<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T | undefined;
  options: (T | { value: T; label: string })[];
  onChange: (v: T) => void;
}) {
  return (
    <select value={value ?? ''} onChange={(e) => onChange(e.target.value as T)}>
      {value === undefined && <option value="">—</option>}
      {options.map((o) => {
        const opt = typeof o === 'string' ? { value: o, label: o } : o;
        return (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        );
      })}
    </select>
  );
}

export function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <label className="toggle">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span>{label}</span>
    </label>
  );
}

export function Modal({
  title,
  children,
  onClose,
  footer,
  wide,
}: {
  title: string;
  children: ReactNode;
  onClose: () => void;
  footer?: ReactNode;
  wide?: boolean;
}) {
  useEffect(() => {
    const h = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    window.addEventListener('keydown', h);
    return () => window.removeEventListener('keydown', h);
  }, [onClose]);
  return (
    <div className="modal-backdrop" onMouseDown={onClose}>
      <div className={`modal ${wide ? 'wide' : ''}`} onMouseDown={(e) => e.stopPropagation()}>
        <header className="modal-title">
          <span>{title}</span>
          <button className="icon-btn" onClick={onClose}>
            ×
          </button>
        </header>
        <div className="modal-body">{children}</div>
        {footer && <footer className="modal-footer">{footer}</footer>}
      </div>
    </div>
  );
}

export function DataGrid({ columns, rows }: { columns: { name: string; dataType: string }[]; rows: unknown[][] }) {
  return (
    <div className="grid-wrap">
      <table className="data-grid">
        <thead>
          <tr>
            {columns.map((c) => (
              <th key={c.name}>
                {c.name}
                <small>{c.dataType}</small>
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={i}>
              {r.map((v, j) => (
                <td key={j} className={typeof v === 'number' ? 'num' : undefined}>
                  {v === null ? <span className="null">NULL</span> : String(v)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
