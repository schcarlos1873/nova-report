import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Group, Layer, Line, Rect, Shape, Stage, Text, Transformer } from 'react-konva';
import type Konva from 'konva';
import type { Band, ReportElement } from '../../domain/types';
import { BAND_COLORS, BAND_LABELS, bandTitle, clamp, printableWidth, pxPerMm, pxToMm, round, sortBands } from '../../domain/geometry';
import { draftElement, moveElementToBand, useReport } from '../../state/reportStore';
import { useUi } from '../../state/uiStore';
import { ElementNode } from './ElementNode';
import { snapRect, type Guide } from './guides';
import { draggedPayload, elementForPayload, endDrag, resolveFieldDrop, tableAt } from './dnd';

const RULER = 22;
const STRIP = 20;
const PAD = 24;
const ORIGIN_X = RULER + PAD;
const SNAP_PX = 5;

interface BandLayout {
  band: Band;
  stripTop: number;
  areaTop: number;
  areaH: number;
}

/** Destino destacado durante um arraste vindo do toolbox / painel de dados. */
interface DropTarget {
  bandIdx: number;
  /** Retângulo (mm, relativo à banda) do elemento destino, quando houver. */
  table?: { x: number; y: number; w: number; h: number };
  hint: string;
}

export function DesignerCanvas() {
  const report = useReport((s) => s.report);
  const update = useReport((s) => s.update);
  const { zoom, gridMm, snapToGrid, showGrid, selectedIds, selectedBandId, select, toggle } = useUi();
  const stageRef = useRef<Konva.Stage>(null);
  const trRef = useRef<Konva.Transformer>(null);
  const [guides, setGuides] = useState<{ bandIdx: number; guides: Guide[] }>();
  const [resizing, setResizing] = useState<{ bandId: string; heightMm: number }>();
  const [marquee, setMarquee] = useState<{ x0: number; y0: number; x1: number; y1: number }>();
  const [dropTarget, setDropTarget] = useState<DropTarget>();
  const dragStart = useRef<Record<string, { x: number; y: number }>>({});

  const ppm = pxPerMm(zoom);
  const widthMm = printableWidth(report);
  const widthPx = widthMm * ppm;
  const bands = useMemo(() => sortBands(report), [report]);

  const layout: BandLayout[] = useMemo(() => {
    let y = RULER + 12;
    return bands.map((band) => {
      const h = resizing?.bandId === band.id ? resizing.heightMm : band.height;
      const l = { band, stripTop: y, areaTop: y + STRIP, areaH: h * ppm };
      y += STRIP + l.areaH + 4;
      return l;
    });
  }, [bands, ppm, resizing]);

  const stageH = (layout.at(-1)?.areaTop ?? 0) + (layout.at(-1)?.areaH ?? 0) + 60;
  const stageW = ORIGIN_X + widthPx + PAD * 2;

  const bandAt = useCallback(
    (yPx: number) => layout.findIndex((l) => yPx >= l.stripTop && yPx < l.areaTop + l.areaH + 4),
    [layout],
  );

  // ---- Transformer acompanha a seleção
  useEffect(() => {
    const stage = stageRef.current;
    const tr = trRef.current;
    if (!stage || !tr) return;
    const nodes = selectedIds.map((id) => stage.findOne(`#${id}`)).filter((n): n is Konva.Node => !!n);
    tr.nodes(nodes);
    tr.getLayer()?.batchDraw();
  }, [selectedIds, report, layout]);

  const elementPos = (l: BandLayout, el: ReportElement) => ({ x: ORIGIN_X + el.x * ppm, y: l.areaTop + el.y * ppm });

  // ---- arrastar elementos (com snap, guias e múltipla seleção)
  const onDragStart = (e: Konva.KonvaEventObject<DragEvent>) => {
    const id = e.target.id();
    if (!selectedIds.includes(id)) select([id]);
    const ids = selectedIds.includes(id) ? selectedIds : [id];
    dragStart.current = {};
    for (const sid of ids) {
      const n = stageRef.current?.findOne(`#${sid}`);
      if (n) dragStart.current[sid] = { x: n.x(), y: n.y() };
    }
  };

  const onDragMove = (e: Konva.KonvaEventObject<DragEvent>) => {
    const node = e.target;
    const id = node.id();
    const bi = bandAt(node.y());
    const l = layout[bi];
    const found = l?.band.elements.find((x) => x.id === id) ?? report.bands.flatMap((b) => b.elements).find((x) => x.id === id);
    if (l && found) {
      const others = l.band.elements
        .filter((o) => o.id !== id && !selectedIds.includes(o.id))
        .map((o) => ({ x: o.x, y: o.y, w: o.width, h: o.height }));
      const moving = { x: pxToMm(node.x() - ORIGIN_X, zoom), y: pxToMm(node.y() - l.areaTop, zoom), w: found.width, h: found.height };
      const r = snapRect(moving, others, widthMm, l.band.height, SNAP_PX / ppm, snapToGrid ? gridMm : 0);
      node.position({ x: ORIGIN_X + r.x * ppm, y: l.areaTop + r.y * ppm });
      setGuides({ bandIdx: bi, guides: r.guides });
    }
    // demais selecionados acompanham o deslocamento
    const start = dragStart.current[id];
    if (start) {
      const dx = node.x() - start.x;
      const dy = node.y() - start.y;
      for (const [sid, p] of Object.entries(dragStart.current)) {
        if (sid !== id) stageRef.current?.findOne(`#${sid}`)?.position({ x: p.x + dx, y: p.y + dy });
      }
    }
  };

  const onDragEnd = () => {
    setGuides(undefined);
    const stage = stageRef.current;
    if (!stage) return;
    const moves = Object.keys(dragStart.current).map((sid) => {
      const n = stage.findOne(`#${sid}`)!;
      const bi = Math.max(0, bandAt(n.y()));
      const l = layout[bi];
      return {
        id: sid,
        bandId: l.band.id,
        x: round(clamp(pxToMm(n.x() - ORIGIN_X, zoom), 0, widthMm), 0.1),
        y: round(Math.max(0, pxToMm(n.y() - l.areaTop, zoom)), 0.1),
      };
    });
    update((d) => {
      for (const m of moves) moveElementToBand(d, m.id, m.bandId, m.x, m.y);
    });
    dragStart.current = {};
  };

  const onTransformEnd = (e: Konva.KonvaEventObject<Event>) => {
    const node = e.target;
    const id = node.id();
    const sx = node.scaleX();
    const sy = node.scaleY();
    node.scale({ x: 1, y: 1 });
    const l = layout[Math.max(0, bandAt(node.y()))];
    update((d) => {
      const el = draftElement(d, id);
      if (!el) return;
      const g = snapToGrid ? gridMm : 0.1;
      el.width = Math.max(round(el.width * sx, g), el.type === 'line' ? 0 : 1);
      el.height = el.type === 'line' && el.direction !== 'vertical' && el.direction !== 'diagonal' ? 0 : Math.max(round(el.height * sy, g), 1);
      el.x = round(Math.max(0, pxToMm(node.x() - ORIGIN_X, zoom)), 0.1);
      el.y = round(Math.max(0, pxToMm(node.y() - l.areaTop, zoom)), 0.1);
    });
  };

  // ---- seleção por laço (marquee) e clique no fundo
  const onStageMouseDown = (e: Konva.KonvaEventObject<MouseEvent>) => {
    if (e.target.getParent()?.className === 'Transformer') return;
    const pos = e.target.getStage()?.getPointerPosition();
    if (!pos) return;
    const bi = bandAt(pos.y);
    if (!e.evt.shiftKey) select([], bi >= 0 ? layout[bi].band.id : undefined);
    setMarquee({ x0: pos.x, y0: pos.y, x1: pos.x, y1: pos.y });
  };

  const onStageMouseMove = () => {
    if (!marquee) return;
    const pos = stageRef.current?.getPointerPosition();
    if (pos) setMarquee({ ...marquee, x1: pos.x, y1: pos.y });
  };

  const onStageMouseUp = () => {
    if (!marquee) return;
    const [x0, x1] = [Math.min(marquee.x0, marquee.x1), Math.max(marquee.x0, marquee.x1)];
    const [y0, y1] = [Math.min(marquee.y0, marquee.y1), Math.max(marquee.y0, marquee.y1)];
    setMarquee(undefined);
    if (x1 - x0 < 4 && y1 - y0 < 4) return;
    const hits: string[] = [];
    for (const l of layout) {
      for (const el of l.band.elements) {
        const p = elementPos(l, el);
        const [w, h] = [Math.max(el.width * ppm, 2), Math.max(el.height * ppm, 2)];
        if (p.x < x1 && p.x + w > x0 && p.y < y1 && p.y + h > y0) hits.push(el.id);
      }
    }
    select([...new Set([...useUi.getState().selectedIds, ...hits])]);
  };

  // ---- soltar itens do toolbox / campos
  /** Converte o ponteiro do evento DOM em banda + posição (mm) já com encaixe. */
  const dropPoint = (e: React.DragEvent) => {
    const stage = stageRef.current;
    if (!stage) return undefined;
    stage.setPointersPositions(e.nativeEvent);
    const pos = stage.getPointerPosition();
    if (!pos) return undefined;
    const bi = bandAt(pos.y);
    if (bi < 0) return undefined;
    const l = layout[bi];
    const g = snapToGrid ? gridMm : 0.5;
    const rawX = clamp(pxToMm(pos.x - ORIGIN_X, zoom), 0, widthMm);
    const rawY = clamp(pxToMm(pos.y - l.areaTop, zoom), 0, l.band.height);
    return { bi, l, rawX, rawY, x: round(Math.min(rawX, widthMm - 5), g), y: round(rawY, g) };
  };

  const onDragOver = (e: React.DragEvent) => {
    const payload = draggedPayload();
    // sem payload em memória (ex.: arquivo do sistema) o canvas não aceita o drop
    if (!payload) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'copy';
    const pt = dropPoint(e);
    if (!pt) return setDropTarget(undefined);
    let target: DropTarget;
    if (payload.field) {
      const r = resolveFieldDrop(report, pt.l.band, { ...payload, field: payload.field }, pt.x, pt.y, widthMm, e.altKey);
      const t = r.tableColumn && tableAt(pt.l.band, pt.rawX, pt.rawY);
      target = { bandIdx: pt.bi, hint: r.hint, table: t ? { x: t.x, y: t.y, w: t.width, h: t.height } : undefined };
    } else {
      target = { bandIdx: pt.bi, hint: `${payload.label ? `${payload.label} → ` : 'Inserir em '}${BAND_LABELS[pt.l.band.type]}` };
    }
    if (target.bandIdx !== dropTarget?.bandIdx || target.hint !== dropTarget.hint) setDropTarget(target);
  };

  const onDragLeave = (e: React.DragEvent) => {
    if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setDropTarget(undefined);
  };

  const onDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDropTarget(undefined);
    const payload = draggedPayload(e);
    endDrag();
    const pt = payload && dropPoint(e);
    if (!payload || !pt) return;
    const r = useReport.getState().report;
    const band = pt.l.band;
    if (payload.field) {
      const res = resolveFieldDrop(r, band, { ...payload, field: payload.field }, pt.x, pt.y, widthMm, e.altKey);
      update((d) => {
        for (const ins of res.inserts) d.bands.find((b) => b.id === ins.bandId)?.elements.push(ins.element);
        if (res.tableColumn) {
          const t = draftElement(d, res.tableColumn.tableId);
          if (t?.type === 'table') {
            t.columns.push(res.tableColumn.column);
            if (!t.dataSource && payload.query) t.dataSource = payload.query;
            t.width = Math.max(t.width, t.columns.reduce((s, c) => s + c.width, 0));
          }
        }
        if (res.bindDetail) {
          const b = d.bands.find((x) => x.id === res.bindDetail!.bandId);
          if (b) b.dataSource = res.bindDetail.query;
        }
      });
      if (res.tableColumn) select([res.tableColumn.tableId], band.id);
      // só o elemento solto: o rótulo/valor complementar fica em outra banda
      else if (res.inserts[0]) select([res.inserts[0].element.id], band.id);
      if (res.bindDetail) useUi.getState().notify(`Detalhe associado à query ${res.bindDetail.query}`);
    } else {
      const el = elementForPayload(r, payload, pt.x, pt.y, widthMm);
      if (!el) return;
      update((d) => void d.bands.find((b) => b.id === band.id)?.elements.push(el));
      select([el.id], band.id);
    }
  };

  return (
    <div
      className="designer-scroll"
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
    >
      <Stage
        ref={stageRef}
        width={stageW}
        height={stageH}
        onMouseDown={onStageMouseDown}
        onMouseMove={onStageMouseMove}
        onMouseUp={onStageMouseUp}
      >
        {/* bandas, grade e réguas */}
        <Layer listening={true}>
          <Ruler widthPx={widthPx} ppm={ppm} />
          {layout.map((l) => (
            <Group key={l.band.id}>
              <Rect
                x={ORIGIN_X} y={l.stripTop} width={widthPx} height={STRIP}
                fill={selectedBandId === l.band.id ? BAND_COLORS[l.band.type] : '#F1F5F9'}
                stroke="#E2E8F0" strokeWidth={1}
              />
              <Rect x={ORIGIN_X} y={l.stripTop} width={4} height={STRIP} fill={BAND_COLORS[l.band.type]} listening={false} />
              <Text
                x={ORIGIN_X + 10} y={l.stripTop + 5} text={`${bandTitle(l.band)}  ·  ${round(resizing?.bandId === l.band.id ? resizing.heightMm : l.band.height, 0.1)} mm`}
                fontSize={11} fontStyle="bold" fill={selectedBandId === l.band.id ? '#FFFFFF' : '#334155'} listening={false}
              />
              <Rect x={ORIGIN_X} y={l.areaTop} width={widthPx} height={l.areaH} fill="#FFFFFF" stroke="#CBD5E1" strokeWidth={1}
                shadowColor="#0F172A" shadowOpacity={0.05} shadowBlur={4} />
              {showGrid && <Grid x={ORIGIN_X} y={l.areaTop} w={widthPx} h={l.areaH} ppm={ppm} gridMm={gridMm} />}
              <VerticalRuler y={l.areaTop} h={l.areaH} ppm={ppm} />
              {/* alça de redimensionamento da banda */}
              <Rect
                x={ORIGIN_X} y={l.areaTop + l.areaH - 3} width={widthPx} height={6} fill="transparent"
                draggable
                dragBoundFunc={(p) => ({ x: ORIGIN_X, y: Math.max(p.y, l.areaTop + 2 * ppm - 3) })}
                onMouseEnter={(e) => { e.target.getStage()!.container().style.cursor = 'ns-resize'; }}
                onMouseLeave={(e) => { e.target.getStage()!.container().style.cursor = 'default'; }}
                onMouseDown={(e) => { e.cancelBubble = true; }}
                onDragMove={(e) => {
                  const hMm = (e.target.y() + 3 - l.areaTop) / ppm;
                  setResizing({ bandId: l.band.id, heightMm: snapToGrid ? Math.max(gridMm, Math.round(hMm / gridMm) * gridMm) : round(hMm, 0.5) });
                }}
                onDragEnd={(e) => {
                  const h = resizing?.heightMm ?? l.band.height;
                  e.target.y(l.areaTop + h * ppm - 3);
                  setResizing(undefined);
                  update((d) => {
                    const b = d.bands.find((x) => x.id === l.band.id);
                    if (b) b.height = Math.max(1, round(h, 0.1));
                  });
                }}
              />
            </Group>
          ))}
        </Layer>

        {/* elementos */}
        <Layer>
          {layout.map((l) => (
            <Group key={l.band.id}>
              {l.band.elements.map((el) => {
                const p = elementPos(l, el);
                return (
                  <ElementNode
                    key={el.id}
                    el={el}
                    x={p.x}
                    y={p.y}
                    zoom={zoom}
                    selected={selectedIds.includes(el.id)}
                    onSelect={(id, additive) => (additive ? toggle(id) : select([id], l.band.id))}
                    onDragStart={onDragStart}
                    onDragMove={onDragMove}
                    onDragEnd={onDragEnd}
                    onTransformEnd={onTransformEnd}
                    onDblClick={(id) => select([id], l.band.id)}
                  />
                );
              })}
            </Group>
          ))}
        </Layer>

        {/* overlay: guias, transformer, laço */}
        <Layer>
          {guides?.guides.map((g, i) => {
            const l = layout[guides.bandIdx];
            return g.orientation === 'v' ? (
              <Line key={i} points={[ORIGIN_X + g.pos * ppm, l.areaTop - 6, ORIGIN_X + g.pos * ppm, l.areaTop + l.areaH + 6]}
                stroke="#3B82F6" strokeWidth={1} dash={[4, 2]} listening={false} />
            ) : (
              <Line key={i} points={[ORIGIN_X - 6, l.areaTop + g.pos * ppm, ORIGIN_X + widthPx + 6, l.areaTop + g.pos * ppm]}
                stroke="#3B82F6" strokeWidth={1} dash={[4, 2]} listening={false} />
            );
          })}
          {dropTarget && (() => {
            const l = layout[dropTarget.bandIdx];
            if (!l) return null;
            const t = dropTarget.table;
            const box = t
              ? { x: ORIGIN_X + t.x * ppm, y: l.areaTop + t.y * ppm, w: t.w * ppm, h: t.h * ppm }
              : { x: ORIGIN_X, y: l.areaTop, w: widthPx, h: l.areaH };
            return (
              <Group listening={false}>
                <Rect x={box.x} y={box.y} width={box.w} height={box.h} fill="rgba(37,99,235,0.08)" stroke="#2563EB" strokeWidth={2} dash={[6, 3]} />
                <Text x={ORIGIN_X} y={l.stripTop + 5} width={widthPx - 10} align="right" text={`⤓ ${dropTarget.hint}`}
                  fontSize={11} fontStyle="bold" fill={selectedBandId === l.band.id ? '#FFFFFF' : '#1D4ED8'} />
              </Group>
            );
          })()}
          <Transformer
            ref={trRef}
            rotateEnabled={false}
            keepRatio={false}
            ignoreStroke
            anchorSize={7}
            borderStroke="#2563EB"
            anchorStroke="#2563EB"
            boundBoxFunc={(oldBox, newBox) => (newBox.width < 4 || newBox.height < 2 ? oldBox : newBox)}
          />
          {marquee && (
            <Rect
              x={Math.min(marquee.x0, marquee.x1)} y={Math.min(marquee.y0, marquee.y1)}
              width={Math.abs(marquee.x1 - marquee.x0)} height={Math.abs(marquee.y1 - marquee.y0)}
              fill="rgba(37,99,235,0.08)" stroke="#2563EB" strokeWidth={1} dash={[3, 3]} listening={false}
            />
          )}
        </Layer>
      </Stage>
    </div>
  );
}

/** Grade desenhada em um único Shape (rápido mesmo com zoom alto). */
function Grid({ x, y, w, h, ppm, gridMm }: { x: number; y: number; w: number; h: number; ppm: number; gridMm: number }) {
  return (
    <Shape
      listening={false}
      sceneFunc={(ctx) => {
        const step = gridMm * ppm >= 6 ? gridMm : 5;
        const c = ctx._context;
        c.save();
        c.lineWidth = 1;
        for (let mm = step; mm * ppm < w; mm += step) {
          c.strokeStyle = Math.round(mm) % 10 === 0 && Math.abs(mm - Math.round(mm)) < 1e-6 ? '#E2E8F0' : '#F5F7FA';
          const px = Math.round(x + mm * ppm) + 0.5;
          c.beginPath();
          c.moveTo(px, y);
          c.lineTo(px, y + h);
          c.stroke();
        }
        for (let mm = step; mm * ppm < h; mm += step) {
          c.strokeStyle = Math.round(mm) % 10 === 0 && Math.abs(mm - Math.round(mm)) < 1e-6 ? '#E2E8F0' : '#F5F7FA';
          const py = Math.round(y + mm * ppm) + 0.5;
          c.beginPath();
          c.moveTo(x, py);
          c.lineTo(x + w, py);
          c.stroke();
        }
        c.restore();
      }}
    />
  );
}

/** Régua horizontal milimétrica. */
function Ruler({ widthPx, ppm }: { widthPx: number; ppm: number }) {
  return (
    <Shape
      listening={false}
      sceneFunc={(ctx) => {
        const c = ctx._context;
        c.save();
        c.fillStyle = '#F8FAFC';
        c.fillRect(0, 0, ORIGIN_X + widthPx + PAD, RULER);
        c.strokeStyle = '#94A3B8';
        c.fillStyle = '#475569';
        c.font = '9px sans-serif';
        c.lineWidth = 1;
        const every = ppm < 2.5 ? 2 : 1;
        for (let mm = 0; mm * ppm <= widthPx + 0.5; mm += every) {
          const px = Math.round(ORIGIN_X + mm * ppm) + 0.5;
          const len = mm % 10 === 0 ? 10 : mm % 5 === 0 ? 6 : 3;
          c.beginPath();
          c.moveTo(px, RULER);
          c.lineTo(px, RULER - len);
          c.stroke();
          if (mm % 10 === 0) c.fillText(String(mm), px + 2, 10);
        }
        c.restore();
      }}
    />
  );
}

/** Régua vertical por banda (0 no topo de cada banda). */
function VerticalRuler({ y, h, ppm }: { y: number; h: number; ppm: number }) {
  return (
    <Shape
      listening={false}
      sceneFunc={(ctx) => {
        const c = ctx._context;
        c.save();
        c.fillStyle = '#F8FAFC';
        c.fillRect(0, y, RULER, h);
        c.strokeStyle = '#94A3B8';
        c.fillStyle = '#475569';
        c.font = '9px sans-serif';
        for (let mm = 0; mm * ppm <= h + 0.5; mm += 1) {
          const py = Math.round(y + mm * ppm) + 0.5;
          const len = mm % 10 === 0 ? 10 : mm % 5 === 0 ? 6 : 3;
          c.beginPath();
          c.moveTo(RULER, py);
          c.lineTo(RULER - len, py);
          c.stroke();
          if (mm % 10 === 0 && mm > 0) c.fillText(String(mm), 2, py - 2);
        }
        c.restore();
      }}
    />
  );
}

