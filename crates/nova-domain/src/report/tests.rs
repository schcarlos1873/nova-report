use super::*;

const SPEC_EXAMPLE: &str = r#"{
  "$schema": "https://novareport.io/schemas/v1/report.json",
  "meta": {
    "title": "Fatura de Vendas",
    "paperSize": "A4",
    "orientation": "portrait",
    "margins": { "top": 10, "bottom": 10, "left": 15, "right": 15 }
  },
  "dataSources": [{
    "id": "ds_main", "provider": "postgres", "connectionStringRef": "ENV_DB_CONN",
    "queries": [{ "name": "Pedidos", "sql": "SELECT id, cliente, valor_total, data FROM pedidos WHERE data >= :data_inicio" }]
  }],
  "parameters": [{ "name": "data_inicio", "type": "date", "label": "Data Inicial", "required": true }],
  "bands": [
    { "type": "detail", "height": 8, "dataSource": "Pedidos", "elements": [
        { "id": "field_cliente", "type": "expression", "x": 0, "y": 0, "width": 100, "height": 8,
          "expression": "Fields.cliente", "style": { "fontSize": 10 } }
    ]},
    { "type": "pageHeader", "height": 25, "elements": [
        { "id": "txt_title", "type": "label", "x": 0, "y": 5, "width": 180, "height": 10,
          "text": "RELATÓRIO DE VENDAS", "style": { "fontSize": 16, "bold": true, "align": "center" } }
    ]}
  ]
}"#;

#[test]
fn parses_spec_example_and_normalizes_band_order() {
    let def = ReportDefinition::from_json(SPEC_EXAMPLE).unwrap();
    assert_eq!(def.bands[0].band_type, BandType::PageHeader);
    assert_eq!(def.bands[1].band_type, BandType::Detail);
    assert_eq!(def.bands[0].id, "page_header_1");
    assert!(def.ensure_valid().is_ok(), "{:?}", def.validate());
    let ElementKind::Label { text } = &def.bands[0].elements[0].kind else { panic!() };
    assert_eq!(text, "RELATÓRIO DE VENDAS");
    assert!(def.bands[0].elements[0].style.bold);
}

#[test]
fn canonical_json_is_deterministic_and_roundtrips() {
    let def = ReportDefinition::from_json(SPEC_EXAMPLE).unwrap();
    let a = def.to_canonical_json().unwrap();
    let b = ReportDefinition::from_json(&a).unwrap().to_canonical_json().unwrap();
    assert_eq!(a, b);
}

#[test]
fn detects_undeclared_parameters_and_duplicate_ids() {
    let mut def = ReportDefinition::from_json(SPEC_EXAMPLE).unwrap();
    def.parameters.clear();
    let dup = def.bands[1].elements[0].clone();
    def.bands[1].elements.push(dup);
    let issues = def.validate();
    assert!(issues.iter().any(|i| i.message.contains(":data_inicio")));
    assert!(issues.iter().any(|i| i.message.contains("duplicado")));
    assert!(def.ensure_valid().is_err());
}

#[test]
fn landscape_swaps_dimensions() {
    let mut def = ReportDefinition::new_blank("x");
    def.meta.orientation = Orientation::Landscape;
    let g = def.page_geometry();
    assert_eq!((g.width_mm, g.height_mm), (297.0, 210.0));
    assert_eq!(g.printable_width(), 267.0);
}

#[test]
fn group_bands_are_ordered_outer_to_inner() {
    let mut def = ReportDefinition::new_blank("x");
    def.groups = vec![
        GroupDef { name: "regiao".into(), expression: "Fields.regiao".into(), page_break_between: false },
        GroupDef { name: "cliente".into(), expression: "Fields.cliente".into(), page_break_between: false },
    ];
    for (t, g) in [
        (BandType::GroupFooter, "regiao"),
        (BandType::GroupHeader, "cliente"),
        (BandType::GroupFooter, "cliente"),
        (BandType::GroupHeader, "regiao"),
    ] {
        let mut b = Band::new(t, 6.0);
        b.group = Some(g.into());
        def.bands.push(b);
    }
    def.normalize();
    let seq: Vec<_> = def
        .bands
        .iter()
        .filter(|b| b.band_type.is_group_band())
        .map(|b| (b.band_type, b.group.clone().unwrap()))
        .collect();
    assert_eq!(
        seq,
        vec![
            (BandType::GroupHeader, "regiao".to_string()),
            (BandType::GroupHeader, "cliente".to_string()),
            (BandType::GroupFooter, "cliente".to_string()),
            (BandType::GroupFooter, "regiao".to_string()),
        ]
    );
}
