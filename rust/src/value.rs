use std::collections::BTreeMap;
use std::fmt;
#[cfg(feature = "sml")]
use crate::core::to_sml;
use std::mem;
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<Value>),
    /// 对象/块; `__type` / `__name` 裸块元数据以保留字键存放
    Object(BTreeMap<String, Value>),
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
            // 标量（Null/Bool/Int/Float/Str）：仅 Str 持有堆资源（String），手动释放后
            // 丢弃，避免自动 drop 再次进入本 drop 造成无限递归。
            if let Value::Str(ref mut s) = owned {
                let s = std::mem::take(s); // 取回 String 所有权，owned 的 String 置空
                drop(s); // 释放 String 堆内存
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
            }
            // cur 此时已为空壳（数组/对象的字段被 take，或标量资源已释放）；
            // forget 阻止其自动（递归）drop，无堆资源泄漏。
            std::mem::forget(cur);
        }
        // stack 此时为空，其 drop 为常数开销（释放空 Vec）。
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
