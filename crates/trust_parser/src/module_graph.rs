//! Trust 模块图 — 跨文件依赖解析与循环检测

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct ModuleGraph {
    pub modules: HashMap<String, ModuleInfo>,
    pub order: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub path: String,
    pub imports: Vec<String>,
    pub exports: HashSet<String>,
}

impl ModuleGraph {
    pub fn new() -> Self {
        ModuleGraph { modules: HashMap::new(), order: vec![] }
    }

    /// 注册模块
    pub fn add_module(&mut self, path: &str, imports: Vec<String>, exports: HashSet<String>) {
        self.modules.insert(path.to_string(), ModuleInfo {
            path: path.to_string(), imports, exports,
        });
    }

    /// 拓扑排序 + 循环检测。返回 Ok(order) 或 Err(cycle_path)。
    pub fn resolve(&mut self) -> Result<Vec<String>, String> {
        let mut visited = HashSet::new();
        let mut in_stack = HashSet::new();
        let mut order = vec![];

        for path in self.modules.keys().cloned().collect::<Vec<_>>() {
            if !visited.contains(&path) {
                if let Err(cycle) = self.dfs(&path, &mut visited, &mut in_stack, &mut order) {
                    return Err(cycle);
                }
            }
        }
        order.reverse();
        self.order = order.clone();
        Ok(order)
    }

    fn dfs(&self, path: &str, visited: &mut HashSet<String>, in_stack: &mut HashSet<String>, order: &mut Vec<String>) -> Result<(), String> {
        visited.insert(path.to_string());
        in_stack.insert(path.to_string());
        if let Some(info) = self.modules.get(path) {
            for imp in &info.imports {
                if in_stack.contains(imp) {
                    return Err(format!("circular import: {} -> {}", path, imp));
                }
                if !visited.contains(imp) {
                    self.dfs(imp, visited, in_stack, order)?;
                }
            }
        }
        in_stack.remove(path);
        order.push(path.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_graph_no_cycle_returns_topological_order() {
        let mut g = ModuleGraph::new();
        // a imports b, b imports c
        g.add_module("a.trust", vec!["b.trust".into()], HashSet::new());
        g.add_module("b.trust", vec!["c.trust".into()], HashSet::new());
        g.add_module("c.trust", vec![], HashSet::new());
        let order = g.resolve().unwrap();
        // c before b before a
        let ca = order.iter().position(|p| p == "c.trust").unwrap();
        let ba = order.iter().position(|p| p == "b.trust").unwrap();
        let aa = order.iter().position(|p| p == "a.trust").unwrap();
        assert!(ca < ba);
        assert!(ba < aa);
    }

    #[test]
    fn module_graph_cycle_detection_returns_error() {
        let mut g = ModuleGraph::new();
        g.add_module("a.trust", vec!["b.trust".into()], HashSet::new());
        g.add_module("b.trust", vec!["a.trust".into()], HashSet::new());
        assert!(g.resolve().is_err());
    }

    #[test]
    fn module_graph_multi_entry_resolves_correctly() {
        let mut g = ModuleGraph::new();
        g.add_module("main.trust", vec!["lib.trust".into()], HashSet::new());
        g.add_module("other.trust", vec!["lib.trust".into()], HashSet::new());
        g.add_module("lib.trust", vec![], HashSet::new());
        let order = g.resolve().unwrap();
        assert_eq!(order.len(), 3);
        assert_eq!(order[0], "lib.trust"); // 被导入者先
    }
}
