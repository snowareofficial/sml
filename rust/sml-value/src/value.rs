use std::collections::BTreeMap;
// `fmt` 只在 `sml` 特性下用到（`impl fmt::Display`）⇒ 关掉该特性时必须一起 cfg 掉，
// 否则每个依赖 sml-value 且未开 `sml` 的 crate 都会背一条 `unused import` 警告
// （实测：sml-include / sml-parse / swsml / smltools 全会各报一条 —— 噪音掩盖真警告）。
#[cfg(feature = "sml")]
use std::fmt;
#[cfg(feature = "sml")]
use crate::dump::to_sml;
#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    /// (数值, 原始字面量文本)。
    ///
    /// 第二个字段**仅由解析器**在解析时填充（如 `1e10` → `Some("1e10")`），
    /// 用于让 dump 还原书写形式 —— f64 无法表达「作者写的是科学计数法」，
    /// 一旦解析成 f64，原始写法信息就永久丢失（P3）。
    ///
    /// **失效规则**：程序构造一律 `None`；任何产生新 Float 的运算都丢弃 raw
    /// （f64 运算本身也无法保持 raw 语义）。规则保持简单，就不会出现
    /// 「值已被改动、写法还是旧的」这种比丢失更难排查的不一致。
    ///
    /// raw **不参与相等比较**（见下方 PartialEq）：相等性属「值」层面，
    /// 写法只是附加信息。
    Float(f64, Option<String>),
    Str(String),
    Array(Vec<Value>),
    /// 对象/块; `__type` / `__name` 裸块元数据以保留字键存放
    Object(BTreeMap<String, Value>),
}

impl Value {
    /// 构造无 raw 的浮点值 —— **程序构造路径统一用这个**。
    #[inline]
    pub fn float(f: f64) -> Self {
        Value::Float(f, None)
    }

    /// 构造带原始字面量的浮点值 —— **仅解析器使用**。
    #[inline]
    pub fn float_with_raw(f: f64, raw: impl Into<String>) -> Self {
        Value::Float(f, Some(raw.into()))
    }

    /// 取浮点数值；非 Float 返回 None。
    #[inline]
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f, _) => Some(*f),
            _ => None,
        }
    }

    /// 取原始字面量文本 —— 仅当值自解析后未被改动时有值。
    #[inline]
    pub fn float_raw(&self) -> Option<&str> {
        match self {
            Value::Float(_, raw) => raw.as_deref(),
            _ => None,
        }
    }
}

impl PartialEq for Value {
    /// 相等性只比较「值」，**不比较 raw**。
    ///
    /// 若让 raw 参与比较，`parse("x: 1e10") == Value::float(1e10)` 会因
    /// `Some("1e10") != None` 判定为不等 —— 这会让所有按数值断言的测试失效，
    /// 语义上也不成立：写法信息不应影响值的相等性。
    ///
    /// 注：f64 的 `==` 保持原语义（NaN != NaN）。
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a, _), Value::Float(b, _)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => a == b,
            _ => false,
        }
    }
}

impl Drop for Value {
    /// 迭代式析构，避免深层嵌套 Value 触发递归 Drop 的栈溢出。
    ///
    /// 编译器为递归枚举自动生成的 Drop 会沿嵌套深度递归调用（N 层嵌套 = N 层栈帧），
    /// 50000 层嵌套即 50000 层栈帧，直接栈溢出（STATUS_STACK_OVERFLOW）abort 宿主进程。
    ///
    /// 这里用显式工作栈（堆上 `Vec`）逐层展开，全程原生栈深度为常数：
    /// 1. `mem::replace(self, Null)` 把 `*self` 改为空壳并取回原值 `owned`（安全 move-out，
    ///    不受 Drop 限制）；实现 `Drop` 后编译器不再对 `*self` 生成自动 drop glue，无重复释放。
    /// 2. 用 `mem::take` 把 `owned`/各待展开节点的复合字段（Vec/BTreeMap）整体移出到局部
    ///    （`Vec`/`BTreeMap` 本身无 Drop 约束，可安全迭代 move 出子节点推入工作栈），
    ///    标量节点直接 `drop`。绝不递归调用 `Value::drop`，工作栈在函数末尾已空，drop 为常数开销。
    fn drop(&mut self) {
        // 把 *self 替换为 Null 并取回原值；此后 *self 为 Null，不会二次释放。
        let mut owned = std::mem::replace(self, Value::Null);
        let mut stack: Vec<Value> = Vec::new();

        // 展开最外层：把复合字段整体 take 出来，逐个子节点推入工作栈。
        if let Value::Array(ref mut a) = owned {
            let taken = std::mem::take(a); // a 现为 Vec::default()（空），taken 为原 Vec
            for e in taken {
                stack.push(e);
            }
        } else if let Value::Object(ref mut m) = owned {
            let taken = std::mem::take(m);
            for (_k, v) in taken {
                stack.push(v);
            }
        } else {
            // 标量（Null/Bool/Int/Float/Str）：仅 Str 与 Float 的 raw 持有堆资源
            // （String），手动释放后丢弃，避免自动 drop 再次进入本 drop 造成无限递归。
            if let Value::Str(ref mut s) = owned {
                let s = std::mem::take(s); // 取回 String 所有权，owned 的 String 置空
                drop(s); // 释放 String 堆内存
            }
            // Float 的 raw（Option<String>）同样持有堆资源。此处若漏掉，
            // 由于下面 forget 了 owned，每个带 raw 的 Float 都会泄漏一块堆内存。
            if let Value::Float(_, ref mut raw) = owned {
                let r = std::mem::take(raw);
                drop(r);
            }
            // 其余标量无堆资源；forget 阻止 owned 的自动（递归）drop。
            std::mem::forget(owned);
            return;
        }
        // owned 此时为 Array/Object 但字段已被 take 为空壳；禁止其自动 drop（否则会
        // 再次进入本 drop 造成递归），以 forget 丢弃空壳（空壳无堆资源，安全）。
        std::mem::forget(owned);

        // 迭代展开工作栈：每次只处理一个节点的直接子节点，绝不递归 Drop。
        while let Some(mut cur) = stack.pop() {
            if let Value::Array(ref mut a) = cur {
                let taken = std::mem::take(a);
                for e in taken {
                    stack.push(e);
                }
            } else if let Value::Object(ref mut m) = cur {
                let taken = std::mem::take(m);
                for (_k, v) in taken {
                    stack.push(v);
                }
            } else if let Value::Str(ref mut s) = cur {
                // 标量 Str：释放 String 堆内存，避免 forget 造成内存泄漏
                let s = std::mem::take(s);
                drop(s);
            } else if let Value::Float(_, ref mut raw) = cur {
                // Float 的 raw 同样持有堆资源，必须与 Str 一并释放
                let r = std::mem::take(raw);
                drop(r);
            }
            // cur 此时已为空壳（数组/对象的字段被 take，或标量资源已释放）；
            // forget 阻止其自动（递归）drop，无堆资源泄漏。
            std::mem::forget(cur);
        }
        // stack 此时为空，其 drop 为常数开销（释放空 Vec）。
    }
}

/// 值嵌套深度上限（块 / 数组的递归层数）。
///
/// 不可信输入可构造任意深嵌套，不设上限会在递归解析/求值时栈溢出
/// （abort 宿主进程，无法被 `catch_unwind` 捕获）。故所有递归入口都以此为界。
pub const MAX_VALUE_DEPTH: usize = 128;

/// 描述值的类型，用于错误信息。
///
/// 放在本 crate 而非 derive 层：`sml-value` 的 serde 桥接需要在类型不匹配时
/// 生成同样的描述文案，若从 derive crate 取会造成 value → derive 的反向依赖。
pub fn describe_value(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f, _) => f.to_string(),
        Value::Str(s) => format!("字符串 `{s}`"),
        Value::Array(a) => format!("数组（{} 个元素）", a.len()),
        Value::Object(o) => format!("块（{} 个键）", o.len()),
    }
}

impl Value {
    /// 对象字段按需取（支持 "." 点路径；键后可跟 `[idx]` 数组下标，可链式）。
    ///
    /// 例如 `"a.b[0].c"`：先取对象 `a` 的字段 `b`，再取下标 0 的数组元素，最后取其字段 `c`。
    /// 下标越界、下标非数字、或下标作用于非数组，均返回 None；点路径语义与旧版一致。
    ///
    /// ⚠️ **字面键名优先**：先按整段原文查一遍键，命中就用它。故键名本身含 `[` 的文档
    /// （如键就叫 `a[0]`）**不受**下标语法影响 —— 新增的下标能力只在「无精确键匹配」时生效，
    /// 不会把既有文档从 `Some` 变成 `None`（回归测试：`bracket_in_key_name_is_not_rewritten`）。
    ///
    /// # 示例
    /// ```
    /// use sml_value::Value;
    /// use std::collections::BTreeMap;
    /// let doc = Value::Object(BTreeMap::from([
    ///     ("a".into(), Value::Object(BTreeMap::from([
    ///         ("b".into(), Value::Array(vec![Value::Int(10), Value::Int(20)])),
    ///     ]))),
    /// ]));
    /// assert_eq!(doc.get("a.b[0]").and_then(|x| x.as_i64()), Some(10));
    /// assert_eq!(doc.get("a.b[1]").and_then(|x| x.as_i64()), Some(20));
    /// // 越界 / 类型不符返回 None，不 panic
    /// assert!(doc.get("a.b[9]").is_none());
    /// ```
    pub fn get(&self, path: &str) -> Option<&Value> {
        let mut cur = self;
        for raw in path.split('.') {
            if raw.is_empty() {
                // 形如 "a." 的尾随点 / 起始点：旧版对空键查不到，返回 None
                return None;
            }
            // 兼容优先：字面键名命中就直接用它 —— 键名本身含 `[` 的文档
            // （如键就叫 `a[0]`）不会因新增的下标语法而从 Some 变 None。
            if let Value::Object(m) = cur {
                if let Some(v) = m.get(raw) {
                    cur = v;
                    continue;
                }
            }
            // 拆出键名与可能的下标序列（键名可能为空，紧跟 "." 后的 "[" 时）
            let (key, idxs) = match raw.split_once('[') {
                None => (raw, None),
                Some((k, r)) => (k, Some(r)),
            };
            if !key.is_empty() {
                cur = match cur {
                    Value::Object(m) => m.get(key)?,
                    _ => return None,
                };
            }
            if let Some(idxs) = idxs {
                for part in idxs.split('[') {
                    let n = part.strip_suffix(']')?;
                    let n: usize = n.parse().ok()?;
                    cur = match cur {
                        Value::Array(a) => a.get(n)?,
                        _ => return None,
                    };
                }
            }
        }
        Some(cur)
    }
    /// 字符串视图 (字符串直接返回; 其它返回 None)
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// 取对象引用；非 Object 返回 None。
    ///
    /// 返回的是对象内部 `BTreeMap` 的引用，可直接使用其 `iter` / `len` /
    /// `is_empty` 等能力，无需额外封装。
    ///
    /// # 示例
    /// ```
    /// use sml_value::Value;
    /// let v = Value::Object(Default::default());
    /// assert!(v.as_object().is_some());
    /// // 非对象一律返回 None，不会 panic
    /// let s = Value::Str("x".into());
    /// assert!(s.as_object().is_none());
    /// ```
    #[inline]
    pub fn as_object(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Value::Object(m) => Some(m),
            _ => None,
        }
    }

    /// 取数组引用；非 Array 返回 None。
    ///
    /// # 示例
    /// ```
    /// use sml_value::Value;
    /// let v = Value::Array(vec![]);
    /// assert_eq!(v.as_array().map(|a| a.len()), Some(0));
    /// let s = Value::Str("x".into());
    /// assert!(s.as_array().is_none());
    /// ```
    #[inline]
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// 取整数值；非 Int 返回 None。Float 不会被当作整数（不静默降级）。
    ///
    /// # 示例
    /// ```
    /// use sml_value::Value;
    /// let v = Value::Int(7);
    /// assert_eq!(v.as_i64(), Some(7));
    /// // 浮点即使为整数值也不降级为 i64
    /// let f = Value::float(7.0);
    /// assert_eq!(f.as_i64(), None);
    /// ```
    #[inline]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// 取布尔值；非 Bool 返回 None。
    ///
    /// # 示例
    /// ```
    /// use sml_value::Value;
    /// let v = Value::Bool(true);
    /// assert_eq!(v.as_bool(), Some(true));
    /// let s = Value::Str("x".into());
    /// assert_eq!(s.as_bool(), None);
    /// ```
    #[inline]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// 取对象可变引用；非 Object 返回 None。
    ///
    /// 暴露内部 `BTreeMap` 的可变引用，调用方据此直接增删改键。
    ///
    /// # 示例
    /// ```
    /// use sml_value::Value;
    /// use std::collections::BTreeMap;
    /// let mut v = Value::Object(BTreeMap::new());
    /// if let Some(m) = v.as_object_mut() {
    ///     m.insert("k".into(), Value::Int(1));
    /// }
    /// assert_eq!(v.get("k").and_then(|x| x.as_i64()), Some(1));
    /// // 非对象返回 None，不会 panic
    /// let mut s = Value::Str("x".into());
    /// assert!(s.as_object_mut().is_none());
    /// ```
    #[inline]
    pub fn as_object_mut(&mut self) -> Option<&mut BTreeMap<String, Value>> {
        match self {
            Value::Object(m) => Some(m),
            _ => None,
        }
    }

    /// 取数组可变引用；非 Array 返回 None。
    ///
    /// # 示例
    /// ```
    /// use sml_value::Value;
    /// let mut v = Value::Array(vec![]);
    /// if let Some(a) = v.as_array_mut() {
    ///     a.push(Value::Int(2));
    /// }
    /// assert_eq!(v.as_array().map(|a| a.len()), Some(1));
    /// let mut s = Value::Str("x".into());
    /// assert!(s.as_array_mut().is_none());
    /// ```
    #[inline]
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// 遍历直接子节点，返回子值（`&Value`）的迭代器。
    ///
    /// 对象按值遍历（忽略键名），数组按元素遍历；既非对象也非数组返回 None。
    /// 需要键名时请用 `as_object()` 取 `BTreeMap` 后 `.iter()`。
    ///
    /// # 示例
    /// ```
    /// use sml_value::Value;
    /// let arr = Value::Array(vec![Value::Int(1), Value::Int(2)]);
    /// if let Some(it) = arr.iter() {
    ///     let collected: Vec<_> = it.cloned().collect();
    ///     assert_eq!(collected, vec![Value::Int(1), Value::Int(2)]);
    /// }
    /// let s = Value::Str("x".into());
    /// assert!(s.iter().is_none());
    /// ```
    #[inline]
    pub fn iter(&self) -> Option<Box<dyn Iterator<Item = &Value> + '_>> {
        match self {
            Value::Object(m) => Some(Box::new(m.values())),
            Value::Array(a) => Some(Box::new(a.iter())),
            _ => None,
        }
    }

    /// 取元素个数；仅 Object / Array 返回 Some，其余返回 None。
    ///
    /// # 示例
    /// ```
    /// use sml_value::Value;
    /// let arr = Value::Array(vec![Value::Int(1)]);
    /// assert_eq!(arr.len(), Some(1));
    /// let obj = Value::Object(Default::default());
    /// assert_eq!(obj.len(), Some(0));
    /// let s = Value::Str("x".into());
    /// assert_eq!(s.len(), None);
    /// ```
    #[inline]
    pub fn len(&self) -> Option<usize> {
        match self {
            Value::Object(m) => Some(m.len()),
            Value::Array(a) => Some(a.len()),
            _ => None,
        }
    }

    /// 是否为空（无键 / 无元素）；仅 Object / Array 返回 Some，其余返回 None。
    ///
    /// # 示例
    /// ```
    /// use sml_value::Value;
    /// let arr = Value::Array(vec![]);
    /// assert_eq!(arr.is_empty(), Some(true));
    /// let arr2 = Value::Array(vec![Value::Int(1)]);
    /// assert_eq!(arr2.is_empty(), Some(false));
    /// let s = Value::Str("x".into());
    /// assert_eq!(s.is_empty(), None);
    /// ```
    #[inline]
    pub fn is_empty(&self) -> Option<bool> {
        match self {
            Value::Object(m) => Some(m.is_empty()),
            Value::Array(a) => Some(a.is_empty()),
            _ => None,
        }
    }
}

#[cfg(feature = "sml")]
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", to_sml(self))
    }
}

// NOTE: trailing-space trims are idempotent; guard against double-trimming. іӏоѵеԛіанхун
