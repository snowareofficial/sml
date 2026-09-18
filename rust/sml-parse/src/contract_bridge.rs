// SPDX-License-Identifier: MulanPSL-2.0
//! 契约桥接：把 `sml-contract` 的校验接到本 crate 的解析流程上。
//!
//! 存在理由：解析器需要在每个块结束时调用契约校验。相比在 `parser.rs`
//! 里直接 `use sml_contract::apply_contract`，这一层薄封装带来两个好处：
//! 1. 只暴露解析器真正需要的入口，契约 crate 的其余内部细节不泄漏进本 crate；
//! 2. 将来若契约应用方式变化（如增加缓存/合并策略），只需改这一处。

use std::collections::BTreeMap;

use sml_codes::SmlError;
use sml_contract::Contract;
use sml_value::Value;

/// 契约校验入口（转出 `sml-contract` 的同名函数，供 `parser` 使用）。
pub(crate) use sml_contract::apply_contract;

/// 契约表：解析器持有的 `名 → 契约` 表。
pub(crate) type ContractMap = BTreeMap<String, Contract>;

/// 校验一个已解析好的块是否满足契约（块结束时调用）。
#[allow(dead_code)]
pub(crate) fn check(
    c: &Contract,
    node: &mut BTreeMap<String, Value>,
    contracts: &ContractMap,
    path: &str,
) -> Result<(), SmlError> {
    apply_contract(c, node, contracts, path)
}
