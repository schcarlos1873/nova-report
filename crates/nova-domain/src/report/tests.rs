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

/// Fábrica → Máquina (vários valores) → relatório, com queries de lista.
fn cascade_report() -> ReportDefinition {
    let mut def = ReportDefinition::from_json(SPEC_EXAMPLE).unwrap();
    def.data_sources[0].queries.push(QueryDef { name: "LkFabricas".into(), sql: "SELECT Id, Code FROM fab".into() });
    def.data_sources[0].queries.push(QueryDef { name: "LkMaquinas".into(), sql: "SELECT Id, Code FROM maq WHERE FactoryId IN (:fabrica)".into() });
    def.parameters.push(
        serde_json::from_value(serde_json::json!({
            "name": "fabrica", "type": "int", "multiValue": true,
            "availableValues": { "query": "LkFabricas", "valueField": "Id", "labelField": "Code" }
        }))
        .unwrap(),
    );
    def.parameters.push(
        serde_json::from_value(serde_json::json!({
            "name": "maquina", "type": "int", "multiValue": true, "dependsOn": ["fabrica"],
            "availableValues": { "query": "LkMaquinas", "valueField": "Id" }
        }))
        .unwrap(),
    );
    def
}

#[test]
fn cascading_parameters_validate_and_roundtrip() {
    let def = cascade_report();
    assert!(def.ensure_valid().is_ok(), "{:?}", def.validate());
    let back: ReportDefinition = serde_json::from_str(&serde_json::to_string(&def).unwrap()).unwrap();
    assert_eq!(back, def);
    // campos novos são opcionais: parâmetro simples não serializa nada além do que já existia
    let plain = serde_json::to_value(&def.parameters[0]).unwrap();
    assert!(plain.get("multiValue").is_none() && plain.get("dependsOn").is_none() && plain.get("availableValues").is_none());
}

#[test]
fn lookup_queries_are_not_report_data() {
    let mut def = cascade_report();
    assert!(def.is_parameter_only_query("LkMaquinas"));
    assert!(!def.is_parameter_only_query("Pedidos"));
    // sem dataSource no detalhe, a única query de dados continua sendo a principal
    for b in &mut def.bands {
        b.data_source = None;
    }
    assert_eq!(def.main_query_name().as_deref(), Some("Pedidos"));
}

#[test]
fn detects_dependency_problems() {
    // índices: 0 = data_inicio, 1 = fabrica, 2 = maquina
    let mut def = cascade_report();
    def.parameters[2].depends_on.clear();
    let msgs: Vec<String> = def.validate().into_iter().map(|i| i.message).collect();
    assert!(msgs.iter().any(|m| m.contains("':fabrica'") && m.contains("dependsOn")), "{msgs:?}");

    let mut def = cascade_report();
    def.parameters[1].depends_on = vec!["maquina".into()];
    def.parameters[0].depends_on = vec!["inexistente".into()];
    let issues = def.validate();
    assert!(issues.iter().any(|i| i.severity == Severity::Error && i.message.contains("circular")), "{issues:?}");
    assert!(issues.iter().any(|i| i.message.contains("'inexistente' não declarado")));

    let mut def = cascade_report();
    def.parameters[2].available_values.as_mut().unwrap().query = "Nada".into();
    assert!(def.validate().iter().any(|i| i.message.contains("'Nada' não existe")));
}
