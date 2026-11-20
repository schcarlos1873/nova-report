import { useCallback, useEffect, useRef, useState } from 'react';
import type { ExportFormat, ParameterDef, PreviewResult } from '../../domain/types';
import { api } from '../../infra/api';
import { dialogs } from '../../infra/dialogs';
import { useReport } from '../../state/reportStore';
import { useUi } from '../../state/uiStore';

const EXPORTS: { format: ExportFormat; label: string; ext: string }[] = [
  { format: 'pdf', label: 'PDF', ext: 'pdf' },
  { format: 'xlsx', label: 'Excel', ext: 'xlsx' },
  { format: 'csv', label: 'CSV', ext: 'csv' },
  { format: 'typst', label: 'Typst', ext: 'typ' },
];

export function PreviewPane() {
  const report = useReport((s) => s.report);
  const filePath = useReport((s) => s.filePath);
  const { paramValues, set, notify } = useUi();
  const [result, setResult] = useState<PreviewResult>();
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [page, setPage] = useState(1);
  const [zoom, setZoom] = useState(1);
  const [live, setLive] = useState(true);
  const scrollRef = useRef<HTMLDivElement>(null);
  const seq = useRef(0);

  const run = useCallback(
    async (refresh: boolean) => {
      const id = ++seq.current;
      setBusy(true);
      try {
        const r = await api.preview(useReport.getState().report, useUi.getState().paramValues, useReport.getState().filePath, refresh);
        if (id !== seq.current) return; // resposta obsoleta
        setResult(r);
        setError(undefined);
        setPage((p) => Math.min(Math.max(1, p), Math.max(1, r.pageCount)));
      } catch (e) {
        if (id === seq.current) setError((e as Error).message);
      } finally {
        if (id === seq.current) setBusy(false);
      }
    },
    [],
  );

  // pré-visualização ao vivo: re-renderiza (com cache de dados) quando o layout muda
  useEffect(() => {
    if (!live && result) return;
    const t = setTimeout(() => run(false), result ? 250 : 0);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [report, live]);

  const goTo = (n: number) => {
    const p = Math.min(Math.max(1, n), result?.pageCount ?? 1);
    setPage(p);
    scrollRef.current?.querySelector(`[data-page="${p}"]`)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  };

  const onScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    const pages = Array.from(el.querySelectorAll<HTMLElement>('[data-page]'));
    const mid = el.scrollTop + el.clientHeight / 3;
    const current = pages.find((p) => p.offsetTop + p.offsetHeight > mid);
    if (current) setPage(Number(current.dataset.page));
  };

  const doExport = async (format: ExportFormat, ext: string) => {
    const base = (filePath?.replace(/\.nrep$/i, '') ?? report.meta.title.replace(/[^\w\- ]/g, '')) + `.${ext}`;
    const out = await dialogs.saveExport(ext, base);
    if (!out) return;
    try {
      const bytes = await api.exportReport(report, paramValues, format, out, filePath);
      notify(`Exportado: ${out} (${(bytes / 1024).toFixed(0)} KB)`, 'success');
    } catch (e) {
      notify((e as Error).message, 'error');
    }
  };

  // largura da página em px CSS no zoom 100% (1 mm = 96/25.4 px)
  const pageWidthPx = ((result?.pageWidthMm ?? 210) * 96) / 25.4;

  return (
    <div className="preview">
      {report.parameters.length > 0 && (
        <div className="param-bar">
          {report.parameters.map((p) => (
            <ParamInput key={p.name} p={p} value={paramValues[p.name]} onChange={(v) => set({ paramValues: { ...useUi.getState().paramValues, [p.name]: v } })} />
          ))}
          <button className="primary" onClick={() => run(true)} disabled={busy}>Aplicar</button>
        </div>
      )}
      <div className="preview-toolbar">
        <button onClick={() => goTo(1)} disabled={page <= 1}>⏮</button>
        <button onClick={() => goTo(page - 1)} disabled={page <= 1}>◀</button>
        <span className="page-indicator">
          Página <input type="number" value={page} min={1} onChange={(e) => goTo(Number(e.target.value))} /> de {result?.pageCount ?? '–'}
        </span>
        <button onClick={() => goTo(page + 1)} disabled={!result || page >= result.pageCount}>▶</button>
        <button onClick={() => goTo(result?.pageCount ?? 1)} disabled={!result || page >= result.pageCount}>⏭</button>
        <span className="sep" />
        <button onClick={() => setZoom((z) => Math.max(0.3, +(z - 0.1).toFixed(2)))}>−</button>
        <span className="zoom-label">{Math.round(zoom * 100)}%</span>
        <button onClick={() => setZoom((z) => Math.min(3, +(z + 0.1).toFixed(2)))}>+</button>
        <button onClick={() => setZoom(scrollRef.current ? (scrollRef.current.clientWidth - 64) / pageWidthPx : 1)}>Ajustar</button>
        <span className="sep" />
        <label className="toggle"><input type="checkbox" checked={live} onChange={(e) => setLive(e.target.checked)} /> Ao vivo</label>
        <button onClick={() => run(true)} disabled={busy} title="Reexecuta as queries">⟳ Atualizar dados</button>
        <span className="spacer" />
        {result && (
          <span className="muted small" title={`dados ${result.stats.dataMs} ms${result.stats.dataCached ? ' (cache)' : ''} · layout ${result.stats.layoutMs} ms · Typst ${result.stats.compileMs} ms`}>
            {busy ? 'Renderizando…' : `${result.renderTimeMs} ms`}
            {result.stats.dataCached ? ' · cache' : ''}
          </span>
        )}
        {EXPORTS.map((x) => (
          <button key={x.format} onClick={() => doExport(x.format, x.ext)}>⤓ {x.label}</button>
        ))}
      </div>
      {error && <pre className="preview-error">{error}</pre>}
      {result && result.warnings.length > 0 && (
        <details className="preview-warnings">
          <summary>{result.warnings.length} aviso(s)</summary>
          <ul>{result.warnings.map((w, i) => <li key={i}>{w}</li>)}</ul>
        </details>
      )}
      <div className="preview-pages" ref={scrollRef} onScroll={onScroll}>
        {result?.pagesSvg.map((svg, i) => (
          <div
            key={i}
            data-page={i + 1}
            className={`page ${page === i + 1 ? 'current' : ''}`}
            style={{ width: pageWidthPx * zoom }}
            // SVG produzido pelo Typst a partir de strings já escapadas pelo transpilador
            dangerouslySetInnerHTML={{ __html: svg }}
          />
        ))}
        {!result && !error && <p className="muted">Renderizando…</p>}
      </div>
    </div>
  );
}

function ParamInput({ p, value, onChange }: { p: ParameterDef; value: unknown; onChange: (v: unknown) => void }) {
  const label = p.label || p.name;
  const shown = value ?? (typeof p.defaultValue === 'string' && /^\d{4}-\d{2}-\d{2}/.test(p.defaultValue) ? p.defaultValue : '');
  const common = { title: p.name };
  let input: React.ReactNode;
  switch (p.type) {
    case 'bool':
      input = <input {...common} type="checkbox" checked={value === true} onChange={(e) => onChange(e.target.checked)} />;
      break;
    case 'date':
      input = <input {...common} type="date" value={String(shown)} onChange={(e) => onChange(e.target.value || null)} />;
      break;
    case 'datetime':
      input = <input {...common} type="datetime-local" value={String(shown)} onChange={(e) => onChange(e.target.value || null)} />;
      break;
    case 'int':
    case 'float':
      input = <input {...common} type="number" step={p.type === 'int' ? 1 : 0.01} value={String(shown)} onChange={(e) => onChange(e.target.value === '' ? null : Number(e.target.value))} />;
      break;
    default:
      input = <input {...common} value={String(shown)} onChange={(e) => onChange(e.target.value)} />;
  }
  return (
    <label className="param">
      <span>{label}{p.required ? ' *' : ''}</span>
      {input}
    </label>
  );
}
