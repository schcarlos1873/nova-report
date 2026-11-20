// Modo navegador (somente `vite dev` fora do Tauri): simula o backend via mockIPC para
// desenvolver/inspecionar a UI sem o processo Rust. Nunca incluído no build de produção.
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import demo from '../../../../examples/vendas_por_regiao.nrep?raw';
import { blankReport } from '../domain/factory';
import type { ReportDefinition } from '../domain/types';

export const demoReport = JSON.parse(demo) as ReportDefinition;

const FIELDS = ['regiao', 'cliente', 'cidade', 'pedido', 'data', 'produto', 'quantidade', 'valor_total', 'status'];

export function installDevMock() {
  mockWindows('main');
  mockIPC((cmd) => {
    switch (cmd) {
      case 'app_info':
        return {
          version: '0.1.0-dev',
          fonts: ['Arial', 'Helvetica', 'Inter', 'Libertinus Serif', 'DejaVu Sans Mono'],
          drivers: [
            { id: 'sqlite', name: 'SQLite', tier: '1-nativo', fields: ['path'], supportsIntrospection: true },
            { id: 'postgres', name: 'PostgreSQL', tier: '1-nativo', defaultPort: 5432, fields: ['host', 'port', 'database', 'username', 'password', 'useTls', 'connectionString'], supportsIntrospection: true },
          ],
        };
      case 'list_connections':
        return [{ alias: 'ERP_LOCAL', provider: 'sqlite', path: 'vendas.db', useTls: false }];
      case 'new_report':
        return blankReport();
      case 'validate_report':
        return [];
      case 'check_expression':
        return null;
      case 'preview_report_query':
        return { columns: FIELDS.map((name) => ({ name, dataType: 'str' })), rows: [], elapsedMs: 1 };
      case 'fetch_schema_tree':
        return {
          databaseName: 'vendas.db',
          schemas: [{ name: 'main', tables: [{ schema: 'main', name: 'pedidos', kind: 'TABLE', columns: [] }], views: [], procedures: [] }],
        };
      case 'render_report_preview':
        return {
          pageCount: 1,
          pagesSvg: ['<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 595 842" width="595pt" height="842pt"><text x="40" y="60">Pré-visualização simulada (modo navegador)</text></svg>'],
          pageWidthMm: 210,
          pageHeightMm: 297,
          renderTimeMs: 1,
          stats: { dataMs: 0, layoutMs: 0, compileMs: 0, totalMs: 1, dataCached: false, rows: {} },
          warnings: [],
        };
      default:
        return null;
    }
  });
}
