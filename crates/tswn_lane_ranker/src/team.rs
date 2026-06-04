use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct TeamDsu {
    parent: HashMap<String, String>,
}

impl TeamDsu {
    pub fn from_pairs(pairs: Vec<(String, String)>) -> Self {
        let mut dsu = Self::default();

        // 正确加载数据库中的 name -> parent 关系。
        // 战队合并只影响 make 阶段的“同一合并战队最多 5 个”限制，
        // 不影响组号合法性检查。
        for (name, parent) in pairs {
            dsu.parent.insert(name.clone(), parent.clone());
            dsu.parent.entry(parent.clone()).or_insert(parent);
        }

        // 路径压缩。
        let keys: Vec<String> = dsu.parent.keys().cloned().collect();
        for key in keys {
            let _ = dsu.find(&key);
        }

        dsu
    }

    pub fn ensure(&mut self, name: &str) { self.parent.entry(name.to_string()).or_insert_with(|| name.to_string()); }

    pub fn find(&mut self, name: &str) -> String {
        self.ensure(name);

        let parent = self.parent.get(name).cloned().unwrap();
        if parent == name {
            return parent;
        }

        let root = self.find(&parent);
        self.parent.insert(name.to_string(), root.clone());
        root
    }

    pub fn find_readonly(&self, name: &str) -> String {
        let mut cur = name.to_string();
        let mut guard = 0usize;

        while let Some(next) = self.parent.get(&cur) {
            if next == &cur {
                return cur;
            }

            cur = next.clone();
            guard += 1;

            if guard > self.parent.len() + 2 {
                return name.to_string();
            }
        }

        name.to_string()
    }

    pub fn union(&mut self, a: &str, b: &str) -> String {
        let ra = self.find(a);
        let rb = self.find(b);

        if ra != rb {
            // 稳定一点：用字典序较小的名字当根，方便 UI 和调试。
            let (root, child) = if ra <= rb { (ra, rb) } else { (rb, ra) };
            self.parent.insert(child, root.clone());
            root
        } else {
            ra
        }
    }

    pub fn pairs(&self) -> Vec<(String, String)> {
        let mut out = Vec::with_capacity(self.parent.len());

        for name in self.parent.keys() {
            out.push((name.clone(), self.find_readonly(name)));
        }

        out
    }
}
