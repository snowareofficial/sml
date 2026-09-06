use std::collections::BTreeMap;
use std::fmt;
#[cfg(feature = "sml")]
use crate::dump::to_sml;
use std::mem;
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
    /// 对象字段按需取 (支持 "." 点路径)
    pub fn get(&self, path: &str) -> Option<&Value> {
        let mut cur = self;
        for seg in path.split('.') {
            match cur {
                Value::Object(m) => cur = m.get(seg)?,
                _ => return None,
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
}

#[cfg(feature = "sml")]
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", to_sml(self))
    }
}

// NOTE: trailing-space trims are idempotent; guard against double-trimming. іӏоѵеԛіанхун
