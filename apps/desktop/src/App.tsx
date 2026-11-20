import { useCallback, useEffect, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { ValidationIssue } from './domain/types';
import { cloneElements } from './domain/factory';
import { api } from './infra/api';
import { dialogs } from './infra/dialogs';
import { draftElement, findElement, useReport } from './state/reportStore';
import { useUi, type LeftTab } from './state/uiStore';
import { DesignerCanvas } from './features/designer/DesignerCanvas';
import { Toolbox } from './features/toolbox/Toolbox';
import { PropertiesPanel } from './features/properties/PropertiesPanel';
import { DataPanel } from './features/data/DataPanel';
import { ConnectionsPanel } from './features/connections/ConnectionsPanel';
import { PreviewPane } from './features/preview/PreviewPane';

const TABS: { id: LeftTab; label: string }[] = [
  { id: 'toolbox', label: 'Elementos' },
  { id: 'data', label: 'Dados' },
  { id: 'connections', label: 'Conexões' },
];

export default function App() {
  const { report, filePath, dirty, past, future, undo, redo } = useReport();
  const ui = useUi();
  const [issues, setIssues] = useState<ValidationIssue[]>([]);
  const [showIssues, setShowIssues] = useState(false);

  // ---- inicialização
  useEffect(() => {
    api.appInfo().then((info) => useUi.getState().set({ info }), (e: Error) => ui.notify(e.message, 'error'));
    api.listConnections().then((connections) => useUi.getState().set({ connections }), () => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ---- título da janela
  useEffect(() => {
    const name = filePath?.split(/[\\/]/).pop() ?? 'Sem título';
    getCurrentWindow().setTitle(`${dirty ? '● ' : ''}${name} — NovaReport Builder`).catch(() => {});
  }, [filePath, dirty]);

  // ---- validação contínua (domínio + sintaxe das expressões), com debounce
  useEffect(() => {
    const t = setTimeout(() => api.validateReport(report).then(setIssues, () => {}), 300);
    return () => clearTimeout(t);
  }, [report]);

  // ---- operações de arquivo
  const confirmDiscard = useCallback(async () => !useReport.getState().dirty || (await dialogs.confirm('Descartar alterações não salvas?')), []);

  const newReport = useCallback(async () => {
    if (!(await confirmDiscard())) return;
    useReport.getState().load(await api.newReport('Novo relatório'));
    useUi.getState().select([]);
  }, [confirmDiscard]);

  const openReport = useCallback(async () => {
    if (!(await confirmDiscard())) return;
    const path = await dialogs.openReport();
    if (!path) return;
    try {
      const { report: r, issues: i } = await api.openReport(path);
      useReport.getState().load(r, path);
      useUi.getState().set({ fields: {}, paramValues: {}, selectedIds: [], selectedBandId: undefined });
      setIssues(i);
    } catch (e) {
      dialogs.error((e as Error).message);
    }
  }, [confirmDiscard]);

  const save = useCallback(async (saveAs = false) => {
    const st = useReport.getState();
    let path = st.filePath;
    if (!path || saveAs) {
      path = (await dialogs.saveReport(`${st.report.meta.title || 'relatorio'}.nrep`)) ?? undefined;
      if (!path) return;
      if (!path.toLowerCase().endsWith('.nrep')) path += '.nrep';
    }
    try {
      setIssues(await api.saveReport(path, st.report));
      useReport.getState().markSaved(path);
      useUi.getState().notify('Relatório salvo', 'success');
    } catch (e) {
      dialogs.error((e as Error).message);
    }
  }, []);

  // ---- atalhos de teclado
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      const typing = ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName);
      const mod = e.metaKey || e.ctrlKey;
      const { selectedIds, gridMm, select, set } = useUi.getState();
      const update = useReport.getState().update;

      if (mod && e.key.toLowerCase() === 's') { e.preventDefault(); save(e.shiftKey); return; }
      if (mod && e.key.toLowerCase() === 'o') { e.preventDefault(); openReport(); return; }
      if (mod && e.key.toLowerCase() === 'n') { e.preventDefault(); newReport(); return; }
      if (mod && e.key.toLowerCase() === 'p') { e.preventDefault(); set({ mode: useUi.getState().mode === 'design' ? 'preview' : 'design' }); return; }
      if (typing || useUi.getState().mode !== 'design') return;

      if (mod && e.key.toLowerCase() === 'z') { e.preventDefault(); if (e.shiftKey) useReport.getState().redo(); else useReport.getState().undo(); return; }
      if (mod && e.key.toLowerCase() === 'y') { e.preventDefault(); useReport.getState().redo(); return; }
      if (e.key === 'Escape') { select([]); return; }
      if (!selectedIds.length) return;

      const r = useReport.getState().report;
      if (e.key === 'Delete' || e.key === 'Backspace') {
        e.preventDefault();
        update((d) => { for (const b of d.bands) b.elements = b.elements.filter((x) => !selectedIds.includes(x.id)); });
        select([]);
      } else if (mod && e.key.toLowerCase() === 'c') {
        set({ clipboard: selectedIds.map((id) => findElement(r, id)?.element).filter((x) => !!x) });
      } else if (mod && (e.key.toLowerCase() === 'v' || e.key.toLowerCase() === 'd')) {
        e.preventDefault();
        const source = e.key.toLowerCase() === 'd'
          ? selectedIds.map((id) => findElement(r, id)?.element).filter((x) => !!x)
          : useUi.getState().clipboard;
        if (!source.length) return;
        const bandId = useUi.getState().selectedBandId ?? findElement(r, selectedIds[0])?.band.id ?? r.bands[0].id;
        const clones = cloneElements(r, source);
        update((d) => { d.bands.find((b) => b.id === bandId)?.elements.push(...clones); });
        select(clones.map((c) => c.id), bandId);
      } else if (e.key.startsWith('Arrow')) {
        e.preventDefault();
        const step = (gridMm || 0.5) * (e.shiftKey ? 5 : 1);
        const [dx, dy] = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }[e.key] ?? [0, 0];
        update((d) => {
          for (const id of selectedIds) {
            const el = draftElement(d, id);
            if (el) { el.x = Math.max(0, Math.round((el.x + dx) * 10) / 10); el.y = Math.max(0, Math.round((el.y + dy) * 10) / 10); }
          }
        }, 'nudge');
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [save, openReport, newReport]);

  // ---- notificações temporárias
  useEffect(() => {
    if (!ui.toast) return;
    const t = setTimeout(() => useUi.getState().set({ toast: undefined }), ui.toast.kind === 'error' ? 6000 : 2500);
    return () => clearTimeout(t);
  }, [ui.toast]);

  const errors = issues.filter((i) => i.severity === 'error').length;
  const warnings = issues.length - errors;

  return (
    <div className="app">
      <header className="toolbar">
        <div className="brand">
          <span className="logo">N</span>
          <span>NovaReport</span>
        </div>
        <div className="tb-group">
          <button onClick={newReport} title="Novo (⌘N)">Novo</button>
          <button onClick={openReport} title="Abrir (⌘O)">Abrir</button>
          <button onClick={() => save(false)} title="Salvar (⌘S)">Salvar</button>
          <button onClick={() => save(true)} title="Salvar como (⇧⌘S)">Salvar como</button>
        </div>
        <div className="tb-group">
          <button onClick={undo} disabled={!past.length} title="Desfazer (⌘Z)">↶</button>
          <button onClick={redo} disabled={!future.length} title="Refazer (⇧⌘Z)">↷</button>
        </div>
        {ui.mode === 'design' && (
          <div className="tb-group">
            <button onClick={() => ui.set({ zoom: Math.max(0.5, +(ui.zoom - 0.25).toFixed(2)) })}>−</button>
            <span className="zoom-label">{Math.round(ui.zoom * 100)}%</span>
            <button onClick={() => ui.set({ zoom: Math.min(4, +(ui.zoom + 0.25).toFixed(2)) })}>+</button>
            <label className="toggle"><input type="checkbox" checked={ui.showGrid} onChange={(e) => ui.set({ showGrid: e.target.checked })} /> Grade</label>
            <label className="toggle"><input type="checkbox" checked={ui.snapToGrid} onChange={(e) => ui.set({ snapToGrid: e.target.checked })} /> Encaixar</label>
            <select value={ui.gridMm} onChange={(e) => ui.set({ gridMm: Number(e.target.value) })} title="Tamanho da grade">
              {[0.5, 1, 2, 2.5, 5].map((g) => <option key={g} value={g}>{g} mm</option>)}
            </select>
          </div>
        )}
        <span className="spacer" />
        <div className="mode-switch">
          <button className={ui.mode === 'design' ? 'active' : ''} onClick={() => ui.set({ mode: 'design' })}>Design</button>
          <button className={ui.mode === 'preview' ? 'active' : ''} onClick={() => ui.set({ mode: 'preview' })}>Pré-visualizar</button>
        </div>
      </header>

      <main className="workspace">
        {ui.mode === 'design' ? (
          <>
            <aside className="left">
              <nav className="tabs">
                {TABS.map((t) => (
                  <button key={t.id} className={ui.leftTab === t.id ? 'active' : ''} onClick={() => ui.set({ leftTab: t.id })}>{t.label}</button>
                ))}
              </nav>
              <div className="panel-scroll">
                {ui.leftTab === 'toolbox' && <Toolbox />}
                {ui.leftTab === 'data' && <DataPanel />}
                {ui.leftTab === 'connections' && <ConnectionsPanel />}
              </div>
            </aside>
            <section className="center">
              <DesignerCanvas />
            </section>
            <aside className="right">
              <div className="panel-scroll">
                <PropertiesPanel />
              </div>
            </aside>
          </>
        ) : (
          <section className="center full">
            <PreviewPane />
          </section>
        )}
      </main>

      <footer className="statusbar">
        <span>{filePath ?? 'Sem título'}{dirty ? ' · não salvo' : ''}</span>
        <span className="spacer" />
        <span>{report.bands.reduce((n, b) => n + b.elements.length, 0)} elementos · {report.bands.length} bandas</span>
        <button className={`issues ${errors ? 'has-errors' : warnings ? 'has-warnings' : 'clean'}`} onClick={() => setShowIssues((v) => !v)}>
          {errors ? `✖ ${errors} erro(s)` : '✔ válido'}{warnings ? ` · ⚠ ${warnings}` : ''}
        </button>
        <span className="muted">v{ui.info?.version ?? '…'}</span>
      </footer>

      {showIssues && issues.length > 0 && (
        <div className="issues-panel">
          <header>
            <strong>Validação</strong>
            <button className="icon-btn" onClick={() => setShowIssues(false)}>×</button>
          </header>
          <ul>
            {issues.map((i, k) => (
              <li key={k} className={i.severity}>
                <span className="mono">{i.path}</span>
                <span>{i.message}</span>
              </li>
            ))}
          </ul>
        </div>
      )}

      {ui.toast && <div className={`toast ${ui.toast.kind}`}>{ui.toast.text}</div>}
    </div>
  );
}
