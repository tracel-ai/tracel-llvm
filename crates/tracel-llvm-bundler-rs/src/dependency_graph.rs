use std::{
    collections::{HashMap, HashSet},
    fs, io,
    ops::{Deref, DerefMut},
    path::Path,
};

use regex::Regex;

pub struct DependencyGraph(HashMap<String, Vec<String>>);

impl Deref for DependencyGraph {
    type Target = HashMap<String, Vec<String>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DependencyGraph {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl DependencyGraph {
    pub fn from_cmake(path: impl AsRef<Path>) -> io::Result<DependencyGraph> {
        let content = fs::read_to_string(path)?;

        let re_target = Regex::new(r#"add_library\(([a-zA-Z0-9]+) STATIC IMPORTED\)"#).unwrap();
        let mut static_libs = HashSet::new();
        for cap in re_target.captures_iter(&content) {
            static_libs.insert(cap[1].to_string());
        }

        let re_deps = Regex::new(
            r#"set_target_properties\(([a-zA-z0-9]+) PROPERTIES(\s+)INTERFACE_LINK_LIBRARIES \"(.*)\""#,
        )
        .unwrap();
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        for cap in re_deps.captures_iter(&content) {
            let target = cap[1].to_string();
            if static_libs.contains(&target) {
                let deps: Vec<String> = cap[3].split(';').map(String::from).collect();
                let filtered_deps: Vec<String> = deps
                    .into_iter()
                    .filter(|d| static_libs.contains(d))
                    .collect();
                graph.insert(target, filtered_deps);
            }
        }
        // Necessary if some static_libs like MLIRTableGen, doesn't have any dependency
        for lib in &static_libs {
            graph.entry(lib.to_string()).or_default();
        }
        Ok(Self(graph))
    }
}
