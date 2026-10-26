//! Edits JSON / JSON-with-comments settings files (VS Code, Cursor, Claude Code)
//! in place, keeping comments, ordering and formatting of everything else.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};
use jsonc_parser::cst::{CstInputValue, CstObject, CstRootNode};
use jsonc_parser::ParseOptions;
use serde_json::Value;

fn to_input(v: &Value) -> CstInputValue {
    match v {
        Value::Null => CstInputValue::Null,
        Value::Bool(b) => CstInputValue::Bool(*b),
        Value::Number(n) => CstInputValue::Number(n.to_string()),
        Value::String(s) => CstInputValue::String(s.clone()),
        Value::Array(a) => CstInputValue::Array(a.iter().map(to_input).collect()),
        Value::Object(o) => {
            CstInputValue::Object(o.iter().map(|(k, v)| (k.clone(), to_input(v))).collect())
        }
    }
}

fn parse(text: &str) -> Result<CstRootNode> {
    let src = if text.trim().is_empty() { "{}" } else { text };
    CstRootNode::parse(src, &ParseOptions::default()).map_err(|e| anyhow!("invalid JSON: {e}"))
}

fn walk(root: &CstRootNode, key: &[&str], create: bool) -> Option<CstObject> {
    let mut obj = if create {
        root.object_value_or_set()
    } else {
        root.object_value()?
    };
    for k in &key[..key.len() - 1] {
        obj = if create {
            obj.object_value_or_set(k)
        } else {
            obj.object_value(k)?
        };
    }
    Some(obj)
}

pub fn get(text: &str, key: &[&str]) -> Result<Option<Value>> {
    let root = parse(text)?;
    let Some(obj) = walk(&root, key, false) else {
        return Ok(None);
    };
    Ok(obj
        .get(key[key.len() - 1])
        .and_then(|p| p.value())
        .and_then(|v| v.to_serde_value()))
}

/// `None` removes the key.
pub fn set(text: &str, key: &[&str], value: Option<&Value>) -> Result<String> {
    let root = parse(text)?;
    let last = key[key.len() - 1];
    match value {
        Some(v) => {
            let obj = walk(&root, key, true).ok_or_else(|| anyhow!("not an object"))?;
            match obj.get(last) {
                Some(prop) => prop.set_value(to_input(v)),
                None => {
                    obj.append(last, to_input(v));
                }
            }
        }
        None => {
            if let Some(obj) = walk(&root, key, false) {
                if let Some(prop) = obj.get(last) {
                    prop.remove();
                }
                // Drop parent objects GitAssist created and that are now empty.
                if key.len() > 1 && obj.properties().is_empty() {
                    if let Some(parent) = walk(&root, &key[..key.len() - 1], false) {
                        if let Some(p) = parent.get(key[key.len() - 2]) {
                            p.remove();
                        }
                    }
                }
            }
        }
    }
    let mut out = root.to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

pub fn get_file(path: &Path, key: &[&str]) -> Result<Option<Value>> {
    match fs::read_to_string(path) {
        Ok(t) => get(&t, key),
        Err(_) => Ok(None),
    }
}

pub fn set_file(path: &Path, key: &[&str], value: Option<&Value>) -> Result<()> {
    let text = fs::read_to_string(path).unwrap_or_default();
    let new = set(&text, key, value)?;
    if new != text {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, new)?;
    }
    Ok(())
}
