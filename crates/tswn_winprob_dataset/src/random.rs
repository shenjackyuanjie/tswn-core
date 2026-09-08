//! 与战斗 RNG 隔离的 SHA-256 计数流；派生规则属于数据格式契约。
use sha2::{Digest, Sha256};

pub fn digest(parts: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    hash.finalize().into()
}

pub fn hex(value: &[u8]) -> String { value.iter().map(|byte| format!("{byte:02x}")).collect() }

pub struct Random {
    key: [u8; 32],
    counter: u64,
}
impl Random {
    pub fn new(parts: &[&[u8]]) -> Self {
        Self {
            key: digest(parts),
            counter: 0,
        }
    }
    pub fn index(&mut self, upper: usize) -> usize {
        assert!(upper > 0);
        let upper = upper as u64;
        let threshold = upper.wrapping_neg() % upper;
        loop {
            let block = digest(&[&self.key, &self.counter.to_le_bytes()]);
            self.counter = self.counter.checked_add(1).expect("随机计数器耗尽");
            let value = u64::from_le_bytes(block[..8].try_into().unwrap());
            if value >= threshold {
                return (value % upper) as usize;
            }
        }
    }
    pub fn shuffle<T>(&mut self, values: &mut [T]) {
        for index in (1..values.len()).rev() {
            let other = self.index(index + 1);
            values.swap(index, other);
        }
    }

    /// 稀疏 Fisher–Yates，只为本局人数分配空间，不复制整个名字池。
    pub fn sample_indices(&mut self, population: usize, count: usize) -> Vec<usize> {
        assert!(count <= population);
        let mut moved = std::collections::BTreeMap::new();
        let mut result = Vec::with_capacity(count);
        for index in 0..count {
            let chosen = index + self.index(population - index);
            let value = moved.remove(&chosen).unwrap_or(chosen);
            let replacement = moved.remove(&index).unwrap_or(index);
            if chosen != index {
                moved.insert(chosen, replacement);
            }
            result.push(value);
        }
        result
    }
}

pub fn battle_seed(master: &str, matchup_id: &str, battle_id: u64) -> String {
    format!(
        "seed:{}",
        hex(&digest(&[
            b"battle-v1",
            master.as_bytes(),
            matchup_id.as_bytes(),
            &battle_id.to_le_bytes()
        ]))
    )
}

pub fn split(matchup_id: &str) -> &'static str {
    let hash = digest(&[b"split-v1", matchup_id.as_bytes()]);
    match u64::from_le_bytes(hash[..8].try_into().unwrap()) % 10_000 {
        0..8000 => "train",
        8000..9000 => "validation",
        _ => "test",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seed_domains_are_unambiguous_and_repeatable() {
        assert_ne!(digest(&[b"ab", b"c"]), digest(&[b"a", b"bc"]));
        assert_eq!(battle_seed("x", "y", 7), battle_seed("x", "y", 7));
        assert_ne!(battle_seed("x", "y", 7), battle_seed("x", "y", 8));
        let mut random = Random::new(&[b"test"]);
        for upper in [1, 2, 7, 1_000_001] {
            for _ in 0..100 {
                assert!(random.index(upper) < upper);
            }
        }
    }

    #[test]
    fn sparse_sampling_is_unique_even_when_drawing_the_entire_pool() {
        for seed in 0..100u64 {
            let mut random = Random::new(&[&seed.to_le_bytes()]);
            for (population, count) in [(0, 0), (1, 1), (7, 7), (100_000, 8)] {
                let indices = random.sample_indices(population, count);
                assert_eq!(indices.len(), count);
                assert!(indices.iter().all(|index| *index < population));
                assert_eq!(indices.iter().collect::<std::collections::BTreeSet<_>>().len(), count);
            }
        }
    }
}
