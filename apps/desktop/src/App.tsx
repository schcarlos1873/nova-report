import { useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { api } from './infra/api';
import { useReport } from './state/reportStore';
import { LEFT_MAX, LEFT_MIN, persistLeftWidth, useUi } from './state/uiStore';
import * as cmd from './state/commands';
import { Ribbon } from './features/ribbon/Ribbon';
import { DesignerCanvas } from './features/designer/DesignerCanvas';
import { PropertiesPanel } from './features/properties/PropertiesPanel';
import { DataDialogs, ReportDataPanel } from './features/reportData/ReportDataPanel';
import { PreviewPane } from './features/preview/PreviewPane';

/** Arrastar a borda direita da barra "Dados do Relatório" ajusta a largura (duplo clique restaura). */
function Splitter() {
  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    const el = e.currentTarget;
    el.setPointerCapture(e.pointerId);
    const startX = e.clientX;
    const startW = useUi.getState().leftWidth;
    const move = (ev: PointerEvent) => {
      useUi.getState().set({ leftWidth: Math.min(LEFT_MAX, Math.max(LEFT_MIN, startW + ev.clientX - startX)) });
    };
    const up = () => {
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerup', up);
      persistLeftWidth(useUi.getState().leftWidth);
    };
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerup', up);
  };
  const onKeyDown = (e: React.KeyboardEvent) => {
    const step = e.shiftKey ? 40 : 10;
    const d = e.key === 'ArrowLeft' ? -step : e.key === 'ArrowRight' ? step : 0;
    if (!d) return;
    e.preventDefault();
    e.stopPropagation();
    const w = Math.min(LEFT_MAX, Math.max(LEFT_MIN, useUi.getState().leftWidth + d));
    useUi.getState().set({ leftWidth: w });
    persistLeftWidth(w);
  };
  const w = useUi((s) => s.leftWidth);
  return (
    <div
      className="splitter"
      role="separator"
      aria-orientation="vertical"
      aria-label="Redimensionar Dados do Relatório"
      aria-valuemin={LEFT_MIN}
      aria-valuemax={LEFT_MAX}
      aria-valuenow={w}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onKeyDown={onKeyDown}
      onDoubleClick={() => {
        useUi.getState().set({ leftWidth: 280 });
        persistLeftWidth(280);
      }}
    />
  );
}

export default function App() {
  const { report, filePath, dirty } = useReport();
  const ui = useUi();
  const issues = ui.issues;

  // ---- inicialização
  useEffect(() => {
    api.appInfo().then((info) => useUi.getState().set({ info }), (e: Error) => useUi.getState().notify(e.message, 'error'));
    api.listConnections().then((connections) => useUi.getState().set({ connections }), () => {});
  }, []);

  // ---- título da janela
  useEffect(() => {
    const name = filePath?.split(/[\\/]/).pop() ?? 'Sem título';
    getCurrentWindow().setTitle(`${dirty ? '● ' : ''}${name} — NovaReport Builder`).catch(() => {});
  }, [filePath, dirty]);

  // ---- validação contínua (domínio + sintaxe das expressões), com debounce
  useEffect(() => {
    const t = setTimeout(() => api.validateReport(report).then((i) => useUi.getState().set({ issues: i }), () => {}), 300);
    return () => clearTimeout(t);
  }, [report]);

  // ---- atalhos de teclado (mesmos comandos da Ribbon)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      const typing = ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName) || target.isContentEditable;
      const mod = e.metaKey || e.ctrlKey;
      const key = e.key.toLowerCase();
      const { selectedIds, gridMm, select, set } = useUi.getState();

      if (mod && key === 's') { e.preventDefault(); cmd.save(e.shiftKey); return; }
      if (mod && key === 'o') { e.preventDefault(); cmd.openReport(); return; }
      if (mod && key === 'n') { e.preventDefault(); cmd.newReport(); return; }
      if (mod && key === 'p') { e.preventDefault(); cmd.toggleMode(); return; }
      if (e.ctrlKey && e.key === 'F1') { e.preventDefault(); set({ ribbonCollapsed: !useUi.getState().ribbonCollapsed }); return; }
      if (typing || useUi.getState().mode !== 'design') return;

      if (mod && (key === '=' || key === '+')) { e.preventDefault(); cmd.zoomIn(); return; }
      if (mod && key === '-') { e.preventDefault(); cmd.zoomOut(); return; }
      if (mod && key === '0') { e.preventDefault(); cmd.setZoom(1); return; }
      if (mod && key === 'z') { e.preventDefault(); if (e.shiftKey) useReport.getState().redo(); else useReport.getState().undo(); return; }
      if (mod && key === 'y') { e.preventDefault(); useReport.getState().redo(); return; }
      if (mod && key === 'a') { e.preventDefault(); cmd.selectAll(); return; }
      if (mod && key === 'v') { e.preventDefault(); cmd.paste(); return; }
      if (e.key === 'Escape') { select([]); return; }
      if (!selectedIds.length) return;

      if (e.key === 'Delete' || e.key === 'Backspace') {
        e.preventDefault();
        cmd.deleteSelection();
      } else if (mod && key === 'c') {
        cmd.copySelection();
      } else if (mod && key === 'x') {
        e.preventDefault();
        cmd.cutSelection();
      } else if (mod && key === 'd') {
        e.preventDefault();
        cmd.duplicate();
      } else if (e.key.startsWith('Arrow')) {
        e.preventDefault();
        const step = (gridMm || 0.5) * (e.shiftKey ? 5 : 1);
        const [dx, dy] = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }[e.key] ?? [0, 0];
        cmd.nudge(dx, dy);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  // ---- notificações temporárias
  useEffect(() => {
    if (!ui.toast) return;
    const t = setTimeout(() => useUi.getState().set({ toast: undefined }), ui.toast.kind === 'error' ? 6000 : 2500);
    return () => clearTimeout(t);
  }, [ui.toast]);

  const errors = issues.filter((i) => i.severity === 'error').length;
  const warnings = issues.length - errors;

  const columns = [ui.showLeft ? `${ui.leftWidth}px` : null, '1fr', ui.showRight ? '300px' : null].filter(Boolean).join(' ');

  return (
    <div className="app">
      <Ribbon />

      <main className="workspace" style={ui.mode === 'design' ? { gridTemplateColumns: columns } : undefined}>
        {ui.mode === 'design' ? (
          <>
            {ui.showLeft && (
              <aside className="left" aria-label="Dados do Relatório">
                <ReportDataPanel />
                <Splitter />
              </aside>
            )}
            <section className="center">
              <DesignerCanvas />
            </section>
            {ui.showRight && (
              <aside className="right" aria-label="Propriedades">
                <div className="panel-scroll">
                  <PropertiesPanel />
                </div>
              </aside>
            )}
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
        <span>Zoom {Math.round(ui.zoom * 100)}%</span>
        <button className={`issues ${errors ? 'has-errors' : warnings ? 'has-warnings' : 'clean'}`} onClick={() => ui.set({ showIssues: !ui.showIssues })}>
          {errors ? `✖ ${errors} erro(s)` : '✔ válido'}{warnings ? ` · ⚠ ${warnings}` : ''}
        </button>
        <span className="muted">v{ui.info?.version ?? '…'}</span>
      </footer>

      {ui.showIssues && (
        <div className="issues-panel">
          <header>
            <strong>Validação</strong>
            <button className="icon-btn" aria-label="Fechar" onClick={() => ui.set({ showIssues: false })}>×</button>
          </header>
          {issues.length === 0 && <p className="muted" style={{ padding: '8px 12px', margin: 0 }}>✔ Nenhum problema encontrado.</p>}
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

      <DataDialogs />
      {ui.toast && <div className={`toast ${ui.toast.kind}`}>{ui.toast.text}</div>}
    </div>
  );
}
