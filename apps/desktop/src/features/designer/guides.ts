// Guias inteligentes: alinhamento magnético por bordas/centros de elementos vizinhos e da banda.

export interface RectMm {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Guide {
  orientation: 'v' | 'h';
  /** posição em mm, relativa à banda */
  pos: number;
}

interface Candidate {
  delta: number;
  pos: number;
}

function best(points: number[], targets: number[], threshold: number): Candidate | undefined {
  let found: Candidate | undefined;
  for (const p of points) {
    for (const t of targets) {
      const d = t - p;
      if (Math.abs(d) <= threshold && (!found || Math.abs(d) < Math.abs(found.delta))) found = { delta: d, pos: t };
    }
  }
  return found;
}

/**
 * Ajusta o retângulo em movimento: guias têm prioridade sobre a grade.
 * `thresholdMm` define a distância de atração.
 */
export function snapRect(
  moving: RectMm,
  others: RectMm[],
  bandWidth: number,
  bandHeight: number,
  thresholdMm: number,
  grid: number,
): { x: number; y: number; guides: Guide[] } {
  const vTargets = [0, bandWidth / 2, bandWidth, ...others.flatMap((o) => [o.x, o.x + o.w / 2, o.x + o.w])];
  const hTargets = [0, bandHeight / 2, bandHeight, ...others.flatMap((o) => [o.y, o.y + o.h / 2, o.y + o.h])];
  const v = best([moving.x, moving.x + moving.w / 2, moving.x + moving.w], vTargets, thresholdMm);
  const h = best([moving.y, moving.y + moving.h / 2, moving.y + moving.h], hTargets, thresholdMm);
  const guides: Guide[] = [];
  let { x, y } = moving;
  if (v) {
    x += v.delta;
    guides.push({ orientation: 'v', pos: v.pos });
  } else if (grid > 0) {
    x = Math.round(x / grid) * grid;
  }
  if (h) {
    y += h.delta;
    guides.push({ orientation: 'h', pos: h.pos });
  } else if (grid > 0) {
    y = Math.round(y / grid) * grid;
  }
  return { x, y, guides };
}
