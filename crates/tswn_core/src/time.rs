//! 跨平台计时辅助。
//!
//! `wasm32-unknown-unknown` 上 `std::time::Instant::now()` 会直接 panic：std 没有为
//! 该 target 实现时钟（见 `library/std/src/sys/time/unsupported.rs`），而 wasm-bindgen
//! 运行环境（浏览器、Node）也不暴露其它系统时钟。因此这里在 wasm 上改用
//! `js_sys::Date::now()`：毫秒精度且不保证严格单调，但足够承载 `*_timed` 诊断字段
//! （`init_nanos` / `fight_nanos` / matchup 墙钟耗时）。
//!
//! 战斗与胜率语义不读取这些计时值，精度回退不会影响计算结果；此前 wasm 侧一旦走到
//! `Instant::now()` 就会 `RuntimeError: unreachable` 整个实例，胜率、评分等接口全部不可用。

#[cfg(target_family = "wasm")]
pub(crate) use self::wasm::Stopwatch;
#[cfg(target_family = "wasm")]
mod wasm {
    /// 计时点；记录 `Date::now()` 的毫秒时间戳。
    #[derive(Clone, Copy, Debug)]
    pub struct Stopwatch(f64);

    impl Stopwatch {
        pub fn now() -> Self { Self(js_sys::Date::now()) }

        /// 自本计时点起经过的纳秒数；时钟回拨时记为 0。
        pub fn elapsed_nanos(&self) -> u128 { elapsed_nanos(self.0, js_sys::Date::now()) }

        /// 与 [`std::time::Instant::duration_since`] 同语义。
        pub fn duration_since(&self, earlier: Self) -> std::time::Duration {
            std::time::Duration::from_nanos(elapsed_nanos(earlier.0, self.0) as u64)
        }
    }

    /// 聚合 CQP 结果时要比较最早开始 / 最晚结束，`Instant` 原生可比较，这里补齐同语义。
    impl PartialEq for Stopwatch {
        fn eq(&self, other: &Self) -> bool { self.0 == other.0 }
    }

    impl Eq for Stopwatch {}

    impl PartialOrd for Stopwatch {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
    }

    impl Ord for Stopwatch {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering { self.0.total_cmp(&other.0) }
    }

    fn elapsed_nanos(earlier_millis: f64, now_millis: f64) -> u128 {
        let millis = now_millis - earlier_millis;
        if millis <= 0.0 { 0 } else { (millis * 1_000_000.0) as u128 }
    }
}

#[cfg(not(target_family = "wasm"))]
pub(crate) use self::native::Stopwatch;
#[cfg(not(target_family = "wasm"))]
mod native {
    /// 计时点；包装 [`std::time::Instant`]，与 wasm 分支保持同一接口。
    #[derive(Clone, Copy, Debug)]
    pub struct Stopwatch(std::time::Instant);

    impl Stopwatch {
        pub fn now() -> Self { Self(std::time::Instant::now()) }

        /// 自本计时点起经过的纳秒数。
        pub fn elapsed_nanos(&self) -> u128 { self.0.elapsed().as_nanos() }

        /// 与 [`std::time::Instant::duration_since`] 同语义。
        pub fn duration_since(&self, earlier: Self) -> std::time::Duration { self.0.saturating_duration_since(earlier.0) }
    }

    /// 与 [`std::time::Instant`] 保持一致的比较语义，供 CQP 聚合最早开始 / 最晚结束。
    impl PartialEq for Stopwatch {
        fn eq(&self, other: &Self) -> bool { self.0 == other.0 }
    }

    impl Eq for Stopwatch {}

    impl PartialOrd for Stopwatch {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
    }

    impl Ord for Stopwatch {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering { self.0.cmp(&other.0) }
    }
}

// wasm 上 `std::thread::sleep` 同样未实现，计时行为由 JS 侧冒烟脚本覆盖；
// 这里的单测只验证 native 分支与公共接口语义。
#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::Stopwatch;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn stopwatch_measures_elapsed_time() {
        let started = Stopwatch::now();
        thread::sleep(Duration::from_millis(5));

        assert!(started.elapsed_nanos() >= 1_000_000, "睡眠 5ms 后应至少过去 1ms");
        assert!(started.duration_since(started).is_zero(), "同一计时点的差值应为 0");
    }

    #[test]
    fn stopwatch_orders_by_time() {
        let earlier = Stopwatch::now();
        thread::sleep(Duration::from_millis(2));
        let later = Stopwatch::now();

        assert!(earlier < later);
        assert_eq!(earlier.min(later), earlier);
        assert_eq!(later.max(earlier), later);
    }
}
