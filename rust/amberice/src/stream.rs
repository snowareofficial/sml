//! 流式读取：只读需要的段 / 载荷，不把整份文件载入内存。
//!
//! 适用场景：大归档里只取一两个载荷（例如 `.smlex` 安装只读 package 里的几个 `*.smlex`）。
//! 对 `VALUE` / `STRTAB` 也按段读取 —— 不整份文件进内存，但单段仍是整体读入
//! （节点树解码需要连续字节）。
//!
//! 限制：`EMBED` 段被压缩或加密时无法按载荷偏移直读，本模块会明确报错，
//! 请改用 `crate::container::payload`（需要整份字节）。
//!
//! ｜ Streaming reader: reads only the segments/payloads it needs.
//! Compressed/encrypted `EMBED` bodies are refused (no random access after decode).

use std::io::{Read, Seek, SeekFrom};

use crate::container::{
    ContainerError, FileInfo, Segment, SegmentHeader, FMT, HEADER_LEN, MAGIC, SEG_EMBED, SEG_MANIFEST,
    SEG_STRTAB, SEG_VALUE, SEG_HEADER_LEN,
};
use crate::hash;
use crate::manifest::{Manifest, ManifestPayload};

fn io_err(e: std::io::Error) -> ContainerError {
    ContainerError::Io {
        kind: e.kind(),
        message: e.to_string(),
    }
}

fn granule_of(flags: u16) -> u64 {
    if flags & 1 != 0 {
        4096
    } else {
        8
    }
}

fn align_up(x: u64, granule: u64) -> u64 {
    let a = granule.max(1);
    x.div_ceil(a) * a
}

/// A `.ambr` file opened for streaming reads.
/// ｜ 以流式读取打开的 `.ambr`。
pub struct StreamReader<R> {
    src: R,
    info: FileInfo,
}

impl<R: Read + Seek> StreamReader<R> {
    /// Read the header + segment table only (no segment bodies).
    /// 只读头与段表（不读任何段体）。
    pub fn open(mut src: R) -> Result<Self, ContainerError> {
        let file_len = src.seek(SeekFrom::End(0)).map_err(io_err)?;
        if file_len < HEADER_LEN as u64 {
            return Err(ContainerError::TooShort {
                len: file_len as usize,
                need: HEADER_LEN,
            });
        }
        src.seek(SeekFrom::Start(0)).map_err(io_err)?;
        let mut head = [0u8; HEADER_LEN];
        src.read_exact(&mut head).map_err(io_err)?;

        let magic: [u8; 4] = [head[0], head[1], head[2], head[3]];
        if magic != MAGIC {
            return Err(ContainerError::BadMagic { got: magic });
        }
        let edition = u16::from_le_bytes([head[4], head[5]]);
        let fmt = head[6];
        let hdr_len = head[7];
        if fmt != FMT {
            return Err(ContainerError::UnsupportedFmt { fmt });
        }
        if (hdr_len as usize) < HEADER_LEN {
            return Err(ContainerError::HeaderTooShort { hdr_len });
        }
        let table_off = u64::from_le_bytes(head[8..16].try_into().expect("8 bytes"));
        let seg_count = u32::from_le_bytes(head[16..20].try_into().expect("4 bytes"));
        let flags = u16::from_le_bytes([head[20], head[21]]);
        let tail_off = u64::from_le_bytes(head[24..32].try_into().expect("8 bytes"));

        let table_len = (seg_count as u64) * (SEG_HEADER_LEN as u64);
        let Some(table_end) = table_off.checked_add(table_len) else {
            return Err(ContainerError::BadSegmentTable {
                at: table_off,
                need: table_len,
                file_len,
            });
        };
        if table_end > file_len {
            return Err(ContainerError::BadSegmentTable {
                at: table_off,
                need: table_len,
                file_len,
            });
        }

        let mut table = vec![0u8; table_len as usize];
        src.seek(SeekFrom::Start(table_off)).map_err(io_err)?;
        src.read_exact(&mut table).map_err(io_err)?;

        let granule = granule_of(flags);
        let mut segments = Vec::with_capacity(seg_count as usize);
        let mut data_off = align_up(table_end, granule);
        for i in 0..seg_count as usize {
            let at = i * SEG_HEADER_LEN;
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&table[at + 9..at + 41]);
            let header = SegmentHeader {
                seg_id: table[at],
                comp: table[at + 1],
                enc: table[at + 2],
                flags: u16::from_le_bytes([table[at + 3], table[at + 4]]),
                len: u32::from_le_bytes(table[at + 5..at + 9].try_into().expect("4 bytes")),
                hash,
            };
            let end = data_off + header.len as u64;
            if end > file_len {
                return Err(ContainerError::TruncatedSegment {
                    seg_id: header.seg_id,
                    at: data_off,
                    len: header.len as u64,
                    file_len,
                });
            }
            segments.push(Segment {
                header,
                offset: data_off,
            });
            data_off = align_up(end, granule);
        }

        Ok(Self {
            src,
            info: FileInfo {
                edition,
                fmt,
                hdr_len,
                flags,
                align: granule,
                tail_off,
                table_off,
                file_len,
                segments,
            },
        })
    }

    /// What `container::info` would report, without reading segment bodies.
    pub fn info(&self) -> &FileInfo {
        &self.info
    }

    fn find(&self, seg_id: u8) -> Result<Segment, ContainerError> {
        self.info
            .segments
            .iter()
            .find(|s| s.header.seg_id == seg_id)
            .cloned()
            .ok_or(ContainerError::MissingSegment { seg_id })
    }

    fn read_raw(&mut self, seg: &Segment) -> Result<Vec<u8>, ContainerError> {
        let mut buf = vec![0u8; seg.header.len as usize];
        self.src
            .seek(SeekFrom::Start(seg.offset))
            .map_err(io_err)?;
        self.src.read_exact(&mut buf).map_err(io_err)?;
        Ok(buf)
    }

    fn read_verified(
        &mut self,
        seg: &Segment,
        algo: hash::HashAlgo,
    ) -> Result<Vec<u8>, ContainerError> {
        let buf = self.read_raw(seg)?;
        if seg.header.hash != [0u8; 32] {
            let actual = hash::digest(algo, &buf).map_err(ContainerError::Hash)?;
            if actual != seg.header.hash {
                return Err(ContainerError::DigestMismatch {
                    seg_id: seg.header.seg_id,
                    at: seg.offset,
                    expected: seg.header.hash,
                    actual,
                });
            }
        }
        Ok(buf)
    }

    fn read_manifest(&mut self) -> Result<Option<Manifest>, ContainerError> {
        let Ok(seg) = self.find(SEG_MANIFEST) else {
            return Ok(None);
        };
        let raw = self.read_raw(&seg)?;
        Manifest::from_bytes(&raw)
            .map(Some)
            .map_err(ContainerError::BadManifest)
    }

    /// Payload list as declared by the manifest (no payload bytes are read).
    pub fn payloads(&mut self) -> Result<Vec<ManifestPayload>, ContainerError> {
        Ok(self.read_manifest()?.map(|m| m.payloads).unwrap_or_default())
    }

    /// Read **one** payload by name.
    ///
    /// When `EMBED` is plaintext this seeks straight to the requested slice — the rest of
    /// the archive is never read. Compressed/encrypted `EMBED` is refused (random access
    /// would require decoding the whole segment anyway).
    /// ｜ 明文 `EMBED` 时直接 seek 到目标切片；压缩/加密形态明确拒绝。
    pub fn payload(&mut self, name: &str) -> Result<Vec<u8>, ContainerError> {
        let manifest = self
            .read_manifest()?
            .ok_or(ContainerError::MissingSegment {
                seg_id: SEG_MANIFEST,
            })?;
        let seg = self.find(SEG_EMBED)?;
        if seg.header.comp != 0 || seg.header.enc != 0 {
            return Err(ContainerError::UnsupportedAlgorithm {
                seg_id: SEG_EMBED,
                comp: seg.header.comp,
                enc: seg.header.enc,
            });
        }
        let mut cursor = 0u64;
        for p in &manifest.payloads {
            if p.name == name {
                let mut buf = vec![0u8; p.len as usize];
                self.src
                    .seek(SeekFrom::Start(seg.offset + cursor))
                    .map_err(io_err)?;
                self.src.read_exact(&mut buf).map_err(io_err)?;
                let actual = hash::digest(manifest.hash, &buf).map_err(ContainerError::Hash)?;
                if actual != p.digest {
                    return Err(ContainerError::PayloadTableMismatch {
                        name: p.name.clone(),
                        field: "digest",
                    });
                }
                return Ok(buf);
            }
            cursor += p.len as u64;
        }
        Err(ContainerError::PayloadNotFound {
            name: name.to_string(),
        })
    }

    /// Decode the value tree (reads `VALUE` + `STRTAB` only).
    pub fn load(&mut self) -> Result<sml_value::Value, ContainerError> {
        let manifest = self.read_manifest()?;
        let algo = manifest.map(|m| m.hash).unwrap_or(hash::DEFAULT);
        let value_seg = self.find(SEG_VALUE)?;
        let value = self.read_verified(&value_seg, algo)?;
        let strtab_seg = self
            .info
            .segments
            .iter()
            .find(|s| s.header.seg_id == SEG_STRTAB)
            .cloned();
        let strtab = match strtab_seg {
            Some(seg) => self.read_verified(&seg, algo)?,
            None => Vec::new(),
        };
        sml_value::Value::from_canonical_blocks(&strtab, &value)
            .map_err(ContainerError::Canonical)
    }
}
