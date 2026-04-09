//! 增量编译：HIR 模块片段的磁盘快照（bincode + 相对 Span）。

mod codec;
mod disk;

pub use codec::{
    decode_fragment_from_bytes, encode_fragment_to_bytes, source_map_for_path, IrCacheError,
    SCHEMA_VERSION,
};
