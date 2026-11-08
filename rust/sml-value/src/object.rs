// SPDX-License-Identifier: MulanPSL-2.0
//! 有序对象：`Value::Object` 在 feature `preserve_order` 下的载体。
//!
//! 对应 `docs/swsml/spec/value-layout.md` §3（数据结构）/ §5（三档查找）/ §6（顺序语义）。
//! 2026-11-05 起**无条件编译**（方案 2 第一步）：`OrderedMap` 在默认档也存在，
//! 作为与 `Map` 并列的显式类型。此时 `Value::Object` 的载体仍由 `preserve_order`
//! 别名决定，下一步才切换；行为与历史版本逐字节一致，未开 feature 时不变。
//!
//! # 为什么不用第三方（`IndexMap` 等）
//!
//! 本 crate 的立身之本是**零依赖**（见 `Cargo.toml`）。保序映射的成熟实现
//! 只有 `IndexMap`，引入它等于给地基 crate 背上依赖 —— 故自实现。
//! 代价是要自己守下面几条不变式（每条都有测试钉着）：
//!
//! 1. **键唯一**：`entries` 内无重复 key（重复键在解析期已提升为数组，既有行为）。
//! 2. **索引一致**：`index` 一旦建立必须与 `entries` 完全对应；任何**改变键集合**
//!    或**改变键位置**的操作必须先 `index = OnceLock::new()`。
//!    （覆盖已有键的值**不**动位置，故不必失效索引。）
//! 3. **顺序稳定**：`insert` 追加到尾部；覆盖已有键时**原位替换值**。
//! 4. **键序定义**：排序按 UTF-8 **字节序**（对合法 UTF-8 等价于码点序）。
//! 5. **相等不含序**：`PartialEq` 仍按集合相等（§6.1），顺序敏感另走 `eq_ordered`。

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::Value;

/// 线性扫描档的上界：条目数 ≤ 该值时不建任何索引（§5）。
///
/// 8 的依据：典型配置块 2–6 个字段，8 个以内的线性扫描通常快过任何索引
/// （无哈希计算、无间接跳转，且往往落在同一个 cacheline 内）。
pub(crate) const LINEAR_MAX: usize = 8;

/// 排序下标档的上界：条目数 > 该值才升到哈希档（§5）。
///
/// 排序下标只花 4 B/键，哈希表要 8–16 B；且下标表顺带就是 canonical 顺序。
/// 4096 是「4 B/键 的额外内存（16 KB）与哈希 O(1) 收益」的折中点。
pub(crate) const SORTED_MAX: usize = 4096;

/// 哈希槽位的空标记。条目数达到 `u32::MAX` 前不会与真实下标冲突，
/// 且真到那个规模 `Vec<Entry>` 也早已装不下（1 条目 ≥ 64 B）。
const EMPTY: u32 = u32::MAX;

/// 一个键值对。`key` 用 `Box<str>` 而非 `String`：键一经写入不再增长，
/// 无需 capacity 字段，比 `String` 省 8 B（`Box<str>` 16 B / `String` 24 B）。
struct Entry {
    key: Box<str>,
    value: Value,
}

/// 惰性索引（§5 三档中的后两档）。
///
/// 套一层 `Box` 是为了压 `Object` 的体积：`OnceLock<Box<Index>>` 是 16 B，
/// 若直接 `OnceLock<Index>` 会因 `Index` 自带判别式涨到 24 B，
/// 进而把 `size_of::<Value>()` 顶到 56 B —— 超过 §12 的 48 B 红线。
/// 索引本来就是「首次读才建」的冷路径，多一次解引用无所谓。
enum Index {
    /// 9..=4096 档：按 key 字节序排好的**下标**表（4 B/键），二分查找。
    ///
    /// 用 `Vec` 而不是 `Box<[u32]>`：追加新键时要在中间 `insert` 一个下标，
    /// 定长 slice 只能「整块重新分配 + 拷贝」——那等于每写一个字段就 malloc 一次
    /// （实测 25k 键解析比 BTreeMap 慢 33%，几乎全花在这上面）。
    Sorted(Vec<u32>),
    /// >4096 档：开放寻址哈希表（线性探测），命中后再校验 key。
    Hash(HashIndex),
}

/// 开放寻址哈希表：`slots` 长度恒为 2 的幂，`EMPTY` 表空位。
struct HashIndex {
    slots: Vec<u32>,
}

impl HashIndex {
    /// 建表：容量取 ≥ 2n 的最小 2 的幂（装载因子 ≈ 0.5 ⇒ 线性探测平均 ~1.5 次）。
    fn build(entries: &[Entry]) -> Self {
        let cap = (entries.len() * 2).next_power_of_two().max(2);
        let mask = cap - 1;
        let mut slots = vec![EMPTY; cap];
        for (i, e) in entries.iter().enumerate() {
            let mut j = (hash_key(&e.key) as usize) & mask;
            loop {
                if slots[j] == EMPTY {
                    slots[j] = i as u32;
                    break;
                }
                j = (j + 1) & mask;
            }
        }
        HashIndex { slots }
    }

    #[inline]
    fn get(&self, entries: &[Entry], key: &str) -> Option<usize> {
        let mask = self.slots.len() - 1;
        let mut j = (hash_key(key) as usize) & mask;
        loop {
            let s = self.slots[j];
            if s == EMPTY {
                return None;
            }
            if &*entries[s as usize].key == key {
                return Some(s as usize);
            }
            j = (j + 1) & mask;
        }
    }

    /// 增量插入一个**新**条目（调用方保证 `slot` 尚未入表、且容量够）。
    #[inline]
    fn insert(&mut self, entries: &[Entry], slot: usize) {
        let mask = self.slots.len() - 1;
        let mut j = (hash_key(&entries[slot].key) as usize) & mask;
        loop {
            if self.slots[j] == EMPTY {
                self.slots[j] = slot as u32;
                return;
            }
            j = (j + 1) & mask;
        }
    }
}

/// 在排序下标表 `slots` 里二分 `key`：返回「应插入的位置」（第一个 ≥ key 的下标）。
///
/// 抽成自由函数：`position_of`（查找）与 `index_insert`（增量维护）都要它，
/// 两处各写一遍二分最容易在边界上分叉（越界 / 差一）。
fn sorted_lower_bound(entries: &[Entry], slots: &[u32], key: &str) -> usize {
    let mut lo = 0usize;
    let mut hi = slots.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        if entries[slots[mid] as usize].key.as_bytes() < key.as_bytes() {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// FNV-1a 64：确定性、无种子、无 DoS 面。
///
/// 不用 `std::collections::hash_map::DefaultHasher`：它是 SipHash，
/// 在本档规模下比 FNV 慢，而我们需要的只是「分布够均匀」——
/// 真发生碰撞也只退化成线性探测，正确性由「命中后比对 key」保证。
#[inline]
pub(crate) fn hash_key(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Order-preserving object: insertion order == source order; lookups go through a lazy index. ｜ 保序对象：插入序 == 源序，查找靠惰性索引。
pub struct Object {
    entries: Vec<Entry>,
    /// 惰性：首次 `get` 才建；键集合或键位置一变就整体作废（不变式 2）。
    index: OnceLock<Box<Index>>,
}

impl Object {
    /// Empty object; no entry buffer and no index are allocated yet ｜ 新建空对象：不预分配条目、不建索引。
    #[inline]
    pub fn new() -> Self {
        Object { entries: Vec::new(), index: OnceLock::new() }
    }

    /// Empty object whose entry buffer is pre-allocated for `n` keys ｜ 新建空对象，条目缓冲区按 `n` 个键预分配。
    #[inline]
    pub fn with_capacity(n: usize) -> Self {
        Object { entries: Vec::with_capacity(n), index: OnceLock::new() }
    }

    /// Number of entries; duplicate keys are already merged, so this equals the visible key count ｜ 条目数；重名键早已合并，故等于可见键数。
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there is no entry at all ｜ 是否一个条目都没有。
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Allocated capacity of the entry buffer (≥ [`len`](Self::len)) ｜ 条目缓冲区已分配容量（≥ [`len`](Self::len)）。
    #[inline]
    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }

    /// Drop all entries **and** the lazily built index ｜ 清空全部条目，并丢弃惰性索引。
    pub fn clear(&mut self) {
        self.entries.clear();
        self.index = OnceLock::new();
    }

    /// 建索引（§5 中间档 / 大档）：覆盖**当前全部**条目。
    ///
    /// ⚠️ 索引一旦建好就必须与 `entries` 同长 —— 追加新键时要走
    /// [`Self::index_insert`] **增量维护**，而不是整体作废重建。
    /// 早先的做法（每次追加都 `index = OnceLock::new()`）会让「建一个 n 键对象」
    /// 退化成 O(n² log n)：解析器每写一个字段都要先查一次重名，于是每追加一个键
    /// 就重建一次全表 —— 实测 9000 键要 55 秒。这一条是性能红线，不是风格问题。
    fn build_index(&self) -> Index {
        if self.entries.len() > SORTED_MAX {
            Index::Hash(HashIndex::build(&self.entries))
        } else {
            // —— 中对象：排序下标 ——
            let mut slots: Vec<u32> = (0..self.entries.len() as u32).collect();
            slots.sort_unstable_by(|&a, &b| {
                self.entries[a as usize]
                    .key
                    .as_bytes()
                    .cmp(self.entries[b as usize].key.as_bytes())
            });
            Index::Sorted(slots)
        }
    }

    /// 追加一个条目后增量维护索引（若索引尚未建立则什么都不做）。
    ///
    /// - 排序档：二分定位后 `Vec::insert`（搬的是 4 B 的 u32，4096 档内累计不过
    ///   几十 MB memcpy，可忽略）；越过 `SORTED_MAX` 就升级成哈希档。
    /// - 哈希档：一次探测插入；装载因子超过 0.5 就翻倍重建（摊还 O(1)）。
    fn index_insert(&mut self, slot: usize) {
        let Some(idx) = self.index.get_mut() else {
            return; // 还没建索引：小对象走线性扫描，等首次读时再一次性建
        };
        let n = self.entries.len();
        match &mut **idx {
            Index::Sorted(slots) => {
                if n > SORTED_MAX {
                    **idx = Index::Hash(HashIndex::build(&self.entries));
                    return;
                }
                let pos = sorted_lower_bound(&self.entries, slots, &self.entries[slot].key);
                slots.insert(pos, slot as u32);
            }
            Index::Hash(h) => {
                if n * 2 > h.slots.len() {
                    *h = HashIndex::build(&self.entries);
                    return;
                }
                h.insert(&self.entries, slot);
            }
        }
    }

    /// 键 → `entries` 下标。三档入口（§5）。
    fn position_of(&self, key: &str) -> Option<usize> {
        // ≤8 且未建索引：线性扫描（解析路径之外最常见的形状，零额外内存）。
        if self.index.get().is_none() && self.entries.len() <= LINEAR_MAX {
            return self.entries.iter().position(|e| &*e.key == key);
        }
        let idx = self.index.get_or_init(|| Box::new(self.build_index()));
        match &**idx {
            Index::Sorted(slots) => {
                let p = sorted_lower_bound(&self.entries, slots, key);
                if p < slots.len() && &*self.entries[slots[p] as usize].key == key {
                    Some(slots[p] as usize)
                } else {
                    None
                }
            }
            Index::Hash(h) => h.get(&self.entries, key),
        }
    }

    /// Look up a key; the index is built lazily on the first lookup that needs it (§5) ｜ 按键取值；需要时才在首次查找中惰性建索引（§5）。
    #[inline]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.position_of(key).map(|i| &self.entries[i].value)
    }

    /// Look up a key for in-place mutation; position is resolved **before** the `&mut` is taken ｜ 按键取可变引用；先解析出下标再取 `&mut`，两次借用不重叠。
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        // 先算下标再取 `&mut`：不能让 `position_of` 的 `&self` 借用到赋值之后。
        let i = self.position_of(key)?;
        Some(&mut self.entries[i].value)
    }

    /// Whether the key exists (same lookup path as [`get`](Self::get)) ｜ 键是否存在（与 [`get`](Self::get) 同一条查找路径）。
    #[inline]
    pub fn contains_key(&self, key: &str) -> bool {
        self.position_of(key).is_some()
    }

    /// Write: existing key ⇒ **value replaced in place, position unchanged** (§6.3); new key ⇒ appended to the tail. ｜ 写入：键已存在 ⇒ **原位替换值、位置不动**（§6.3）；不存在 ⇒ 追加到尾部。
    ///
    /// 覆盖路径**不**作废索引（键集合与位置都没变），这是刻意的设计：
    /// 「改一个已有字段」是配置场景的高频操作，不该让整张索引重建。
    pub fn insert(&mut self, key: String, value: Value) -> Option<Value> {
        if let Some(i) = self.position_of(&key) {
            return Some(std::mem::replace(&mut self.entries[i].value, value));
        }
        let slot = self.entries.len();
        self.entries.push(Entry { key: key.into_boxed_str(), value });
        self.index_insert(slot);
        None
    }

    /// "Append or merge" in a single lookup — the parser's dedicated field-write path. ｜ 一次查找完成「追加 or 合并」—— 解析器的字段写入专用法。
    ///
    /// - 键不存在 ⇒ 追加到尾部，**不**调用 `merge`；
    /// - 键已存在 ⇒ **不动位置**，把旧值与 `value` 一起交给 `merge` 处理。
    ///
    /// 为什么要有它：解析每个字段都要先「查重名」再「写入」，写成
    /// `get_mut(key)` + `insert(key, v)` 是**两次查找**（`get_mut` 一次、`insert`
    /// 又一次）—— 实测这让 5k 键解析比 `BTreeMap` 慢约 30%。`BTreeMap` 的
    /// `insert` 只查一次，故保序实现也必须只查一次才谈得上「不比现状慢」。
    ///
    /// `key` 借 `&str` 传入：只有真的要追加时才 `to_string()` 分配，
    /// 重复键（罕见）路径上一次分配都不花。
    pub fn insert_or_merge<F>(&mut self, key: &str, value: Value, merge: F)
    where
        F: FnOnce(&mut Value, Value),
    {
        if let Some(i) = self.position_of(key) {
            merge(&mut self.entries[i].value, value);
            return;
        }
        let slot = self.entries.len();
        self.entries.push(Entry { key: key.to_string().into_boxed_str(), value });
        self.index_insert(slot);
    }

    /// Same as [`Object::insert_or_merge`], but **takes ownership of the key**.
    /// 同 [`Object::insert_or_merge`]，但**接管键的所有权**。
    ///
    /// Parser already owns a `String` for every field; the borrowed variant had to copy it
    /// into a `Box<str>` (`to_string()` + `into_boxed_str()` = one copy and often a realloc).
    /// Here the buffer is converted directly. `merge` also receives the stored key so the
    /// caller can build diagnostics without keeping a second copy alive.
    /// ｜ 解析器本来就持有 `String`：借用版要复制一遍，这里直接转 `Box<str>`。
    pub fn insert_or_merge_owned<F>(&mut self, key: String, value: Value, merge: F)
    where
        F: FnOnce(&str, &mut Value, Value),
    {
        if let Some(i) = self.position_of(&key) {
            let e = &mut self.entries[i];
            merge(&e.key, &mut e.value, value);
            return;
        }
        let slot = self.entries.len();
        self.entries.push(Entry { key: key.into_boxed_str(), value });
        self.index_insert(slot);
    }

    /// Remove a key and return its value; later keys shift left, so the whole index is discarded ｜ 删键并返回其值；其后的键位置前移，故整张索引作废。
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let i = self.position_of(key)?;
        let e = self.entries.remove(i);
        self.index = OnceLock::new();
        Some(e.value)
    }

    /// Iterate in **source order** (the reference order for `to_sml` and the C-ABI iteration order, §8). ｜ 按**源序**迭代（`to_sml` / C-ABI 迭代序都以此为准，§8）。
    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        Iter { inner: self.entries.iter() }
    }

    /// Iterate `(key, &mut value)` in **source order**; keys themselves are immutable ｜ 按**源序**迭代 `(键, &mut 值)`；键本身不可改。
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_> {
        IterMut { inner: self.entries.iter_mut() }
    }

    /// Iterate keys in **source order** ｜ 按**源序**迭代键。
    #[inline]
    pub fn keys(&self) -> Keys<'_> {
        Keys { inner: self.entries.iter() }
    }

    /// Iterate values in **source order** ｜ 按**源序**迭代值。
    #[inline]
    pub fn values(&self) -> Values<'_> {
        Values { inner: self.entries.iter() }
    }

    /// Iterate values mutably in **source order** ｜ 按**源序**迭代可变值。
    #[inline]
    pub fn values_mut(&mut self) -> ValuesMut<'_> {
        ValuesMut { inner: self.entries.iter_mut() }
    }

    /// **Consuming** iterator over values only: used by `Value::drop` to avoid rebuilding a `String` per key. ｜ 仅取值的**消费**迭代器：给 `Value::drop` 用，避免为键再造一份 `String`。
    #[inline]
    pub fn into_values(self) -> IntoValues {
        IntoValues { inner: self.entries.into_iter() }
    }

    /// Reorder in place by key UTF-8 byte order (for canonical output, §6.2). ｜ 原地按键的 UTF-8 字节序重排（canonical 输出用，§6.2）。
    /// 重排会改变位置 ⇒ 索引作废。
    pub fn sort_keys(&mut self) {
        self.entries.sort_by(|a, b| a.key.as_bytes().cmp(b.key.as_bytes()));
        self.index = OnceLock::new();
    }

    /// Order-sensitive equality (§6.1): `PartialEq` compares as a set, this one requires same order **and** equal values. ｜ 顺序敏感相等（§6.1）：`PartialEq` 是集合相等，这个是「同序且相等」。
    pub fn eq_ordered(&self, other: &Object) -> bool {
        self.entries.len() == other.entries.len()
            && self
                .entries
                .iter()
                .zip(other.entries.iter())
                .all(|(a, b)| a.key == b.key && a.value == b.value)
    }
}

impl Default for Object {
    #[inline]
    fn default() -> Self {
        Object::new()
    }
}

impl Clone for Object {
    /// **不**克隆索引：它是纯派生数据，且克隆后立刻作废更省事
    /// （`entries` 顺序一致，克隆体其实可以继续用同一张表，但 `OnceLock`
    /// 无法从 `&self` 搬出，重建一次的成本与克隆一份相当）。
    fn clone(&self) -> Self {
        Object {
            entries: self
                .entries
                .iter()
                .map(|e| Entry { key: e.key.clone(), value: e.value.clone() })
                .collect(),
            index: OnceLock::new(),
        }
    }
}

impl std::fmt::Debug for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 与 `BTreeMap` 的 `Debug` 同形（`{"k": v}`），便于既有调试输出/快照对齐
        f.write_str("{")?;
        for (i, e) in self.entries.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{:?}: {:?}", &*e.key, e.value)?;
        }
        f.write_str("}")
    }
}

/// 相等性**按集合**（§6.1）：键序不参与比较。
///
/// 为什么不让 `PartialEq` 含顺序：所有 golden、契约校验、去重逻辑都按
/// 「map 相等」写；改语义会一次性炸掉一大片，而收益只是少一个方法名。
/// 需要顺序敏感时用 [`Object::eq_ordered`]。
impl PartialEq for Object {
    fn eq(&self, other: &Object) -> bool {
        if self.entries.len() != other.entries.len() {
            return false;
        }
        if std::ptr::eq(self, other) {
            return true;
        }
        for e in &self.entries {
            match other.get(&e.key) {
                Some(v) if v == &e.value => {}
                _ => return false,
            }
        }
        true
    }
}

// —— 迭代器：一律按 `entries` 顺序（== 源序）——
//
// 写成具名结构体而非 `impl Iterator`：要给 `&Object` 实现 `IntoIterator`
// （`for (k, v) in &map` 是既有代码最普遍的写法），那需要一个可命名的类型。

/// `(&str, &Value)` iterator in source order ｜ `(&str, &Value)` 迭代器（源序）。
pub struct Iter<'a> {
    inner: std::slice::Iter<'a, Entry>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a str, &'a Value);
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|e| (&*e.key, &e.value))
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a> ExactSizeIterator for Iter<'a> {}

/// `(&str, &mut Value)` iterator in source order ｜ `(&str, &mut Value)` 迭代器（源序）。
pub struct IterMut<'a> {
    inner: std::slice::IterMut<'a, Entry>,
}

impl<'a> Iterator for IterMut<'a> {
    type Item = (&'a str, &'a mut Value);
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|e| (&*e.key, &mut e.value))
    }
}

/// Key iterator in source order ｜ 键迭代器（源序）。
pub struct Keys<'a> {
    inner: std::slice::Iter<'a, Entry>,
}

impl<'a> Iterator for Keys<'a> {
    type Item = &'a str;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|e| &*e.key)
    }
}

/// Value iterator in source order ｜ 值迭代器（源序）。
pub struct Values<'a> {
    inner: std::slice::Iter<'a, Entry>,
}

impl<'a> Iterator for Values<'a> {
    type Item = &'a Value;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|e| &e.value)
    }
}

/// Mutable value iterator in source order ｜ 值可变迭代器（源序）。
pub struct ValuesMut<'a> {
    inner: std::slice::IterMut<'a, Entry>,
}

impl<'a> Iterator for ValuesMut<'a> {
    type Item = &'a mut Value;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|e| &mut e.value)
    }
}

/// Consuming iterator that yields **values only** — avoids rebuilding key `String`s; used by `Value::drop` ｜ 只产值的**消费**迭代器。
pub struct IntoValues {
    inner: std::vec::IntoIter<Entry>,
}

impl Iterator for IntoValues {
    type Item = Value;
    #[inline]
    fn next(&mut self) -> Option<Value> {
        self.inner.next().map(|e| e.value)
    }
}

impl<'a> IntoIterator for &'a Object {
    type Item = (&'a str, &'a Value);
    type IntoIter = Iter<'a>;
    #[inline]
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

/// Consuming iterator yielding `(String, Value)` — same shape as `BTreeMap`, so existing code ports over unchanged ｜ 消费迭代产出 `(String, Value)`。
pub struct IntoIter {
    inner: std::vec::IntoIter<Entry>,
}

impl Iterator for IntoIter {
    type Item = (String, Value);
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|e| (e.key.to_string(), e.value))
    }
}

impl IntoIterator for Object {
    type Item = (String, Value);
    type IntoIter = IntoIter;
    #[inline]
    fn into_iter(self) -> IntoIter {
        IntoIter { inner: self.entries.into_iter() }
    }
}

// —— 与 BTreeMap 的互转：给「`Map::Unordered` ↔ `Map::Ordered`」两种态的测试用 ——

impl From<BTreeMap<String, Value>> for Object {
    fn from(m: BTreeMap<String, Value>) -> Self {
        Object {
            entries: m
                .into_iter()
                .map(|(k, v)| Entry { key: k.into_boxed_str(), value: v })
                .collect(),
            index: OnceLock::new(),
        }
    }
}

impl From<Object> for BTreeMap<String, Value> {
    fn from(o: Object) -> Self {
        // `Box<str>` → `String`：走 `str::to_string`（转换路径只在互转时用，
        // 不在热路径上，故不纠结能否复用分配）。
        o.entries.into_iter().map(|e| (e.key.to_string(), e.value)).collect()
    }
}

impl<const N: usize> From<[(String, Value); N]> for Object {
    fn from(arr: [(String, Value); N]) -> Self {
        let mut o = Object::with_capacity(N);
        for (k, v) in arr {
            o.insert(k, v);
        }
        o
    }
}

impl FromIterator<(String, Value)> for Object {
    fn from_iter<I: IntoIterator<Item = (String, Value)>>(iter: I) -> Self {
        let it = iter.into_iter();
        let mut o = Object::with_capacity(it.size_hint().0);
        for (k, v) in it {
            o.insert(k, v);
        }
        o
    }
}

impl Extend<(String, Value)> for Object {
    /// 与 §6.3 同一套规则（已存在原位替换 / 不存在追加尾部），
    /// 故 `&frag` 拼接与 include 合并直接复用它。
    fn extend<I: IntoIterator<Item = (String, Value)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(pairs: &[(&str, Value)]) -> Object {
        let mut o = Object::new();
        for (k, v) in pairs {
            o.insert((*k).to_string(), v.clone());
        }
        o
    }

    #[test]
    fn preserves_insertion_order() {
        let o = obj(&[
            ("z", Value::Int(1)),
            ("a", Value::Int(2)),
            ("m", Value::Int(3)),
        ]);
        let keys: Vec<&str> = o.keys().collect();
        assert_eq!(keys, vec!["z", "a", "m"]);
    }

    #[test]
    fn overwrite_keeps_position() {
        let mut o = obj(&[("a", Value::Int(1)), ("b", Value::Int(2))]);
        assert_eq!(o.insert("a".to_string(), Value::Int(9)), Some(Value::Int(1)));
        let keys: Vec<&str> = o.keys().collect();
        assert_eq!(keys, vec!["a", "b"], "覆盖必须原位，不能移到尾部");
        assert_eq!(o.get("a"), Some(&Value::Int(9)));
    }

    #[test]
    fn append_goes_to_tail() {
        let mut o = obj(&[("a", Value::Int(1))]);
        assert_eq!(o.insert("b".to_string(), Value::Int(2)), None);
        assert_eq!(o.keys().collect::<Vec<_>>(), vec!["a", "b"]);
    }

    /// 三档都要测：≤8 线性 / 9..=4096 排序下标 / >4096 哈希。
    /// 只测中间档是这类结构最典型的漏测 —— 阈值写错时小规模全绿。
    #[test]
    fn all_three_tiers_lookup_correctly() {
        for n in [0usize, 1, 8, 9, 64, 4096, 4097, 9000] {
            let mut o = Object::with_capacity(n);
            for i in 0..n {
                o.insert(format!("k{i}"), Value::Int(i as i64));
            }
            assert_eq!(o.len(), n);
            for i in 0..n {
                assert_eq!(o.get(&format!("k{i}")), Some(&Value::Int(i as i64)), "n={n} i={i}");
            }
            assert!(o.get("nope").is_none(), "n={n}");
        }
    }

    /// 索引一旦建立，后续的「覆盖值」不得让它失效（不变式 2 的例外分支）。
    #[test]
    fn index_stays_valid_across_overwrite() {
        let mut o = Object::new();
        for i in 0..100 {
            o.insert(format!("k{i}"), Value::Int(i));
        }
        assert_eq!(o.get("k50"), Some(&Value::Int(50))); // 触发建索引
        o.insert("k50".to_string(), Value::Int(-50));
        assert_eq!(o.get("k50"), Some(&Value::Int(-50)));
        // 新增键会让索引作废，但查找结果仍须正确
        o.insert("new".to_string(), Value::Int(1));
        assert_eq!(o.get("new"), Some(&Value::Int(1)));
        assert_eq!(o.get("k99"), Some(&Value::Int(99)));
    }

    #[test]
    fn remove_invalidates_index() {
        let mut o = Object::new();
        for i in 0..100 {
            o.insert(format!("k{i}"), Value::Int(i));
        }
        assert_eq!(o.get("k10"), Some(&Value::Int(10)));
        assert_eq!(o.remove("k10"), Some(Value::Int(10)));
        assert!(o.get("k10").is_none());
        assert_eq!(o.get("k11"), Some(&Value::Int(11)));
        assert_eq!(o.len(), 99);
    }

    /// §6.1：相等按集合，不含顺序。
    #[test]
    fn eq_is_order_insensitive() {
        let a = obj(&[("a", Value::Int(1)), ("b", Value::Int(2))]);
        let b = obj(&[("b", Value::Int(2)), ("a", Value::Int(1))]);
        assert_eq!(a, b);
        assert!(!a.eq_ordered(&b));
        assert!(a.eq_ordered(&a.clone()));
    }

    #[test]
    fn extend_follows_merge_rule() {
        let mut a = obj(&[("x", Value::Int(1)), ("y", Value::Int(2))]);
        let b = obj(&[("y", Value::Int(9)), ("z", Value::Int(3))]);
        a.extend(b);
        assert_eq!(a.keys().collect::<Vec<_>>(), vec!["x", "y", "z"]);
        assert_eq!(a.get("y"), Some(&Value::Int(9)));
    }

    /// 排序按 UTF-8 字节序，不受 locale 影响（§4.4 / 不变式 4）。
    #[test]
    fn sort_keys_is_byte_order() {
        let mut o = obj(&[("b", Value::Null), ("A", Value::Null), ("a", Value::Null)]);
        o.sort_keys();
        assert_eq!(o.keys().collect::<Vec<_>>(), vec!["A", "a", "b"]);
    }

    #[test]
    fn into_values_drops_keys_without_allocating() {
        let o = obj(&[("a", Value::Int(1)), ("b", Value::Int(2))]);
        let vs: Vec<Value> = o.into_values().collect();
        assert_eq!(vs, vec![Value::Int(1), Value::Int(2)]);
    }
}