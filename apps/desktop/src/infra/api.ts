// Adaptador IPC: único ponto do frontend que conhece os comandos Tauri.
import { invoke } from '@tauri-apps/api/core';
import type {
  AppInfo,
  ConnectionProfile,
  DatabaseTree,
  DriverDescriptor,
  ExportFormat,
  ParameterValues,
  PreviewResult,
  QueryPreview,
  ReportDefinition,
  ValidationIssue,
} from '../domain/types';

export type Params = Record<string, unknown>;

const call = <T>(cmd: string, args?: Record<string, unknown>) =>
  invoke<T>(cmd, args).catch((e: unknown) => {
    throw new Error(typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e));
  });

export const api = {
  appInfo: () => call<AppInfo>('app_info'),
  newReport: (title?: string) => call<ReportDefinition>('new_report', { title }),
  openReport: (path: string) => call<{ report: ReportDefinition; issues: ValidationIssue[] }>('open_report', { path }),
  saveReport: (path: string, report: ReportDefinition) => call<ValidationIssue[]>('save_report', { path, report }),
  validateReport: (report: ReportDefinition) => call<ValidationIssue[]>('validate_report', { report }),
  checkExpression: (expression: string) => call<void>('check_expression', { expression }),
  preview: (report: ReportDefinition, params: Params, reportPath?: string, refresh = false) =>
    call<PreviewResult>('render_report_preview', { report, params, reportPath, refresh }),
  exportReport: (report: ReportDefinition, params: Params, format: ExportFormat, outputPath: string, reportPath?: string) =>
    call<number>('export_report', { report, params, format, outputPath, reportPath }),
  listDrivers: () => call<DriverDescriptor[]>('list_drivers'),
  listConnections: () => call<ConnectionProfile[]>('list_connections'),
  saveConnection: (profile: ConnectionProfile, password?: string) => call<void>('save_connection', { profile, password }),
  deleteConnection: (alias: string) => call<void>('delete_connection', { alias }),
  testConnection: (profile: ConnectionProfile, password?: string) => call<number>('test_database_connection', { profile, password }),
  schemaTree: (alias: string) => call<DatabaseTree>('fetch_schema_tree', { alias }),
  selectAllSql: (alias: string, schema: string | undefined, table: string) => call<string>('select_all_sql', { alias, schema, table }),
  previewSql: (alias: string, sql: string, params?: Params, limit = 100) => call<QueryPreview>('preview_sql', { alias, sql, params, limit }),
  previewReportQuery: (report: ReportDefinition, queryName: string, params?: Params, reportPath?: string, limit = 100) =>
    call<QueryPreview>('preview_report_query', { report, queryName, params, reportPath, limit }),
  parameterValues: (report: ReportDefinition, name: string, params: Params, reportPath?: string, limit?: number) =>
    call<ParameterValues>('parameter_values', { report, name, params, reportPath, limit }),
  readFileDataUri: (path: string) => call<string>('read_file_data_uri', { path }),
};
