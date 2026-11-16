// SPDX-License-Identifier: MulanPSL-2.0
//! RFC 7396 JSON Merge Patch: `null` deletes keys, objects recurse, arrays replace.
//! RFC 7396 JSON Merge Patch：`null` 删键、对象递归、数组整体替换。

use alloc::string::String;

use crate::error::{ErrorKind, PatchError};
use crate::value::{Object, Value, MAX_VALUE_DEPTH};

/// Apply an RFC 7396 merge patch to a base value. ｜ 对基值应用 RFC 7396 Merge Patch。
pub fn merge_patch(base: &Value, patch: &Value) -> Result<Value, PatchError> {
    merge(base, patch, 0)
}

fn merge(base: &Value, patch: &Value, depth: usize) -> Result<Value, PatchError> {
    if depth > MAX_VALUE_DEPTH {
        return Err(PatchError::new(ErrorKind::DepthLimit));
    }
    let patch_obj = match patch {
        Value::Object(o) => o,
        _ => return Ok(patch.clone()),
    };
    let mut out = match base {
        Value::Object(b) => Object::clone(b),
        _ => Object::new(),
    };
    for (key, patch_value) in patch_obj.iter() {
        if matches!(patch_value, Value::Null) {
            out.remove(key);
        } else {
            let base_value = out.get(key).cloned().unwrap_or(Value::Null);
            let merged = merge(&base_value, patch_value, depth + 1)?;
            out.insert(String::from(key), merged);
        }
    }
    Ok(Value::Object(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_str;

    fn p(s: &str) -> Value {
        parse_str(s).unwrap()
    }

    #[test]
    fn null_deletes_key() {
        let out = merge_patch(&p("a: 1 b: 2"), &p("a: null c: 3")).unwrap();
        assert_eq!(out.get("a"), None);
        assert_eq!(out.get("b").and_then(Value::as_int), Some(2));
        assert_eq!(out.get("c").and_then(Value::as_int), Some(3));
    }

    #[test]
    fn nested_object_recurses() {
        let out = merge_patch(&p("o: { a: 1 b: 2 }"), &p("o: { a: null c: 3 }")).unwrap();
        let o = out.get("o").and_then(Value::as_object).unwrap();
        assert_eq!(o.get("a"), None);
        assert_eq!(o.get("b").and_then(Value::as_int), Some(2));
        assert_eq!(o.get("c").and_then(Value::as_int), Some(3));
    }

    #[test]
    fn array_replaces_whole() {
        let out = merge_patch(&p("a: [1 2 3]"), &p("a: [9]")).unwrap();
        let a = out.get("a").and_then(Value::as_array).unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].as_int(), Some(9));
    }

    #[test]
    fn scalar_patch_replaces() {
        // 非对象 patch 在顶层直接返回 patch 本身
        let out = merge_patch(&p("a: 1"), &p("7")).unwrap();
        assert_eq!(out.as_int(), Some(7));
        let out = merge_patch(&p("a: 1"), &p("hello")).unwrap();
        assert_eq!(out.as_str(), Some("hello"));
    }

    #[test]
    fn non_object_base_starts_empty() {
        let out = merge_patch(&p("7"), &p("a: 1")).unwrap();
        assert_eq!(out.get("a").and_then(Value::as_int), Some(1));
    }
}
