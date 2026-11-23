//! 与战斗 RNG 隔离的 SHA-256 计数流；派生规则属于数据格式契约。
//!
//! 每个输入片段带小端长度前缀，调用方用不同域标签分离阵容、抽样、seed 和切分。
//! index 使用拒绝采样避免模偏差，upper=0 会 panic；同样初始参数和调用顺序产生同样输出。
//! 这里只服务可复现采样，不从系统取熵，也不把派生 seed 当作保密令牌。
use sha2::{Digest, Sha256};

/// 对带长度前缀的字节片段序列计算 SHA-256 摘要。
pub fn digest(parts: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    hash.finalize().into()
}

/// 把字节序列格式化为小写十六进制字符串。
pub fn hex(value: &[u8]) -> String { value.iter().map(|byte| format!("{byte:02x}")).collect() }

/// 确定性随机源：由密钥与计数器派生，跨平台与跨版本稳定。
pub struct Random {
    key: [u8; 32],
    counter: u64,
}
impl Random {
    /// 由字节片段派生密钥构造随机源。
    pub fn new(parts: &[&[u8]]) -> Self {
        Self {
            key: digest(parts),
            counter: 0,
        }
    }
    /// 返回 [0, upper) 内的均匀随机下标；用拒绝采样消除取模偏差。
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
    /// 原地 Fisher-Yates 洗牌。
    pub fn shuffle<T>(&mut self, values: &mut [T]) {
        for index in (1..values.len()).rev() {
            let other = self.index(index + 1);
            values.swap(index, other);
        }
    }

    /// 返回 count 个互不重复的 [0, population) 下标，顺序即抽取顺序。
    ///
    /// 稀疏 Fisher–Yates 用映射记录交换，不复制名字池，额外存储随 count 增长。
    /// count=0 返回空向量；count>population 会 panic，输入人数检查由调用方负责。
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

/// 由主种子、对局 ID 与场次序号派生战斗种子字符串。
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

/// 由对局 ID 确定性地派生数据切分：train/validation/test。
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
