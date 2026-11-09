// SPDX-License-Identifier: MulanPSL-2.0
//! `Value::Object` 的**显式两态载体**（用户 2026-11-05 裁决「方案 2 / B」）。
//! The explicit two-state object carrier: ordered vs unordered is a *value-level choice*,
//! not a global compiler switch.
//!
//! - [`Map::Unordered`] — `BTreeMap`，键按 UTF-8 字节序；`Map::new()` 的默认，保持历史
//!   `to_sml` 输出逐字节不变。
//! - [`Map::Ordered`] — `Box<`[`OrderedMap`]`>`，键序 == 源序；canonical 只接受这一态。
//!
//! 为什么 `Ordered` 套 `Box`：`OrderedMap` 是 40 B，直接内联会让 `Value` 超过
//! `value.rs` 里 `size_of::<Value>() <= 48` 的编译期红线；装箱后本枚举 32 B，
//! `Value` 40 B。多一次堆分配只落在「显式选择保序」的路径上。
//!
//! 默认 `parse()` 仍产出 `Unordered`（历史兼容）。要签 canonical，必须**显式**
//! 走 `parse_ordered()` / `Map::ordered()` / `From<OrderedMap>` —— 这正是「少隐式」：
//! 能不能签，取决于你手里这个对象是哪一态，而不是这台机器开了什么 feature。

use std::collections::BTreeMap;

use crate::object::Object as OrderedMap;
use crate::Value;

/// Which ordering an object carries. ｜ 对象携带的是哪一种键序。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ordering {
    /// Keys in UTF-8 byte order ｜ 键按 UTF-8 字节序。
    Sorted,
    /// Keys in source order ｜ 键按源序。
    Source,
}

/// Backing map of `Value::Object`, with the ordering **explicit in the value**.
/// `Value::Object` 的载体；键序是值的一部分，显式可查。
#[derive(Debug, Clone)]
pub enum Map {
    /// Unordered / sorted map (default; `parse()` legacy behaviour).
    /// 无序（按字节序排序）；默认态，保持历史输出。
    Unordered(BTreeMap<String, Value>),
    /// Source-ordered map (required by canonical encoding).
    /// 保源序；canonical 编码只接受这一态。
    Ordered(Box<OrderedMap>),
}

impl Map {
    /// Default is **unordered**; source order is always an explicit choice
    /// ([`Map::ordered`] / `From<OrderedMap>` / `parse_ordered()`).
    /// 默认无序；要源序必须显式选择。
    ///
    /// **过渡兼容已结束**：`new()` 一律是**无序**态。
    /// `Map::Ordered` 态也不再自动给 `Ordered` —— 那正是「正确性靠编译开关」的最后一个残留。
    /// 要源序请显式 [`Map::ordered`] / `From<OrderedMap>` / `parse_ordered()`。
    pub fn new() -> Self {
        Map::Unordered(BTreeMap::new())
    }

    /// Explicitly **unordered** (UTF-8 byte order). ｜ 显式无序（UTF-8 字节序）。
    ///
    /// 与 [`Map::ordered`] 成对：要哪一态就写哪一个，不靠 `new()` 的默认。
    pub fn unordered() -> Self {
        Map::Unordered(BTreeMap::new())
    }

    /// Explicitly source-ordered. ｜ 显式保源序。
    pub fn ordered() -> Self {
        Map::Ordered(Box::default())
    }

    /// Which ordering this object has. ｜ 这个对象是哪种键序。
    pub fn ordering(&self) -> Ordering {
        match self {
            Map::Unordered(_) => Ordering::Sorted,
            Map::Ordered(_) => Ordering::Source,
        }
    }

    /// Can this object be canonical-encoded? ｜ 能否签 canonical。
    pub fn is_ordered(&self) -> bool {
        matches!(self, Map::Ordered(_))
    }

    pub fn len(&self) -> usize {
        match self {
            Map::Unordered(m) => m.len(),
            Map::Ordered(m) => m.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn capacity(&self) -> usize {
        match self {
            Map::Unordered(m) => m.len(),
            Map::Ordered(m) => m.capacity(),
        }
    }

    pub fn clear(&mut self) {
        match self {
            Map::Unordered(m) => m.clear(),
            Map::Ordered(m) => m.clear(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Map::Unordered(m) => m.get(key),
            Map::Ordered(m) => m.get(key),
        }
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        match self {
            Map::Unordered(m) => m.get_mut(key),
            Map::Ordered(m) => m.get_mut(key),
        }
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Insert; overwriting an existing key keeps its position in the ordered variant.
    /// 插入；保序态下覆盖已有键**不改变位置**（canonical 幂等的前提）。
    pub fn insert(&mut self, key: String, value: Value) -> Option<Value> {
        match self {
            Map::Unordered(m) => m.insert(key, value),
            Map::Ordered(m) => m.insert(key, value),
        }
    }

    pub fn insert_or_merge<F>(&mut self, key: &str, value: Value, merge: F)
    where
        F: FnOnce(&mut Value, Value),
    {
        match self {
            Map::Unordered(m) => {
                if let Some(old) = m.get_mut(key) {
                    merge(old, value);
                } else {
                    m.insert(key.to_string(), value);
                }
            }
            Map::Ordered(m) => m.insert_or_merge(key, value, merge),
        }
    }

    /// Owned-key variant: see [`Object::insert_or_merge_owned`](crate::Object::insert_or_merge_owned).
    /// 接管键所有权的版本：解析器每字段本来就持有 `String`，避免再复制一份。
    pub fn insert_or_merge_owned<F>(&mut self, key: String, value: Value, merge: F)
    where
        F: FnOnce(&str, &mut Value, Value),
    {
        match self {
            Map::Unordered(m) => {
                use std::collections::btree_map::Entry;
                match m.entry(key) {
                    Entry::Occupied(mut e) => {
                        // 重复键是罕见路径；这里克隆一份键名只是为了给 merge 出诊断文案。
                        let owned = e.key().clone();
                        merge(&owned, e.get_mut(), value);
                    }
                    Entry::Vacant(e) => {
                        e.insert(value);
                    }
                }
            }
            Map::Ordered(m) => m.insert_or_merge_owned(key, value, merge),
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        match self {
            Map::Unordered(m) => m.remove(key),
            Map::Ordered(m) => m.remove(key),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> + '_ {
        let (a, b) = match self {
            Map::Unordered(m) => (Some(m.iter().map(|(k, v)| (k.as_str(), v))), None),
            Map::Ordered(m) => (None, Some(m.iter())),
        };
        a.into_iter().flatten().chain(b.into_iter().flatten())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&str, &mut Value)> + '_ {
        let (a, b) = match self {
            Map::Unordered(m) => (Some(m.iter_mut().map(|(k, v)| (k.as_str(), v))), None),
            Map::Ordered(m) => (None, Some(m.iter_mut())),
        };
        a.into_iter().flatten().chain(b.into_iter().flatten())
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> + '_ {
        self.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl Iterator<Item = &Value> + '_ {
        self.iter().map(|(_, v)| v)
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Value> + '_ {
        self.iter_mut().map(|(_, v)| v)
    }

    /// Consume and yield values only — no `String` allocation per key (used by `Drop`).
    /// 只取值、不为每个键再分配 `String`（`Value` 的迭代式 `Drop` 依赖这条）。
    pub fn into_values(self) -> std::vec::IntoIter<Value> {
        let mut out = Vec::with_capacity(self.len());
        match self {
            Map::Unordered(m) => out.extend(m.into_values()),
            Map::Ordered(m) => out.extend(m.into_values()),
        }
        out.into_iter()
    }

    /// Convert this object into the **ordered** carrier, keeping its current key order.
    /// 转成显式有序载体，**保持当前键序**（不声称这就是源序）。
    ///
    /// 用于「值来自 JSON/YAML/程序构造、没有源序概念」的场景：显式承认
    /// 「我要写 canonical」，代价是键序就是手里这份。SML 文本输入应优先用
    /// `parse_ordered()`，它才真的保源序。
    pub fn into_ordered(self) -> Map {
        match self {
            Map::Ordered(_) => self,
            Map::Unordered(bt) => {
                let mut o = OrderedMap::with_capacity(bt.len());
                for (k, v) in bt {
                    o.insert(k, v);
                }
                Map::Ordered(Box::new(o))
            }
        }
    }

    /// Keys in byte order (no-op for the already-sorted variant).
    /// 按字节序排序；对无序态本来就有序，故是空操作。
    pub fn sort_keys(&mut self) {
        if let Map::Ordered(m) = self {
            m.sort_keys();
        }
    }

    /// Order-sensitive equality. ｜ 顺序敏感相等。
    pub fn eq_ordered(&self, other: &Map) -> bool {
        match (self, other) {
            (Map::Ordered(a), Map::Ordered(b)) => a.eq_ordered(b),
            (a, b) => {
                let ak: Vec<&str> = a.keys().collect();
                let bk: Vec<&str> = b.keys().collect();
                ak == bk
                    && a.iter()
                        .zip(b.iter())
                        .all(|((_, x), (_, y))| x.eq_ordered(y))
            }
        }
    }
}

impl Value {
    /// Recursively turn every object into an explicitly ordered one.
    /// 递归把树里每个对象都转成显式有序态（同样保持各自当前键序）。
    pub fn into_ordered(mut self) -> Value {
        self.make_ordered();
        self
    }

    /// In-place variant. `Value` has an iterative `Drop`, so its fields cannot be moved
    /// out of a consumed value (E0509) — mutate through `&mut` instead.
    /// 原地版本：`Value` 带迭代式 `Drop`，不能从消耗的值里移出字段（E0509），故走 `&mut`。
    pub fn make_ordered(&mut self) {
        match self {
            Value::Object(m) => {
                let old = std::mem::take(m);
                let mut ordered = old.into_ordered();
                if let Map::Ordered(ref mut o) = ordered {
                    for (_, v) in o.iter_mut() {
                        v.make_ordered();
                    }
                }
                *m = ordered;
            }
            Value::Array(a) => {
                for v in a.iter_mut() {
                    v.make_ordered();
                }
            }
            _ => {}
        }
    }
}

impl Default for Map {
    fn default() -> Self {
        Map::new()
    }
}

impl PartialEq for Map {
    /// 相等性**按集合**，不含键序（§6.1）。顺序敏感另走 [`Map::eq_ordered`]。
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.iter()
            .all(|(k, v)| other.get(k).is_some_and(|w| v == w))
    }
}

impl From<BTreeMap<String, Value>> for Map {
    fn from(m: BTreeMap<String, Value>) -> Self {
        Map::Unordered(m)
    }
}

impl From<OrderedMap> for Map {
    fn from(m: OrderedMap) -> Self {
        Map::Ordered(Box::new(m))
    }
}

/// `BTreeMap` natively supports `From<[(K, V); N]>`; downstream code uses that form.
/// 下游有 `Map::from([...])` / `.into()` 的写法，这里补上（一律落成**无序**态，
/// 与 `BTreeMap::from` 的排序行为一致）。
impl<const N: usize> From<[(String, Value); N]> for Map {
    fn from(arr: [(String, Value); N]) -> Self {
        // 只有这一个具体 impl，`("a".into(), v)` 才能推断出 `String`
        // （泛型 `K: Into<String>` 会让 `.into()` 的目标类型无法判定）。
        Map::Unordered(BTreeMap::from(arr))
    }
}

impl FromIterator<(String, Value)> for Map {
    fn from_iter<I: IntoIterator<Item = (String, Value)>>(iter: I) -> Self {
        Map::Unordered(BTreeMap::from_iter(iter))
    }
}

impl Extend<(String, Value)> for Map {
    fn extend<I: IntoIterator<Item = (String, Value)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

/// Yields `(&str, &Value)` regardless of which variant is in play.
/// 无论哪一态都产出 `(&str, &Value)`，下游 `for (k, v) in &m` 不用分叉。
pub struct MapIter<'a> {
    inner: Box<dyn Iterator<Item = (&'a str, &'a Value)> + 'a>,
}

impl<'a> Iterator for MapIter<'a> {
    type Item = (&'a str, &'a Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<'a> IntoIterator for &'a Map {
    type Item = (&'a str, &'a Value);
    type IntoIter = MapIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        MapIter {
            inner: Box::new(self.iter()),
        }
    }
}

impl IntoIterator for Map {
    type Item = (String, Value);
    type IntoIter = std::vec::IntoIter<(String, Value)>;

    fn into_iter(self) -> Self::IntoIter {
        let mut out = Vec::with_capacity(self.len());
        match self {
            Map::Unordered(m) => out.extend(m),
            Map::Ordered(m) => {
                for (k, v) in m.iter() {
                    out.push((k.to_string(), v.clone()));
                }
            }
        }
        out.into_iter()
    }
}
