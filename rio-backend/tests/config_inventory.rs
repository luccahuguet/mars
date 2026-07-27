use rio_backend::config::Config;
use serde_json::Value as JsonValue;
use std::collections::BTreeSet;
use toml::Value as TomlValue;

const INVENTORY: &str = include_str!("../../docs/yazelix/config_inventory.v1.json");

fn inventory() -> JsonValue {
    serde_json::from_str(INVENTORY).expect("Mars config inventory must be valid JSON")
}

fn entries(inventory: &JsonValue) -> &[JsonValue] {
    inventory["entries"]
        .as_array()
        .expect("inventory entries must be an array")
}

fn current_platform() -> &'static str {
    match std::env::consts::OS {
        "linux" => "linux",
        "macos" => "macos",
        "windows" => "windows",
        other => panic!("inventory does not define defaults for {other}"),
    }
}

fn literal_default(entry: &JsonValue) -> Option<&JsonValue> {
    let default = &entry["default"];
    default.get("value").or_else(|| {
        default
            .get("platform")
            .and_then(|values| values.get(current_platform()))
    })
}

fn choice_value(choice: &JsonValue) -> &JsonValue {
    choice.get("value").unwrap_or(choice)
}

fn available_here(value: &JsonValue) -> bool {
    value["platforms"].as_array().is_none_or(|platforms| {
        platforms
            .iter()
            .any(|platform| platform.as_str() == Some(current_platform()))
    }) && value["features"].as_array().is_none_or(|features| {
        features
            .iter()
            .all(|feature| feature == "wgpu" && cfg!(feature = "wgpu"))
    })
}

fn assert_shape_refs_resolve(
    value: &JsonValue,
    definitions: &serde_json::Map<String, JsonValue>,
) {
    match value {
        JsonValue::Array(values) => {
            for value in values {
                assert_shape_refs_resolve(value, definitions);
            }
        }
        JsonValue::Object(values) => {
            if let Some(reference) = values.get("ref").and_then(JsonValue::as_str) {
                assert!(
                    definitions.contains_key(reference),
                    "unknown shape reference {reference}"
                );
            }
            for value in values.values() {
                assert_shape_refs_resolve(value, definitions);
            }
        }
        _ => {}
    }
}

fn set_path(document: &mut toml::Table, path: &str, value: &JsonValue) {
    let mut segments = path.split('.').peekable();
    let mut table = document;
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            let value =
                TomlValue::try_from(value).expect("inventory value must encode as TOML");
            table.insert(segment.to_string(), value);
            return;
        }
        table = table
            .entry(segment)
            .or_insert_with(|| TomlValue::Table(toml::Table::new()))
            .as_table_mut()
            .unwrap_or_else(|| panic!("inventory path {path} conflicts with a scalar"));
    }
}

fn default_document(inventory: &JsonValue) -> toml::Table {
    let mut document = toml::Table::new();
    for entry in entries(inventory)
        .iter()
        .filter(|entry| entry.get("constraints").is_none_or(available_here))
    {
        if let Some(value) = literal_default(entry) {
            set_path(&mut document, entry["path"].as_str().unwrap(), value);
        }
    }
    document
}

fn parse_field(inventory: &JsonValue, path: &str, value: &JsonValue) {
    let mut document = default_document(inventory);
    set_path(&mut document, path, value);
    let source =
        toml::to_string(&document).expect("inventory sample must encode as TOML");
    toml::from_str::<Config>(&source).unwrap_or_else(|error| {
        panic!("inventory sample for {path} was rejected: {error}\n{source}")
    });
}

fn parse_referenced_shape_choices(
    inventory: &JsonValue,
    entry: &JsonValue,
    value: &JsonValue,
) {
    let Some(reference) = entry["shape"].get("ref").and_then(JsonValue::as_str) else {
        return;
    };
    let Some(fields) = inventory["shape_definitions"][reference]["fields"].as_object()
    else {
        return;
    };
    let path = entry["path"].as_str().unwrap();
    for (field, shape) in fields {
        if shape
            .get("constraints")
            .is_some_and(|constraints| !available_here(constraints))
        {
            continue;
        }
        let Some(choices) = shape.get("choices").and_then(JsonValue::as_array) else {
            continue;
        };
        for choice in choices.iter().filter(|choice| available_here(choice)) {
            let mut candidate = value.clone();
            candidate[field] = choice_value(choice).clone();
            parse_field(inventory, path, &candidate);
        }
    }
}

#[test]
fn inventory_is_deterministic_and_explicit() {
    let inventory = inventory();
    assert_eq!(inventory["schema_version"], 1);
    assert_eq!(inventory["owner"], "mars");

    let entries = entries(&inventory);
    let definitions = inventory["shape_definitions"]
        .as_object()
        .expect("shape definitions must be an object");

    let paths = entries
        .iter()
        .map(|entry| {
            assert!(entry["group"].is_string());
            assert!(entry["shape"].is_string() || entry["shape"].is_object());
            assert_shape_refs_resolve(&entry["shape"], definitions);
            let default = entry["default"]
                .as_object()
                .expect("entry default must be an object");
            assert_eq!(default.len(), 1, "entry must have exactly one default kind");
            assert!(default
                .keys()
                .all(|kind| ["value", "platform", "none", "built_in"]
                    .contains(&kind.as_str())));
            if let Some(platforms) =
                default.get("platform").and_then(JsonValue::as_object)
            {
                assert_eq!(
                    platforms
                        .keys()
                        .map(String::as_str)
                        .collect::<BTreeSet<_>>(),
                    BTreeSet::from(["linux", "macos", "windows"])
                );
            }
            assert!(entry["description"]
                .as_str()
                .is_some_and(|description| !description.is_empty()));
            entry["path"].as_str().expect("entry path must be a string")
        })
        .collect::<Vec<_>>();
    for definition in definitions.values() {
        assert_shape_refs_resolve(definition, definitions);
    }
    assert!(paths.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        paths.iter().copied().collect::<BTreeSet<_>>().len(),
        paths.len()
    );

    let excluded = inventory["excluded"]
        .as_array()
        .expect("excluded entries must be an array")
        .iter()
        .map(|entry| entry["path"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        excluded,
        BTreeSet::from(["adaptive_colors", "load_diagnostic", "yazelix"])
    );
}

#[test]
fn advertised_defaults_match_config_default() {
    let inventory = inventory();
    let document = default_document(&inventory);
    let source = toml::to_string(&document).expect("defaults must encode as TOML");
    let parsed: Config = toml::from_str(&source).unwrap_or_else(|error| {
        panic!("advertised defaults were rejected: {error}\n{source}")
    });
    let mut expected = Config::default();
    assert_eq!(parsed.colors.background.0, expected.colors.background.0);
    expected.colors.background.1 = parsed.colors.background.1;
    assert_eq!(parsed, expected);
}

#[test]
fn choices_and_structured_examples_parse_through_mars() {
    let inventory = inventory();
    for entry in entries(&inventory) {
        let path = entry["path"].as_str().unwrap();
        let default_or_example = entry.get("example").or_else(|| literal_default(entry));
        if let Some(value) = default_or_example {
            if entry.get("constraints").is_none_or(available_here) {
                parse_field(&inventory, path, value);
                parse_referenced_shape_choices(&inventory, entry, value);
            }
        }

        if let Some(choices) = entry.get("choices").and_then(JsonValue::as_array) {
            for choice in choices.iter().filter(|choice| available_here(choice)) {
                parse_field(&inventory, path, choice_value(choice));
            }
        }
    }
}

#[test]
fn window_blur_inventory_matches_the_parser_boundary() {
    let inventory = inventory();
    let blur = entries(&inventory)
        .iter()
        .find(|entry| entry["path"] == "window.blur")
        .expect("window.blur must be inventoried");
    let choices = blur["choices"]
        .as_array()
        .unwrap()
        .iter()
        .map(choice_value)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        choices,
        vec![
            JsonValue::Bool(false),
            JsonValue::Bool(true),
            JsonValue::String("macos-glass-regular".into()),
            JsonValue::String("macos-glass-clear".into()),
        ]
    );
    assert!(toml::from_str::<Config>("[window]\nblur = \"unknown\"").is_err());
}
