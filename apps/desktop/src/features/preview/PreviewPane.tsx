import { useCallback, useEffect, useRef, useState } from 'react';
import type { ExportFormat, ParameterDef, PreviewResult } from '../../domain/types';
import { api } from '../../infra/api';
import { dialogs } from '../../infra/dialogs';
import { useReport } from '../../state/reportStore';
import { useUi } from '../../state/uiStore';
import { ParamPicker, type ListState } from './ParamPicker';
import { defaultCandidates, dependentsOf, emptyValue, keepValid, topoOrder } from './cascade';

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

  // ---- listas de valores e filtros em cascata
  const [lists, setLists] = useState<Record<string, ListState>>({});
  const listSeq = useRef<Record<string, number>>({});
  const parentTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  // recarrega só quando a configuração dos parâmetros/queries muda (não a cada ajuste de layout)
  const paramsKey = JSON.stringify([report.parameters, report.dataSources]);

  /** Carrega as listas de `targets` em ordem; cada filho vê os valores já ajustados dos pais. */
  const loadLists = useCallback(async (targets: ParameterDef[], initial: boolean) => {
    for (const p of targets) {
      if (!p.availableValues) continue;
      const seq = (listSeq.current[p.name] = (listSeq.current[p.name] ?? 0) + 1);
      setLists((l) => ({ ...l, [p.name]: { status: 'loading', options: l[p.name]?.options ?? [] } }));
      try {
        const { report: r, filePath: fp } = useReport.getState();
        const res = await api.parameterValues(r, p.name, useUi.getState().paramValues, fp);
        if (seq !== listSeq.current[p.name]) return; // resposta obsoleta: outra mudança já disparou nova carga
        const options = res.values;
        const state: ListState = res.waitingFor.length
          ? { status: 'waiting', options: [], waitingFor: res.waitingFor }
          : { status: 'ready', options, truncated: res.truncated };
        setLists((l) => ({ ...l, [p.name]: state }));
        const cur = useUi.getState().paramValues[p.name];
        const allowed = options.map((o) => o.value);
        let next: unknown;
        if (cur === undefined && initial) {
          // primeira carga: aplica o padrão do .nrep se ele existir na lista
          const wanted = defaultCandidates(p).map(String);
          const picked = allowed.filter((v) => wanted.includes(String(v)));
          next = p.multiValue ? picked : picked[0] ?? null;
        } else {
          next = keepValid(p, cur ?? emptyValue(p), allowed);
        }
        set({ paramValues: { ...useUi.getState().paramValues, [p.name]: next } });
      } catch (e) {
        if (seq === listSeq.current[p.name]) setLists((l) => ({ ...l, [p.name]: { status: 'error', options: [], error: (e as Error).message } }));
      }
    }
  }, [set]);

  useEffect(() => {
    loadLists(topoOrder(useReport.getState().report.parameters), true);
  }, [paramsKey, loadLists]);

  /** Mudança feita pelo usuário: limpa os dependentes e recarrega suas listas. */
  const changeParam = (p: ParameterDef, v: unknown) => {
    const params = useReport.getState().report.parameters;
    const deps = dependentsOf(params, p.name);
    const values = { ...useUi.getState().paramValues, [p.name]: v };
    for (const d of deps) values[d.name] = emptyValue(d);
    set({ paramValues: values });
    if (!deps.length) return;
    clearTimeout(parentTimer.current);
    // campos digitados (texto, data, número) esperam a digitação parar
    parentTimer.current = setTimeout(() => loadLists(deps, false), p.availableValues ? 0 : 400);
  };

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
          {report.parameters.map((p) =>
            p.availableValues ? (
              <div key={p.name} className="param" role="group" aria-label={p.label || p.name}>
                <span>{p.label || p.name}{p.required ? ' *' : ''}{p.multiValue ? ' (vários)' : ''}</span>
                <ParamPicker p={p} value={paramValues[p.name]} list={lists[p.name]} onChange={(v) => changeParam(p, v)} />
              </div>
            ) : (
              <ParamInput key={p.name} p={p} value={paramValues[p.name]} onChange={(v) => changeParam(p, v)} />
            ),
          )}
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
