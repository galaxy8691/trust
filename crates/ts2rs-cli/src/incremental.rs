//! 多文件 HIR 片段磁盘缓存（`--incremental`）。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts2rs_hir::{
    build_module_ir_fragment, compile_merged_fragments_with_options, decode_fragment_from_bytes,
    encode_fragment_to_bytes, CodegenOptions, ModuleIrFragment,
};
use ts2rs_parser::{rebuild_transitive_importers_from_forward, ParsedModuleGraph};

const MANIFEST_VERSION: u32 = 1;

pub(crate) fn resolve_incremental_cache_root(dir: &Path) -> PathBuf {
    if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|c| c.join(dir))
            .unwrap_or_else(|_| dir.to_path_buf())
    }
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    v: u32,
    /// canonical path string -> sha256 hex of file bytes
    file_hashes: HashMap<String, String>,
}

fn hash_bytes(data: &[u8]) -> String {
    let d = Sha256::digest(data);
    format!("{d:x}")
}

fn graph_bucket_key(graph: &ParsedModuleGraph) -> String {
    let mut paths: Vec<String> = graph
        .modules
        .iter()
        .map(|m| {
            ParsedModuleGraph::canonical_module_path(m)
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    paths.sort();
    let entry = graph.entry.to_string_lossy();
    let key = format!("entry:{entry}\n{}", paths.join("\n"));
    hash_bytes(key.as_bytes())
}

fn module_cache_path(cache_root: &Path, graph_key: &str, canon_path: &Path) -> PathBuf {
    let tag = hash_bytes(canon_path.to_string_lossy().as_bytes());
    cache_root
        .join(graph_key)
        .join("modules")
        .join(format!("{tag}.bin"))
}

fn manifest_path(cache_root: &Path, graph_key: &str) -> PathBuf {
    cache_root.join(graph_key).join("manifest.bin")
}

fn file_content_hash(path: &Path) -> Result<String, String> {
    let b = fs::read(path).map_err(|e| e.to_string())?;
    Ok(hash_bytes(&b))
}

/// 解析并校验后的模块图 → Rust 源码（HIR 片段可缓存）。
pub fn compile_graph_incremental(
    graph: &ParsedModuleGraph,
    cache_root: &Path,
    codegen: &CodegenOptions,
) -> Result<(String, Vec<ts2rs_hir::CompileWarning>), String> {
    let gkey = graph_bucket_key(graph);
    let forward = graph.forward_deps().map_err(|e| e.to_string())?;

    let mut current_hashes: HashMap<String, String> = HashMap::new();
    for pm in &graph.modules {
        let canon = ParsedModuleGraph::canonical_module_path(pm);
        let h = file_content_hash(&pm.path)?;
        current_hashes.insert(canon.to_string_lossy().into_owned(), h);
    }

    let man_path = manifest_path(cache_root, &gkey);
    let old_manifest: Option<Manifest> = fs::read(&man_path).ok().and_then(|b| {
        bincode::deserialize::<Manifest>(&b)
            .ok()
            .filter(|m| m.v == MANIFEST_VERSION)
    });

    let mut dirty: HashSet<PathBuf> = HashSet::new();
    for pm in &graph.modules {
        let canon = ParsedModuleGraph::canonical_module_path(pm);
        let key = canon.to_string_lossy().into_owned();
        let new_h = current_hashes.get(&key).expect("key inserted");
        let old_ok = old_manifest.as_ref().and_then(|m| m.file_hashes.get(&key));
        if old_ok.map(String::as_str) != Some(new_h.as_str()) {
            dirty.insert(canon);
        }
    }

    let rebuild: HashSet<PathBuf> = if dirty.is_empty() {
        HashSet::new()
    } else {
        rebuild_transitive_importers_from_forward(&forward, &dirty)
    };

    let mut fragments: Vec<(String, ModuleIrFragment)> = Vec::with_capacity(graph.modules.len());
    let mut next_id = 0u32;
    let mut fragment_rebuilds: u32 = 0;

    for pm in &graph.modules {
        let canon = ParsedModuleGraph::canonical_module_path(pm);
        let path_str = pm.path.to_string_lossy().into_owned();
        let mod_path = module_cache_path(cache_root, &gkey, &canon);

        let frag = if !rebuild.contains(&canon) {
            match fs::read(&mod_path)
                .ok()
                .zip(fs::read_to_string(&pm.path).ok())
            {
                Some((bytes, src)) => decode_fragment_from_bytes(&pm.path, &src, &bytes).ok(),
                None => None,
            }
        } else {
            None
        };

        let frag = match frag {
            Some(f) => f,
            None => {
                fragment_rebuilds += 1;
                let f = build_module_ir_fragment(
                    &path_str,
                    &pm.source.program,
                    &pm.source.source_map,
                    &pm.source.comments,
                    true,
                    &mut next_id,
                )
                .map_err(|e| e.to_string())?;
                let bytes = encode_fragment_to_bytes(&f).map_err(|e| e.to_string())?;
                if let Some(parent) = mod_path.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fs::write(&mod_path, bytes).map_err(|e| e.to_string())?;
                f
            }
        };

        fragments.push((path_str, frag));
    }

    let out = compile_merged_fragments_with_options(&fragments, &graph.entry_path_str(), codegen)
        .map_err(|e| e.to_string())?;

    let manifest = Manifest {
        v: MANIFEST_VERSION,
        file_hashes: current_hashes,
    };
    if let Some(parent) = man_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let enc = bincode::serialize(&manifest).map_err(|e| e.to_string())?;
    fs::write(&man_path, enc).map_err(|e| e.to_string())?;

    if std::env::var_os("TS2RS_TEST_FRAGMENT_STATS").is_some() {
        eprintln!("ts2rs_fragment_rebuilds={fragment_rebuilds}");
    }

    Ok(out)
}
