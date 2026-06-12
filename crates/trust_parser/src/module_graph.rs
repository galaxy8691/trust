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

    pub fn add_module(&mut self, path: &str, imports: Vec<String>, exports: HashSet<String>) {
        self.modules.insert(path.to_string(), ModuleInfo {
            path: path.to_string(), imports, exports,
        });
    }

    /// 拓扑排序 + 循环检测。
    pub fn resolve(&mut self) -> Result<Vec<String>, String> {
        let mut visited = HashSet::new();
        let mut in_stack = HashSet::new();
        let mut order = vec![];

        for path in self.modules.keys().cloned().collect::<Vec<_>>() {
            if !visited.contains(&path) {
                self.dfs(&path, &mut visited, &mut in_stack, &mut order)?;
            }
        }
        // 后序 DFS: 被导入者在访问导入者之前完成 → 后序即拓扑序
        self.order = order.clone();
        Ok(order)
    }

    fn dfs(
        &self,
        path: &str,
        visited: &mut HashSet<String>,
        in_stack: &mut HashSet<String>,
        order: &mut Vec<String>,
    ) -> Result<(), String> {
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

    /// AC-MOD-001: 循环导入检测
    #[test]
    fn module_graph_cycle_detection_returns_error() {
        let mut g = ModuleGraph::new();
        g.add_module("a.trust", vec!["b.trust".into()], HashSet::new());
        g.add_module("b.trust", vec!["a.trust".into()], HashSet::new());
        assert!(g.resolve().is_err());
    }

    /// AC-MOD-002: 拓扑排序正确
    #[test]
    fn module_graph_no_cycle_returns_topological_order() {
        let mut g = ModuleGraph::new();
        // a imports b, b imports c → c before b before a
        g.add_module("a.trust", vec!["b.trust".into()], HashSet::new());
        g.add_module("b.trust", vec!["c.trust".into()], HashSet::new());
        g.add_module("c.trust", vec![], HashSet::new());
        let order = g.resolve().unwrap();
        let ca = order.iter().position(|p| p == "c.trust").unwrap();
        let ba = order.iter().position(|p| p == "b.trust").unwrap();
        let aa = order.iter().position(|p| p == "a.trust").unwrap();
        assert!(ca < ba, "c (imported by b) must come before b. order: {:?}", order);
        assert!(ba < aa, "b (imported by a) must come before a. order: {:?}", order);
    }

    /// AC-MOD-003: 多入口文件正确解析并拓扑排序
    #[test]
    fn module_graph_multi_entry_resolves_correctly() {
        let mut g = ModuleGraph::new();
        g.add_module("main.trust", vec!["lib.trust".into()], HashSet::new());
        g.add_module("other.trust", vec!["lib.trust".into()], HashSet::new());
        g.add_module("lib.trust", vec![], HashSet::new());
        let order = g.resolve().unwrap();
        assert_eq!(order.len(), 3);
        // lib.trust 被所有入口导入 → 必须在最前面
        let lib_pos = order.iter().position(|p| p == "lib.trust").unwrap();
        let main_pos = order.iter().position(|p| p == "main.trust").unwrap();
        let other_pos = order.iter().position(|p| p == "other.trust").unwrap();
        assert!(lib_pos < main_pos, "lib must come before main. order: {:?}", order);
        assert!(lib_pos < other_pos, "lib must come before other. order: {:?}", order);
    }
}
