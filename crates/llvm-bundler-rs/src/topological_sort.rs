use std::collections::HashSet;

use crate::dependency_graph::DependencyGraph;

pub struct TopologicalSort {
    visited: HashSet<String>,
    ordered_list: Vec<String>,
}

impl TopologicalSort {
    pub fn get_ordered_list(graph: &DependencyGraph) -> Vec<String> {
        let mut topological_sort = Self {
            visited: HashSet::new(),
            ordered_list: Vec::with_capacity(2 * graph.len()),
        };
        for node in graph.keys() {
            topological_sort.dfs(node, graph);
        }
        topological_sort.ordered_list
    }

    fn dfs(&mut self, node: &str, graph: &DependencyGraph) {
        if self.visited.contains(node) {
            return;
        }
        let children = graph.get(node).unwrap();
        for child in children {
            self.dfs(child, graph);
        }
        self.visited.insert(node.to_string());
        self.ordered_list.push(node.to_string());
    }
}
