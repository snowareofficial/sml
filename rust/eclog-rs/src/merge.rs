// SPDX-License-Identifier: MIT OR Apache-2.0 OR MulanPSL-2.0
//! RFC 7396 Merge Patch，语义照 C++ `ObjectNodeImpl::merge` / `removeNullNodes`。
//! RFC 7396 merge patch, following C++ `ObjectNodeImpl::merge` / `removeNullNodes`.
//!
//! 三条规则：`null` 删键；两边都是对象则递归；其它情况整体替换。补丁里新插入的
//! 子树会先**递归删掉其中的 `null` 成员**（`{"a":{"b":null}}` 插进去是 `{"a":{}}`），
//! 这正是 RFC 7396 与"朴素递归赋值"的区别。
//! Three rules: `null` deletes a key, two objects merge recursively, anything else
//! replaces. A subtree inserted from the patch first has its `null` members stripped
//! recursively — the difference from a naive recursive assignment.
//!
//! 位置语义与 `Object::insert` 一致：替换时保留目标键原有的位置，只换键与值。
//! Position semantics match `Object::insert`: replacement keeps the target's slot.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::value::{Object, Value};

/// 把 `patch` 合并进 `target`（原地修改 `target`）。
/// Merge `patch` into `target` in place.
///
/// ```
/// use eclog::{merge_patch, parse};
///
/// let mut target = parse(b"a: b\nc: d").unwrap();
/// let patch = parse(b"a: null\nc: e").unwrap();
/// merge_patch(target.as_object_mut().unwrap(), patch.as_object().unwrap());
/// assert_eq!(eclog::to_typical(&target), "c: e\n");
/// ```
pub fn merge_patch(target: &mut Object, patch: &Object) {
    for (key, patch_value) in patch.iter() {
        let exists = target.contains_key(key.text());

        if !exists {
            if patch_value.is_null() {
                continue;
            }
            let mut value = patch_value.clone();
            strip_nulls(&mut value);
            target.insert(key.clone(), value);
            continue;
        }

        if patch_value.is_null() {
            target.remove(key.text());
            continue;
        }

        if let Value::Object(patch_object) = patch_value {
            if let Some(Value::Object(target_object)) = target.get_mut(key.text()) {
                merge_patch(target_object, patch_object);
                continue;
            }
        }

        let mut value = patch_value.clone();
        strip_nulls(&mut value);
        target.insert(key.clone(), value);
    }
}

/// 递归删掉对象里的 `null` 成员（C++ `removeNullNodes`：不进入数组）。
/// Recursively drop `null` members of an object (C++ `removeNullNodes`; arrays are
/// not descended into).
fn strip_nulls(value: &mut Value) {
    let Value::Object(object) = value else {
        return;
    };

    let null_keys: Vec<String> = object
        .iter()
        .filter(|(_, child)| child.is_null())
        .map(|(key, _)| key.text().to_string())
        .collect();

    for key in null_keys {
        object.remove(&key);
    }

    for (_, child) in object.iter_mut() {
        strip_nulls(child);
    }
}
