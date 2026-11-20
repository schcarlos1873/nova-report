// Ordem e propagação dos parâmetros em cascata (`dependsOn`).
import type { ParameterDef } from '../../domain/types';

/** Parâmetros em ordem de dependência (pais antes dos filhos). Ciclos são ignorados — a validação já os acusa. */
export function topoOrder(params: ParameterDef[]): ParameterDef[] {
  const byName = new Map(params.map((p) => [p.name, p]));
  const done = new Set<string>();
  const visiting = new Set<string>();
  const out: ParameterDef[] = [];
  const visit = (p: ParameterDef) => {
    if (done.has(p.name) || visiting.has(p.name)) return;
    visiting.add(p.name);
    for (const d of p.dependsOn ?? []) {
      const dep = byName.get(d);
      if (dep) visit(dep);
    }
    visiting.delete(p.name);
    done.add(p.name);
    out.push(p);
  };
  params.forEach(visit);
  return out;
}

/** Todos os parâmetros afetados (direta ou indiretamente) por uma mudança em `name`, em ordem. */
export function dependentsOf(params: ParameterDef[], name: string): ParameterDef[] {
  const hit = new Set([name]);
  const out: ParameterDef[] = [];
  for (const p of topoOrder(params)) {
    if (p.name !== name && (p.dependsOn ?? []).some((d) => hit.has(d))) {
      hit.add(p.name);
      out.push(p);
    }
  }
  return out;
}

export const emptyValue = (p: ParameterDef) => (p.multiValue ? [] : null);

/** Mantém só os valores que existem na nova lista (o que sobrar inválido é limpo). */
export function keepValid(p: ParameterDef, value: unknown, allowed: unknown[]): unknown {
  const ok = new Set(allowed.map((v) => JSON.stringify(v)));
  if (p.multiValue) return Array.isArray(value) ? value.filter((v) => ok.has(JSON.stringify(v))) : [];
  return value !== null && value !== undefined && ok.has(JSON.stringify(value)) ? value : null;
}

/** Valor padrão do `.nrep` como lista de candidatos (array ou texto separado por vírgula). */
export function defaultCandidates(p: ParameterDef): unknown[] {
  const d = p.defaultValue;
  if (d === undefined || d === null || d === '') return [];
  if (Array.isArray(d)) return d;
  return typeof d === 'string' && p.multiValue ? d.split(',').map((x) => x.trim()) : [d];
}
