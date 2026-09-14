use std::path::PathBuf;

use serde_json::Value;

/// The contract schema file at `.specs/001-duel-search/contracts/<name>`.
pub fn load_contract(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../.specs/001-duel-search/contracts")
        .join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("{} is not JSON: {error}", path.display()))
}

/// Validates `instance` against the JSON Schema subset the contracts use:
/// `type`, `const`, `enum`, `minimum`, `minLength`, `maximum`, `minItems`, `maxItems`, `items`, `required`, `properties` and
/// `additionalProperties: false`. A keyword outside the subset is an error, so a
/// contract that grows cannot silently stop being checked.
pub fn validate(schema: &Value, instance: &Value) -> Result<(), String> {
    validate_at(schema, instance, "$")
}

const ANNOTATIONS: &[&str] = &[
    "$schema",
    "$id",
    "$comment",
    "title",
    "description",
    "x-contract",
    "x-version",
    "x-captured",
];

fn validate_at(schema: &Value, instance: &Value, at: &str) -> Result<(), String> {
    let object = schema
        .as_object()
        .ok_or_else(|| format!("{at}: schema is not an object"))?;
    for (keyword, rule) in object {
        match keyword.as_str() {
            key if ANNOTATIONS.contains(&key) => {}
            "type" => check_type(rule, instance, at)?,
            "const" => {
                if rule != instance {
                    return Err(format!("{at}: expected constant {rule}, got {instance}"));
                }
            }
            "enum" => {
                let allowed = rule
                    .as_array()
                    .ok_or_else(|| format!("{at}: enum is not a list"))?;
                if !allowed.contains(instance) {
                    return Err(format!("{at}: {instance} is not one of {rule}"));
                }
            }
            "minimum" => {
                if let Some(number) = instance.as_f64() {
                    let floor = rule.as_f64().ok_or_else(|| format!("{at}: bad minimum"))?;
                    if number < floor {
                        return Err(format!("{at}: {number} is below the minimum {floor}"));
                    }
                }
            }
            "maximum" => {
                if let Some(number) = instance.as_f64() {
                    let ceiling = rule.as_f64().ok_or_else(|| format!("{at}: bad maximum"))?;
                    if number > ceiling {
                        return Err(format!("{at}: {number} is above the maximum {ceiling}"));
                    }
                }
            }
            "minItems" | "maxItems" => {
                if let Some(items) = instance.as_array() {
                    let bound = rule
                        .as_u64()
                        .ok_or_else(|| format!("{at}: bad {keyword}"))?;
                    let count = items.len() as u64;
                    if keyword == "minItems" && count < bound {
                        return Err(format!("{at}: {count} items, at least {bound} required"));
                    }
                    if keyword == "maxItems" && count > bound {
                        return Err(format!("{at}: {count} items, at most {bound} allowed"));
                    }
                }
            }
            "items" => {
                if let Some(items) = instance.as_array() {
                    for (index, item) in items.iter().enumerate() {
                        validate_at(rule, item, &format!("{at}[{index}]"))?;
                    }
                }
            }
            "minLength" => {
                if let Some(text) = instance.as_str() {
                    let floor = rule
                        .as_u64()
                        .ok_or_else(|| format!("{at}: bad minLength"))?;
                    if (text.chars().count() as u64) < floor {
                        return Err(format!("{at}: string shorter than {floor}"));
                    }
                }
            }
            "required" => {
                if let Some(members) = instance.as_object() {
                    for name in rule
                        .as_array()
                        .ok_or_else(|| format!("{at}: bad required"))?
                    {
                        let name = name
                            .as_str()
                            .ok_or_else(|| format!("{at}: bad required entry"))?;
                        if !members.contains_key(name) {
                            return Err(format!("{at}: missing required member `{name}`"));
                        }
                    }
                }
            }
            "properties" => {
                if let Some(members) = instance.as_object() {
                    let known = rule
                        .as_object()
                        .ok_or_else(|| format!("{at}: bad properties"))?;
                    for (name, sub_schema) in known {
                        if let Some(value) = members.get(name) {
                            validate_at(sub_schema, value, &format!("{at}.{name}"))?;
                        }
                    }
                }
            }
            "additionalProperties" => {
                if *rule != Value::Bool(false) {
                    return Err(format!(
                        "{at}: only additionalProperties: false is supported"
                    ));
                }
                if let (Some(members), Some(known)) = (
                    instance.as_object(),
                    schema.get("properties").and_then(Value::as_object),
                ) && let Some(extra) = members.keys().find(|name| !known.contains_key(*name))
                {
                    return Err(format!("{at}: unexpected member `{extra}`"));
                }
            }
            other => return Err(format!("{at}: unsupported schema keyword `{other}`")),
        }
    }
    Ok(())
}

fn check_type(rule: &Value, instance: &Value, at: &str) -> Result<(), String> {
    let names: Vec<&str> = match rule {
        Value::String(name) => vec![name.as_str()],
        Value::Array(list) => list.iter().filter_map(Value::as_str).collect(),
        _ => return Err(format!("{at}: bad type rule")),
    };
    let matches = |name: &str| match name {
        "string" => instance.is_string(),
        "integer" => instance.is_i64() || instance.is_u64(),
        "number" => instance.is_number(),
        "boolean" => instance.is_boolean(),
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "null" => instance.is_null(),
        _ => false,
    };
    if names.iter().any(|name| matches(name)) {
        Ok(())
    } else {
        Err(format!("{at}: {instance} is not of type {rule}"))
    }
}
