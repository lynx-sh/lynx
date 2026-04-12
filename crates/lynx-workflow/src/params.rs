//! Parameter resolution, validation, and template expansion.

use crate::schema::{ParamType, WorkflowParam};
use anyhow::{bail, Result};
use std::collections::HashMap;

/// Resolve workflow parameters: validate, apply defaults, type-check.
pub fn resolve_params(
    param_defs: &[WorkflowParam],
    provided: &HashMap<String, String>,
) -> Result<HashMap<String, String>> {
    let mut resolved = HashMap::new();

    for def in param_defs {
        let value = if let Some(v) = provided.get(&def.name) {
            v.clone()
        } else if let Some(ref default) = def.default {
            default.clone()
        } else if def.required {
            bail!("missing required parameter: '{}'", def.name);
        } else {
            continue;
        };

        // Validate choices
        if !def.choices.is_empty() && !def.choices.contains(&value) {
            bail!(
                "parameter '{}': '{}' is not a valid choice (expected one of: {})",
                def.name,
                value,
                def.choices.join(", ")
            );
        }

        // Type check
        match def.param_type {
            ParamType::Int => {
                if value.parse::<i64>().is_err() {
                    bail!("parameter '{}': '{}' is not a valid integer", def.name, value);
                }
            }
            ParamType::Bool => {
                if !matches!(value.as_str(), "true" | "false" | "1" | "0" | "yes" | "no") {
                    bail!(
                        "parameter '{}': '{}' is not a valid boolean (use true/false)",
                        def.name,
                        value
                    );
                }
            }
            ParamType::String => {} // any string is valid
        }

        resolved.insert(def.name.clone(), value);
    }

    // Pass through any extra provided params not in definitions
    for (k, v) in provided {
        if !resolved.contains_key(k) {
            resolved.insert(k.clone(), v.clone());
        }
    }

    Ok(resolved)
}

/// Expand `$param_name` templates in a string with resolved param values.
pub fn expand_template(template: &str, params: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in params {
        result = result.replace(&format!("${key}"), value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_param(name: &str, param_type: ParamType, required: bool) -> WorkflowParam {
        WorkflowParam {
            name: name.into(),
            param_type,
            required,
            default: None,
            choices: vec![],
            description: String::new(),
        }
    }

    #[test]
    fn resolve_with_defaults() {
        let defs = vec![WorkflowParam {
            name: "env".into(),
            param_type: ParamType::String,
            required: false,
            default: Some("staging".into()),
            choices: vec![],
            description: String::new(),
        }];
        let result = resolve_params(&defs, &HashMap::new()).unwrap();
        assert_eq!(result.get("env").unwrap(), "staging");
    }

    #[test]
    fn reject_missing_required() {
        let defs = vec![make_param("version", ParamType::String, true)];
        assert!(resolve_params(&defs, &HashMap::new()).is_err());
    }

    #[test]
    fn validate_choices() {
        let defs = vec![WorkflowParam {
            name: "env".into(),
            param_type: ParamType::String,
            required: true,
            default: None,
            choices: vec!["dev".into(), "staging".into(), "prod".into()],
            description: String::new(),
        }];
        let mut provided = HashMap::new();
        provided.insert("env".into(), "dev".into());
        assert!(resolve_params(&defs, &provided).is_ok());

        provided.insert("env".into(), "invalid".into());
        assert!(resolve_params(&defs, &provided).is_err());
    }

    #[test]
    fn type_check_int() {
        let defs = vec![make_param("count", ParamType::Int, true)];
        let mut p = HashMap::new();
        p.insert("count".into(), "42".into());
        assert!(resolve_params(&defs, &p).is_ok());

        p.insert("count".into(), "abc".into());
        assert!(resolve_params(&defs, &p).is_err());
    }

    #[test]
    fn type_check_bool() {
        let defs = vec![make_param("dry_run", ParamType::Bool, true)];
        let mut p = HashMap::new();
        p.insert("dry_run".into(), "true".into());
        assert!(resolve_params(&defs, &p).is_ok());

        p.insert("dry_run".into(), "maybe".into());
        assert!(resolve_params(&defs, &p).is_err());
    }

    #[test]
    fn template_expansion() {
        let mut params = HashMap::new();
        params.insert("version".into(), "1.0.0".into());
        params.insert("env".into(), "prod".into());
        assert_eq!(
            expand_template("deploy $version to $env", &params),
            "deploy 1.0.0 to prod"
        );
    }
}
