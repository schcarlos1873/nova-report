import { memo, useEffect, useState } from 'react';
import { Group, Image as KImage, Line, Rect, Text } from 'react-konva';
import type Konva from 'konva';
import type { ReportElement } from '../../domain/types';
import { PT_TO_MM, mmToPx } from '../../domain/geometry';

interface Props {
  el: ReportElement;
  x: number;
  y: number;
  zoom: number;
  selected: boolean;
  onSelect: (id: string, additive: boolean) => void;
  onDragStart: (e: Konva.KonvaEventObject<DragEvent>) => void;
  onDragMove: (e: Konva.KonvaEventObject<DragEvent>) => void;
  onDragEnd: (e: Konva.KonvaEventObject<DragEvent>) => void;
  onTransformEnd: (e: Konva.KonvaEventObject<Event>) => void;
  onDblClick: (id: string) => void;
}

function useDataImage(src: string) {
  const [img, setImg] = useState<HTMLImageElement>();
  useEffect(() => {
    if (!src.startsWith('data:')) return setImg(undefined);
    const i = new window.Image();
    i.onload = () => setImg(i);
    i.src = src;
  }, [src]);
  return img;
}

function fontStyle(el: ReportElement) {
  const parts = [el.style.italic ? 'italic' : '', el.style.bold ? 'bold' : ''].filter(Boolean);
  return parts.length ? parts.join(' ') : 'normal';
}

/** Renderização WYSIWYG de um elemento: 1 mm na tela = 1 mm no PDF (no zoom 100%). */
function ElementNodeImpl({ el, x, y, zoom, selected, onSelect, onDragStart, onDragMove, onDragEnd, onTransformEnd, onDblClick }: Props) {
  const w = mmToPx(el.width, zoom);
  const h = mmToPx(el.height, zoom);
  const s = el.style;
  const fontPx = mmToPx(s.fontSize * PT_TO_MM, zoom);
  const pad = mmToPx(s.padding, zoom);
  const strokePx = Math.max(s.borderWidth * (96 / 72) * zoom, 0);
  const img = useDataImage(el.type === 'image' ? el.source : '');

  let body: React.ReactNode = null;
  switch (el.type) {
    case 'label':
    case 'expression': {
      const isExpr = el.type === 'expression';
      // no designer, `Fields.x` é exibido como `{x}` (estilo Crystal) para economizar espaço
      const text = isExpr ? `{${el.expression.replace(/Fields\./g, '')}}` : el.text;
      body = (
        <>
          <Rect width={w} height={h} fill={s.background ?? (isExpr ? 'rgba(37,99,235,0.04)' : undefined)}
            stroke={s.borderWidth > 0 ? s.borderColor : undefined} strokeWidth={strokePx} />
          <Text
            x={pad} y={pad} width={Math.max(w - 2 * pad, 1)} height={Math.max(h - 2 * pad, 1)}
            text={text} fontFamily={s.fontFamily} fontSize={fontPx} fontStyle={fontStyle(el)}
            textDecoration={s.underline ? 'underline' : ''} fill={s.color}
            align={s.align === 'justify' ? 'left' : s.align} verticalAlign={s.verticalAlign}
            wrap={el.canGrow ? 'word' : 'none'} ellipsis listening={false} perfectDrawEnabled={false}
          />
        </>
      );
      break;
    }
    case 'image':
      body = img ? (
        <KImage image={img} width={w} height={h} />
      ) : (
        <>
          <Rect width={w} height={h} fill="#F1F5F9" stroke="#94A3B8" dash={[4, 3]} strokeWidth={1} />
          <Text width={w} height={h} text={el.source ? `🖼 ${el.source.split(/[\\/]/).pop()}` : '🖼 imagem'}
            align="center" verticalAlign="middle" fontSize={11} fill="#64748B" listening={false} />
        </>
      );
      break;
    case 'line': {
      const pts = el.direction === 'vertical' ? [0, 0, 0, h] : el.direction === 'diagonal' ? [0, 0, w, h] : [0, 0, w, 0];
      body = (
        <>
          {/* área de clique maior que o traço */}
          <Rect x={-3} y={-3} width={Math.max(w, 1) + 6} height={Math.max(h, 1) + 6} fill="transparent" />
          <Line points={pts} stroke={s.borderColor} strokeWidth={Math.max(strokePx, 1)} />
        </>
      );
      break;
    }
    case 'rectangle':
      body = (
        <Rect width={w} height={h} fill={s.background} cornerRadius={mmToPx(el.radius ?? 0, zoom)}
          stroke={s.borderWidth > 0 ? s.borderColor : '#CBD5E1'} strokeWidth={s.borderWidth > 0 ? strokePx : 1}
          dash={s.borderWidth > 0 ? undefined : [3, 3]} />
      );
      break;
    case 'table': {
      const rowH = mmToPx(Math.max(s.fontSize * PT_TO_MM * 1.9, 4), zoom);
      let cx = 0;
      body = (
        <>
          <Rect width={w} height={h} stroke="#94A3B8" dash={[4, 3]} strokeWidth={1} />
          {el.columns.map((c, i) => {
            const cw = mmToPx(c.width, zoom);
            const node = (
              <Group key={i} x={cx}>
                <Rect width={cw} height={rowH} fill={el.headerBackground ?? '#E2E8F0'} stroke="#CBD5E1" strokeWidth={0.5} />
                <Text x={3} width={cw - 6} height={rowH} text={c.header} fontStyle="bold" fontSize={fontPx}
                  verticalAlign="middle" align={c.align === 'justify' ? 'left' : (c.align ?? 'left')} ellipsis wrap="none" />
                {[1, 2].map((r) => (
                  <Group key={r} y={rowH * r}>
                    <Rect width={cw} height={rowH} fill={r === 2 && el.zebraBackground ? el.zebraBackground : undefined}
                      stroke="#E2E8F0" strokeWidth={0.5} />
                    <Text x={3} width={cw - 6} height={rowH} text={`{${c.expression.replace(/Fields\./g, '')}}`} fontSize={fontPx} fill="#1D4ED8"
                      opacity={r === 1 ? 1 : 0.45} verticalAlign="middle" align={c.align === 'justify' ? 'left' : (c.align ?? 'left')}
                      ellipsis wrap="none" />
                  </Group>
                ))}
              </Group>
            );
            cx += cw;
            return node;
          })}
          <Text y={h - 14} x={4} text={`⟳ ${el.dataSource || 'sem dados'}`} fontSize={10} fill="#64748B" />
        </>
      );
      break;
    }
  }

  return (
    <Group
      id={el.id}
      name="element"
      x={x}
      y={y}
      draggable
      onMouseDown={(e) => {
        e.cancelBubble = true;
        if (!selected || e.evt.shiftKey) onSelect(el.id, e.evt.shiftKey || e.evt.metaKey);
      }}
      onDblClick={() => onDblClick(el.id)}
      onDragStart={onDragStart}
      onDragMove={onDragMove}
      onDragEnd={onDragEnd}
      onTransformEnd={onTransformEnd}
    >
      {/* hitbox explícita: Groups não têm tamanho próprio */}
      <Rect width={Math.max(w, 2)} height={Math.max(h, 2)} fill="transparent" />
      {body}
      {selected && <Rect width={w} height={h} stroke="#2563EB" strokeWidth={1} dash={[2, 2]} listening={false} />}
    </Group>
  );
}

export const ElementNode = memo(ElementNodeImpl);
