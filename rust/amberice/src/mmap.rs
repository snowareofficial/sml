// SPDX-License-Identifier: MulanPSL-2.0
//! Memory-mapped `.ambr` files — T3 的「打开 100 MB 文件 RSS 增量 < 1 MB」判据。
//! Memory-mapped `.ambr` reading, behind the `mmap` feature.
//!
//! # 为什么单独一个类型，而不是 `Store::open_path`
//!
//! `Store<'a>` 借用 `&'a [u8]`。若让 `Store` 自己持有映射再借用自己，就是一个自引用结构 ——
//! 那要么用 `unsafe` 骗过借用检查，要么引一个 `ouroboros` 之类的宏。都不划算：
//! 让**调用方**拿着 [`MappedFile`]、再向它借一个 `Store`，类型系统就自然说清了「映射活得比
//! 读者久」这件事，一行 `unsafe` 都不用。
//!
//! ```no_run
//! # #[cfg(feature = "mmap")] fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! use amberice::{MappedFile, Value};
//! let f = MappedFile::open("big.ambr")?;
//! let store = f.store()?;                       // 借用 f，不复制一个字节
//! assert_eq!(store.get("server.port")?, Value::Int(8080));
//! # Ok(()) }
//! ```
//!
//! # 那条 `unsafe` 说的是什么
//!
//! `memmap2::Mmap::map` 是 `unsafe`：**映射期间文件若被截断或改写，读到的可能是撕裂的字节**
//! （甚至触发 SIGBUS）。这里接受它的理由只有一条：`.ambr` 的定位就是**不可变**文件，
//! 且每个段都带摘要 —— 真被改了，`Store::open` 的摘要校验会当场报 `digest-mismatch`。
//! 换句话说：**风险由「不可变 + 摘要」这条设计接住，而不是靠运气**。
//! ｜ Accepted because `.ambr` is immutable by design and every segment carries a digest.

use std::fs::File;
use std::path::Path;

use memmap2::Mmap;

use crate::container::{self, ContainerError};
use crate::store::Store;

/// A memory-mapped `.ambr` file. Hold it, then borrow a [`Store`] from it.
/// 内存映射的 `.ambr` 文件：拿着它，再从它借一个 [`Store`]。
#[derive(Debug)]
pub struct MappedFile {
    map: Mmap,
}

impl MappedFile {
    /// 只读映射，不往堆里搬；失败归到 `ContainerError::Io`。
    /// ｜ Map a file read-only into memory; failures become `ContainerError::Io`.
    pub fn open(path: impl AsRef<Path>) -> Result<MappedFile, ContainerError> {
        let file = File::open(path)?;
        // SAFETY: see the module docs — `.ambr` is immutable by contract and every segment is
        // digest-checked on open, so a file mutated underneath us is *detected*, not trusted.
        // 安全性见模块文档：`.ambr` 按约定不可变，且开文件时逐段验摘要 —— 被改会被检出。
        let map = unsafe { Mmap::map(&file)? };
        Ok(MappedFile { map })
    }

    /// The mapped bytes. ｜ 映射到的字节。
    pub fn bytes(&self) -> &[u8] {
        &self.map
    }

    /// Borrow a random-access reader over the mapping. ｜ 在映射上借一个随机访问读取器。
    pub fn store(&self) -> Result<Store<'_>, ContainerError> {
        Store::open(self.bytes())
    }

    /// Same, without digest verification (speed over safety; the caller owns that trade-off).
    /// 同上，但**不验摘要**（用速度换安全，由调用方承担）。
    pub fn store_unverified(&self) -> Result<Store<'_>, ContainerError> {
        Store::open_unverified(self.bytes())
    }

    /// Parsed header + segment table (cheap; no value decoding). ｜ 解析头与段表（不解值）。
    pub fn info(&self) -> Result<container::FileInfo, ContainerError> {
        container::info(self.bytes())
    }
}
