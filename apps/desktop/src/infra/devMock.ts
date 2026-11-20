// Modo navegador (somente `vite dev` fora do Tauri): simula o backend via mockIPC para
// desenvolver/inspecionar a UI sem o processo Rust. Nunca incluído no build de produção.
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import demo from '../../../../examples/vendas_por_regiao.nrep?raw';
import cascata from '../../../../examples/vendas_cascata.nrep?raw';
import { blankReport } from '../domain/factory';
import type { ReportDefinition } from '../domain/types';

export const demoReport = JSON.parse(demo) as ReportDefinition;
export const cascadeReport = JSON.parse(cascata) as ReportDefinition;

// dados mínimos do vendas.db para simular a cascata Região → Cliente
const REGIOES = [{ value: 4, label: 'Centro-Oeste' }, { value: 3, label: 'Nordeste' }, { value: 1, label: 'Sudeste' }, { value: 2, label: 'Sul' }];
const CLIENTES = [
  { value: 1, label: 'Acme Indústria Ltda (São Paulo)', regiao: 1 }, { value: 2, label: 'Beta Comércio S.A. (Rio de Janeiro)', regiao: 1 },
  { value: 3, label: 'Casa & Cia (Curitiba)', regiao: 2 }, { value: 4, label: 'Delta Tecnologia (Porto Alegre)', regiao: 2 },
  { value: 5, label: 'Esperança Alimentos (Recife)', regiao: 3 }, { value: 6, label: 'Farol Distribuidora (Salvador)', regiao: 3 },
  { value: 7, label: 'Goiás Agro (Goiânia)', regiao: 4 }, { value: 8, label: 'Horizonte Serviços (Belo Horizonte)', regiao: 1 },
  { value: 9, label: 'Ipê Móveis (Florianópolis)', regiao: 2 }, { value: 10, label: 'Jangada Pescados (Fortaleza)', regiao: 3 },
];

const FIELDS = ['regiao', 'cliente', 'cidade', 'pedido', 'data', 'produto', 'quantidade', 'valor_total', 'status'];

export function installDevMock() {
  mockWindows('main');
  mockIPC((cmd, args) => {
    switch (cmd) {
      case 'parameter_values': {
        const a = args as { name: string; params: Record<string, unknown> };
        if (a.name === 'regioes') return { values: REGIOES, truncated: false, waitingFor: [] };
        const sel = a.params.regioes as number[] | undefined;
        const regs = sel === undefined ? [1, 2, 3, 4] : sel;
        if (!regs.length) return { values: [], truncated: false, waitingFor: ['Regiões'] };
        return { values: CLIENTES.filter((c) => regs.includes(c.regiao)).map(({ value, label }) => ({ value, label })), truncated: false, waitingFor: [] };
      }
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
