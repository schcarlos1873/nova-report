import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './styles.css';

async function bootstrap() {
  if (import.meta.env.DEV && !('__TAURI_INTERNALS__' in window)) {
    const { installDevMock, demoReport, cascadeReport } = await import('./infra/devMock');
    installDevMock();
    const demo = new URLSearchParams(location.search).get('demo');
    if (demo !== null) {
      const { useReport } = await import('./state/reportStore');
      // ?demo=cascata abre o exemplo de filtros em cascata
      if (demo === 'cascata') useReport.getState().load(cascadeReport, '/demo/vendas_cascata.nrep');
      else useReport.getState().load(demoReport, '/demo/vendas_por_regiao.nrep');
    }
    // ?dialog=parameter|dataSource|dataset|image|group|connections e ?mode=preview facilitam inspeção visual automatizada
    const { useUi } = await import('./state/uiStore');
    const q = new URLSearchParams(location.search);
    const dialog = q.get('dialog');
    if (dialog) useUi.getState().set({ dialog: { kind: dialog, target: q.get('target') ?? undefined } as never });
    if (q.get('mode') === 'preview') useUi.getState().set({ mode: 'preview' });
  }
  ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

bootstrap();
