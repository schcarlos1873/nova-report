import { useReport } from '../state/reportStore';

/** Caminhos dentro da pasta do .nrep viram relativos (portabilidade do relatório). */
export function relativeToReport(p: string): string {
  const fp = useReport.getState().filePath;
  if (!fp) return p;
  const dir = fp.replace(/[\\/][^\\/]*$/, '');
  return p.startsWith(dir + '/') || p.startsWith(dir + '\\') ? p.slice(dir.length + 1) : p;
}
