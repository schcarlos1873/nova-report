# Transpilador Typst & Visualizador Paginado Tauri (Tauri + React)

Este documento especifica a transformação do modelo intermediário renderizado para código Typst, a compilação de PDF/SVG em memória e a integração IPC com o aplicativo Tauri desktop.

---

## 1. Modelo Intermediário Avaliado (`RenderedBandTree`)

Toda banda processada pelo engine gera uma estrutura pronta para conversão visual:

```rust
pub struct PageConfig {
    pub width_mm: f64,
    pub height_mm: f64,
    pub margin_top_mm: f64,
    pub margin_bottom_mm: f64,
    pub margin_left_mm: f64,
    pub margin_right_mm: f64,
}

pub enum RenderedElement {
    TextBox {
        x_mm: f64,
        y_mm: f64,
        width_mm: f64,
        height_mm: f64,
        text: String,
        font_family: String,
        font_size_pt: f64,
        bold: bool,
        color_hex: String,
        bg_color_hex: Option<String>,
        align: String,
        can_grow: bool,
    },
    Table {
        columns_width_mm: Vec<f64>,
        headers: Vec<Vec<TableCell>>,
        rows: Vec<Vec<TableCell>>,
        footers: Vec<Vec<TableCell>>,
        repeat_header: bool,
    },
    Image {
        x_mm: f64,
        y_mm: f64,
        width_mm: f64,
        height_mm: f64,
        virtual_path: String,
    },
}

pub struct TableCell {
    pub text: String,
    pub colspan: usize,
    pub rowspan: usize,
    pub align: String,
    pub bold: bool,
    pub bg_color_hex: Option<String>,
    pub text_color_hex: String,
}

pub struct RenderedBand {
    pub height_mm: f64,
    pub keep_together: bool,
    pub page_break_after: bool,
    pub elements: Vec<RenderedElement>,
}
```

---

## 2. O Transpilador `TypstTranspiler`

Traduz o layout estático e as tabelas com quebra dinâmica de página para código Typst:

```rust
use std::fmt::Write;

pub struct TypstTranspiler;

impl TypstTranspiler {
    fn escape_typst(input: &str) -> String {
        input.replace('#', "\\#")
             .replace('*', "\\*")
             .replace('_', "\\_")
             .replace('$', "\\$")
    }

    pub fn generate_source(
        page: &PageConfig,
        page_header: Option<&RenderedBand>,
        page_footer: Option<&RenderedBand>,
        flow_bands: &[RenderedBand],
    ) -> String {
        let mut code = String::with_capacity(64 * 1024);

        // Configurações de folha e margens
        writeln!(code, "#set page(").unwrap();
        writeln!(code, "  width: {}mm, height: {}mm,", page.width_mm, page.height_mm).unwrap();
        writeln!(
            code,
            "  margin: (top: {}mm, bottom: {}mm, left: {}mm, right: {}mm),",
            page.margin_top_mm, page.margin_bottom_mm, page.margin_left_mm, page.margin_right_mm
        ).unwrap();

        if let Some(ph) = page_header {
            writeln!(code, "  header: [").unwrap();
            Self::render_band(&mut code, ph);
            writeln!(code, "  ],").unwrap();
        }

        if let Some(pf) = page_footer {
            writeln!(code, "  footer: [").unwrap();
            Self::render_band(&mut code, pf);
            writeln!(code, "  ],").unwrap();
        }
        writeln!(code, ")\n").unwrap();

        // Renderização do fluxo de bandas
        for band in flow_bands {
            let is_table = band.elements.iter().any(|e| matches!(e, RenderedElement::Table { .. }));

            if is_table {
                for el in &band.elements {
                    if let RenderedElement::Table { columns_width_mm, headers, rows, footers, repeat_header } = el {
                        Self::render_table(&mut code, columns_width_mm, headers, rows, footers, *repeat_header);
                    }
                }
            } else {
                Self::render_band(&mut code, band);
            }

            if band.page_break_after {
                writeln!(code, "#pagebreak()").unwrap();
            }
        }

        code
    }

    fn render_band(code: &mut String, band: &RenderedBand) {
        writeln!(code, "#box(width: 100%, height: {}mm)[", band.height_mm).unwrap();
        for el in &band.elements {
            if let RenderedElement::TextBox { x_mm, y_mm, width_mm, text, font_family, font_size_pt, bold, color_hex, align, .. } = el {
                let weight = if *bold { "bold" } else { "regular" };
                writeln!(
                    code,
                    "  #place(dx: {}mm, dy: {}mm)[#box(width: {}mm)[#align({})[#text(font: \"{}\", size: {}pt, weight: \"{}\", fill: rgb(\"{}\"))[{}]]]]",
                    x_mm, y_mm, width_mm, align, font_family, font_size_pt, weight, color_hex, Self::escape_typst(text)
                ).unwrap();
            }
        }
        writeln!(code, "]").unwrap();
    }

    fn render_table(code: &mut String, col_widths: &[f64], headers: &[Vec<TableCell>], rows: &[Vec<TableCell>], footers: &[Vec<TableCell>], repeat: bool) {
        let cols_str = col_widths.iter().map(|w| format!("{}mm", w)).collect::<Vec<_>>().join(", ");
        writeln!(code, "#table(columns: ({}), stroke: 0.5pt + rgb(\"#CBD5E1\"),", cols_str).unwrap();
        
        if !headers.is_empty() {
            writeln!(code, "  table.header(repeat: {},", repeat).unwrap();
            for r in headers { for c in r { Self::write_cell(code, c); } }
            writeln!(code, "  ),").unwrap();
        }

        for r in rows { for c in r { Self::write_cell(code, c); } }
        for r in footers { for c in r { Self::write_cell(code, c); } }

        writeln!(code, ")\n").unwrap();
    }

    fn write_cell(code: &mut String, cell: &TableCell) {
        let weight = if cell.bold { "bold" } else { "regular" };
        writeln!(
            code,
            "    table.cell(colspan: {}, rowspan: {}, align: {}, text(weight: \"{}\", fill: rgb(\"{}\"))[{}]),",
            cell.colspan, cell.rowspan, cell.align, weight, cell.text_color_hex, Self::escape_typst(&cell.text)
        ).unwrap();
    }
}
```

---

## 3. Comandos Tauri IPC (Backend Rust)

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{ipc::Response, State};
use typst::text::Font;

pub struct ReportEngineState {
    pub system_fonts: Arc<Vec<Font>>,
}

#[derive(serde::Serialize)]
pub struct PreviewResponse {
    pub page_count: usize,
    pub pages_svg: Vec<String>,
    pub render_time_ms: u128,
}

#[tauri::command]
pub async fn render_report_preview(
    state: State<'_, ReportEngineState>,
    template_json: String,
    params: HashMap<String, serde_json::Value>,
) -> Result<PreviewResponse, String> {
    let start = std::time::Instant::now();
    let fonts = Arc::clone(&state.system_fonts);

    tauri::async_runtime::spawn_blocking(move || {
        let (typst_markup, assets) = crate::engine::pipeline::execute_and_transpile(&template_json, &params)?;
        let world = crate::engine::typst_world::InMemoryReportWorld::new(typst_markup, assets, (*fonts).clone());
        
        let warned = typst::compile(&world);
        let document = warned.output.map_err(|e| format!("{:?}", e))?;

        let pages_svg: Vec<String> = document.pages.iter().map(|p| typst_svg::svg(p)).collect();

        Ok(PreviewResponse {
            page_count: pages_svg.len(),
            pages_svg,
            render_time_ms: start.elapsed().as_millis(),
        })
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn export_report_pdf(
    state: State<'_, ReportEngineState>,
    template_json: String,
    params: HashMap<String, serde_json::Value>,
) -> Result<Response, String> {
    let fonts = Arc::clone(&state.system_fonts);

    let pdf_bytes = tauri::async_runtime::spawn_blocking(move || {
        let (typst_markup, assets) = crate::engine::pipeline::execute_and_transpile(&template_json, &params)?;
        let world = crate::engine::typst_world::InMemoryReportWorld::new(typst_markup, assets, (*fonts).clone());
        world.compile_to_pdf()
    }).await.map_err(|e| e.to_string())??;

    Ok(Response::new(pdf_bytes))
}
```

---

## 4. Componente de Preview Paginado (React / TypeScript)

```tsx
import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface PreviewResponse {
  page_count: number;
  pages_svg: string[];
  render_time_ms: number;
}

export const ReportPreviewViewer: React.FC<{ templateJson: string; parameters: Record<string, unknown> }> = ({
  templateJson,
  parameters,
}) => {
  const [pages, setPages] = useState<string[]>([]);
  const [currentPage, setCurrentPage] = useState<number>(1);
  const [zoom, setZoom] = useState<number>(1.0);

  useEffect(() => {
    invoke<PreviewResponse>('render_report_preview', { templateJson, params: parameters })
      .then((res) => {
        setPages(res.pages_svg);
        setCurrentPage(1);
      });
  }, [templateJson, parameters]);

  return (
    <div className="flex flex-col h-full bg-slate-800 text-slate-100">
      <div className="flex items-center justify-between px-4 py-2 bg-slate-900 border-b border-slate-700">
        <div className="flex items-center gap-2">
          <button disabled={currentPage <= 1} onClick={() => setCurrentPage((p) => p - 1)} className="px-2 py-1 bg-slate-700 rounded">
            Anterior
          </button>
          <span>Página {currentPage} de {pages.length}</span>
          <button disabled={currentPage >= pages.length} onClick={() => setCurrentPage((p) => p + 1)} className="px-2 py-1 bg-slate-700 rounded">
            Próxima
          </button>
        </div>
        <div className="flex items-center gap-2">
          <button onClick={() => setZoom((z) => Math.max(0.5, z - 0.1))} className="px-2 py-1 bg-slate-700 rounded">-</button>
          <span>{Math.round(zoom * 100)}%</span>
          <button onClick={() => setZoom((z) => Math.min(2.0, z + 0.1))} className="px-2 py-1 bg-slate-700 rounded">+</button>
        </div>
      </div>
      <div className="flex-1 overflow-auto p-8 flex flex-col items-center gap-6">
        {pages.map((svg, idx) => (
          <div
            key={idx + 1}
            style={{ transform: `scale(${zoom})`, transformOrigin: 'top center' }}
            className={`bg-white shadow-2xl ${currentPage === idx + 1 ? 'ring-2 ring-blue-500' : ''}`}
            dangerouslySetInnerHTML={{ __html: svg }}
          />
        ))}
      </div>
    </div>
  );
};
```