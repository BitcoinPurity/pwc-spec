use std::sync::OnceLock;

use serde_json::{json, Value};

const BASE: &str = "https://purity-wallet-connect.invalid/schema/";
const CORE_STRICT: &str = "pwc-core-0.2.1.strict.schema.json";
const LOCAL: &str = "pwc-transport-local-0.1.0.schema.json";

fn load(name: &str) -> Value {
    let path = format!("{}/../pwc-schema/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn schemas() -> &'static Vec<(String, Value)> {
    static SCHEMAS: OnceLock<Vec<(String, Value)>> = OnceLock::new();
    SCHEMAS.get_or_init(|| {
        ["pwc-core-0.2.1.schema.json", CORE_STRICT, LOCAL]
            .iter()
            .map(|n| (format!("{BASE}{n}"), load(n)))
            .collect()
    })
}

/// True if the strict Core schema has a `$defs` entry named `def`.
pub fn core_defines(def: &str) -> bool {
    schemas()[1].1["$defs"].get(def).is_some()
}

fn options() -> jsonschema::ValidationOptions {
    let schemas = schemas();
    let mut opts = jsonschema::options().with_draft(jsonschema::Draft::Draft202012);
    for (uri, schema) in schemas {
        opts = opts.with_resource(uri.as_str(), jsonschema::Resource::from_contents(schema.clone()).unwrap());
    }
    opts
}

fn check(file: &str, def: &str, instance: &Value) -> Result<(), String> {
    let validator = options()
        .build(&json!({ "$ref": format!("{BASE}{file}#/$defs/{def}") }))
        .map_err(|e| format!("schema build: {e}"))?;
    let errors: Vec<String> = validator.iter_errors(instance).map(|e| format!("{} at {}", e, e.instance_path)).collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!("{def}: {}\ninstance: {instance}", errors.join("; ")))
    }
}

/// Validates `instance` against a `$defs` entry of the strict Core schema.
pub fn core(def: &str, instance: &Value) -> Result<(), String> {
    check(CORE_STRICT, def, instance)
}

/// Validates `instance` against a `$defs` entry of the LOCAL transport schema.
pub fn local(def: &str, instance: &Value) -> Result<(), String> {
    check(LOCAL, def, instance)
}
