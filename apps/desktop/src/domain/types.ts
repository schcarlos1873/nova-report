// Espelho TypeScript do formato `.nrep` (crate nova-domain). Mantido em sincronia com o serde (camelCase).

export type BandType =
  | 'reportHeader'
  | 'pageHeader'
  | 'groupHeader'
  | 'detail'
  | 'groupFooter'
  | 'pageFooter'
  | 'reportFooter';

export type Align = 'left' | 'center' | 'right' | 'justify';
export type VerticalAlign = 'top' | 'middle' | 'bottom';
export type PaperSize = 'A3' | 'A4' | 'A5' | 'Letter' | 'Legal' | 'Custom';
export type Orientation = 'portrait' | 'landscape';
export type ParamType = 'string' | 'int' | 'float' | 'bool' | 'date' | 'datetime';
export type ProviderKind = 'sqlite' | 'postgres' | 'mysql' | 'mssql' | 'odbc' | 'csv' | 'json' | 'rest' | 'grpc';

export interface Style {
  fontFamily: string;
  fontSize: number;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  color: string;
  background?: string;
  align: Align;
  verticalAlign: VerticalAlign;
  borderWidth: number;
  borderColor: string;
  padding: number;
}

export interface TableColumn {
  header: string;
  expression: string;
  width: number;
  align?: Align;
  format?: string;
  footer?: string;
}

export type ElementKind =
  | { type: 'label'; text: string }
  | { type: 'expression'; expression: string; format?: string }
  | { type: 'image'; source: string; fit?: 'contain' | 'cover' | 'stretch' }
  | { type: 'line'; direction?: 'horizontal' | 'vertical' | 'diagonal' }
  | { type: 'rectangle'; radius?: number }
  | {
      type: 'table';
      dataSource: string;
      columns: TableColumn[];
      repeatHeader?: boolean;
      headerBackground?: string;
      zebraBackground?: string;
    };

export type ElementType = ElementKind['type'];

export type ReportElement = ElementKind & {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  style: Style;
  visible?: string;
  colorExpression?: string;
  backgroundExpression?: string;
  canGrow?: boolean;
};

export interface Band {
  id: string;
  type: BandType;
  height: number;
  group?: string;
  dataSource?: string;
  keepTogether?: boolean;
  pageBreakAfter?: boolean;
  visible?: string;
  elements: ReportElement[];
}

export type QueryKind = 'text' | 'storedProcedure';

export interface ProcedureParam {
  /** Nome na procedure (`@param1`). */
  name: string;
  /** `:parametro` do relatório ou literal SQL. */
  value: string;
  passNull?: boolean;
}

export interface QueryDef {
  name: string;
  sql: string;
  /** Ausente = `text`. */
  kind?: QueryKind;
  procedure?: string;
  procedureParams?: ProcedureParam[];
}

export type ImageKind = 'file' | 'url' | 'embedded' | 'database';

export interface ImageResource {
  name: string;
  kind: ImageKind;
  /** file: caminho; url: http(s); embedded: data URI. */
  value?: string;
  mimeType?: string;
  /** database: query e campo lidos a cada registro. */
  query?: string;
  field?: string;
}

/** `source` de um elemento imagem que aponta para `images`. */
export const IMAGE_REF_PREFIX = 'image:';

export interface DataSourceDef {
  id: string;
  provider: ProviderKind;
  connectionRef?: string;
  connectionStringRef?: string;
  options?: Record<string, string>;
  queries: QueryDef[];
}

export interface AvailableValues {
  /** Query do relatório que lista os valores. */
  query: string;
  valueField: string;
  labelField?: string;
}

export interface ParameterDef {
  name: string;
  type: ParamType;
  label: string;
  required: boolean;
  defaultValue?: unknown;
  /** Vários valores: `:nome` vira lista na SQL (`IN (:nome)`) e `Params.nome` um array. */
  multiValue?: boolean;
  /** Filtro em cascata: a lista recarrega (e o valor é limpo) quando um destes muda. */
  dependsOn?: string[];
  availableValues?: AvailableValues;
}

export interface ParameterOption {
  value: string | number | boolean;
  label: string;
}

export interface ParameterValues {
  values: ParameterOption[];
  truncated: boolean;
  /** Rótulos dos parâmetros de `dependsOn` ainda vazios. */
  waitingFor: string[];
}

export interface GroupDef {
  name: string;
  expression: string;
  pageBreakBetween?: boolean;
}

export interface ReportMeta {
  title: string;
  description?: string;
  author?: string;
  paperSize: PaperSize;
  customSize?: { width: number; height: number };
  orientation: Orientation;
  margins: { top: number; bottom: number; left: number; right: number };
}

export interface ReportDefinition {
  $schema: string;
  schemaVersion: string;
  meta: ReportMeta;
  dataSources: DataSourceDef[];
  parameters: ParameterDef[];
  images?: ImageResource[];
  groups?: GroupDef[];
  bands: Band[];
}

export interface ValidationIssue {
  severity: 'error' | 'warning';
  path: string;
  message: string;
}

// ---- conexões

export interface ConnectionProfile {
  alias: string;
  provider: ProviderKind;
  host?: string;
  port?: number;
  database?: string;
  username?: string;
  path?: string;
  connectionString?: string;
  useTls: boolean;
  options?: Record<string, string>;
}

export interface DriverDescriptor {
  id: ProviderKind;
  name: string;
  tier: string;
  defaultPort?: number;
  fields: string[];
  supportsIntrospection: boolean;
}

export interface ColumnMetadata {
  name: string;
  nativeType: string;
  arrowType: string;
  nullable: boolean;
  isPrimaryKey: boolean;
  isForeignKey: boolean;
  defaultValue?: string;
}

export interface TableMetadata {
  schema: string;
  name: string;
  kind: 'TABLE' | 'VIEW';
  comment?: string;
  columns: ColumnMetadata[];
}

export interface DatabaseTree {
  databaseName: string;
  schemas: { name: string; tables: TableMetadata[]; views: TableMetadata[]; procedures: { schema: string; name: string }[] }[];
}

export type CellValue = string | number | boolean | null;

export interface QueryPreview {
  columns: { name: string; dataType: string }[];
  rows: CellValue[][];
  elapsedMs: number;
}

export interface RenderStats {
  dataMs: number;
  layoutMs: number;
  compileMs: number;
  totalMs: number;
  dataCached: boolean;
  rows: Record<string, number>;
}

export interface PreviewResult {
  pageCount: number;
  pagesSvg: string[];
  pageWidthMm: number;
  pageHeightMm: number;
  renderTimeMs: number;
  stats: RenderStats;
  warnings: string[];
}

export type ExportFormat = 'pdf' | 'xlsx' | 'csv' | 'typst';

export interface AppInfo {
  version: string;
  fonts: string[];
  drivers: DriverDescriptor[];
}
