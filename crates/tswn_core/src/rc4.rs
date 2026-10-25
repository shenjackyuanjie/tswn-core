//! RC4 伪随机数生成器。
//!
//! 提供名字构建与战斗使用的兼容随机流，以及概率、范围、目标选择和排序辅助。
//! 每场 CombatRuntime 持有自己的 [`RC4`]；构建和其他业务工具使用各自过程。
//!
//! # 可复现性
//!
//! [`RC4::next_u8`] 推进状态，其他随机方法按各自规则消费字节；调用顺序影响整场决策。
//! 完整状态包括 256 字节排列和 i/j 游标；[`RC4::peek_next_u8`] 用于保持当前位置的观察。
//! 本模块服务游戏兼容，安全相关的新功能选择适合安全用途的随机/密码学库。
//!
//! # 消费与边界约定
//!
//! - 概率判定 `c*`、单字节范围 `r*` 和 `r3x3` 每次消费一个字节；
//!   `rFFFF` 消费两个，`rFFFFFF` 消费三个。`c94` 等名字是近似比例，不是精确百分数。
//! - [`RC4::next_i32`] 的零上界不消费字节；正上界至少消费两个字节，
//!   即使上界为 1、返回值必定为 0，也会推进随机流。
//! - [`RC4::pick`] 的空列表与单元素列表不消费字节；排除选择方法还要求
//!   有效下标或连续的排除区间，不能用它们代替任意集合过滤。
//! - 字符串混入方法按 UTF-8 字节消费，和名字常见度评估的 UTF-16 码元不同；
//!   它们只更新随机状态，不返回变换后的字符串。
//!
//! [`RC4::update`] 保留 i/j 游标，[`RC4::round`] 重置游标；二者不能互换。
//! `Clone` 复制完整排列与游标，适合分叉确定性流。调试 `byte_count` 只在探针启用时
//! 统计 [`RC4::next_u8`] 调用，不包含直接混入字符串/字节的未跟踪步骤。
//!
//! ```
//! use tswn_core::rc4::RC4;
//! let mut first = RC4::new(b"demo", 1);
//! let mut second = first.clone();
//! assert_eq!(first.peek_next_u8(), second.next_u8());
//! assert_eq!(first.next_u8(), RC4::new(b"demo", 1).next_u8());
//! ```

/// 生成按字节值升序排列的初始 S-box；展开结果固定包含 0 到 255 各一次。
macro_rules! val {
    () => {
        [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
            32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60,
            61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89,
            90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114,
            115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137,
            138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160,
            161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183,
            184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206,
            207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229,
            230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252,
            253, 254, 255,
        ]
    };
}
/// 尚未进行密钥调度的恒等排列，供新状态和 KSA 检查点初始化。
const VAL_INIT: [u8; 256] = val!();

/// 状态数组长度。
pub const VAL_LEN: usize = 256;

/// 对现有状态执行一轮 RC4 密钥调度。
///
/// 热路径会为每个玩家执行多次完整的 256 字节调度。这里用循环键下标代替取模，
/// 并在一次边界证明后直接访问状态和密钥，避免在循环内反复做边界检查。
#[inline]
fn apply_key_scheduling(state: &mut [u8; VAL_LEN], keys: &[u8]) {
    let key_len = keys.len();
    assert!(key_len != 0, "RC4 密钥不能为空");

    let state_ptr = state.as_mut_ptr();
    let key_ptr = keys.as_ptr();
    let mut key_index = 0usize;
    let mut j = 0u8;

    for x in 0..VAL_LEN {
        // SAFETY: x 始终小于 VAL_LEN；key_index 在每轮末尾回绕，始终小于非零的 key_len；
        // j 是 u8，转换后的下标天然落在 0..VAL_LEN。两个状态指针可能相同，逐字节交换对此安全。
        unsafe {
            let x_ptr = state_ptr.add(x);
            j = j.wrapping_add(*x_ptr).wrapping_add(*key_ptr.add(key_index));
            let j_ptr = state_ptr.add(j as usize);
            let value = *x_ptr;
            *x_ptr = *j_ptr;
            *j_ptr = value;
        }

        key_index += 1;
        if key_index == key_len {
            key_index = 0;
        }
    }
}

/// 执行一条 KSA lane 的单步交换。
#[inline(always)]
fn apply_key_scheduling_step(state: &mut [u8; VAL_LEN], key: &[u8], x: usize, key_index: usize, j: &mut u8) {
    let state_ptr = state.as_mut_ptr();
    // SAFETY: 调用方保证 x 小于 256，key_index 小于非空 key 长度；j 为 u8，
    // 因而状态数组的两个下标和密钥下标都始终在界内。
    unsafe {
        let x_ptr = state_ptr.add(x);
        *j = j.wrapping_add(*x_ptr).wrapping_add(*key.as_ptr().add(key_index));
        let j_ptr = state_ptr.add(*j as usize);
        let value = *x_ptr;
        *x_ptr = *j_ptr;
        *j_ptr = value;
    }
}

macro_rules! define_same_len_interleaved_ksa {
    (
        $name:ident,
        $width:literal,
        ($first_state:ident, $first_key:ident, $first_j:ident)
        $(, ($state:ident, $key:ident, $j:ident))*
        $(,)?
    ) => {
        #[inline]
        fn $name(states: &mut [RC4; $width], keys: &[&[u8]; $width]) {
            let [$first_state, $($state),*] = states;
            let &[$first_key, $($key),*] = keys;
            let key_len = $first_key.len();
            debug_assert!(key_len != 0);
            debug_assert!(keys.iter().all(|key| key.len() == key_len));
            let mut $first_j = 0u8;
            $(let mut $j = 0u8;)+
            macro_rules! apply_all_lanes {
                ($x:expr, $key_index:expr) => {{
                    apply_key_scheduling_step(
                        &mut $first_state.main_val,
                        $first_key,
                        $x,
                        $key_index,
                        &mut $first_j,
                    );
                    $(apply_key_scheduling_step(&mut $state.main_val, $key, $x, $key_index, &mut $j);)+
                }};
            }
            if key_len == 9 {
                let mut x = 0usize;
                while x < 252 {
                    apply_all_lanes!(x, 0);
                    apply_all_lanes!(x + 1, 1);
                    apply_all_lanes!(x + 2, 2);
                    apply_all_lanes!(x + 3, 3);
                    apply_all_lanes!(x + 4, 4);
                    apply_all_lanes!(x + 5, 5);
                    apply_all_lanes!(x + 6, 6);
                    apply_all_lanes!(x + 7, 7);
                    apply_all_lanes!(x + 8, 8);
                    x += 9;
                }
                apply_all_lanes!(252, 0);
                apply_all_lanes!(253, 1);
                apply_all_lanes!(254, 2);
                apply_all_lanes!(255, 3);
                return;
            }
            let mut key_index = 0usize;
            for x in 0..VAL_LEN {
                apply_all_lanes!(x, key_index);
                key_index += 1;
                if key_index == key_len {
                    key_index = 0;
                }
            }
        }
    };
}

define_same_len_interleaved_ksa!(apply_key_scheduling_interleaved_2, 2, (state0, key0, j0), (state1, key1, j1),);
define_same_len_interleaved_ksa!(
    apply_key_scheduling_interleaved_3,
    3,
    (state0, key0, j0),
    (state1, key1, j1),
    (state2, key2, j2),
);
define_same_len_interleaved_ksa!(
    apply_key_scheduling_interleaved_4,
    4,
    (state0, key0, j0),
    (state1, key1, j1),
    (state2, key2, j2),
    (state3, key3, j3),
);

/// 交错推进 key 长度不同的 KSA 链，作为通用回退路径。
#[inline]
fn apply_key_scheduling_interleaved_variable_keys<const N: usize>(states: &mut [RC4; N], keys: &[&[u8]; N]) {
    let mut key_indices = [0usize; N];
    let mut js = [0u8; N];
    for x in 0..VAL_LEN {
        for lane in 0..N {
            let state = &mut states[lane].main_val;
            let key = keys[lane];
            let key_index = key_indices[lane];
            js[lane] = js[lane].wrapping_add(state[x]).wrapping_add(key[key_index]);
            state.swap(x, js[lane] as usize);
            key_indices[lane] = if key_index + 1 == key.len() { 0 } else { key_index + 1 };
        }
    }
}

/// 名字构建与战斗共享的确定性随机状态，包含 S-box 与 PRGA 游标。
///
/// `Default` 仅给出恒等排列；一般以 [`Self::new`] 的密钥调度结果开始。
/// 字段公开用于兼容与诊断，逐字节消费时游标按低八位推进；
/// 修改排列会改变后续全部输出，复制流应同时复制排列与游标。
#[allow(unused)]
#[repr(C)]
#[derive(Debug, Clone)]
pub struct RC4 {
    pub i: u32,
    pub j: u32,
    /// 当前 S-box；标准构造与调度保持它是 0 到 255 的排列。
    pub main_val: [u8; 256],
    #[cfg(not(feature = "no_debug"))]
    pub byte_count: u64,
}

/// RC4 第一轮 KSA 的可复用前缀检查点。
///
/// 如果连续密钥共享开头若干字节，可以从检查点继续剩余交换，避免重复执行
/// 已经确定的依赖链。检查点最多覆盖第一轮的 256 个状态槽。
#[derive(Debug, Clone)]
pub struct Rc4KeySchedulePrefix {
    key_prefix: Box<[u8]>,
    main_val: [u8; VAL_LEN],
    j: u8,
}

impl Rc4KeySchedulePrefix {
    pub fn new(key_prefix: &[u8]) -> Self {
        assert!(!key_prefix.is_empty(), "RC4 KSA 前缀不能为空");
        let scheduled_len = key_prefix.len().min(VAL_LEN);
        let key_prefix: Box<[u8]> = key_prefix[..scheduled_len].into();
        let mut main_val = VAL_INIT;
        let mut j = 0u8;
        for x in 0..scheduled_len {
            apply_key_scheduling_step(&mut main_val, &key_prefix, x, x, &mut j);
        }
        Self { key_prefix, main_val, j }
    }

    pub fn len(&self) -> usize { self.key_prefix.len() }

    pub fn is_empty(&self) -> bool { self.key_prefix.is_empty() }
}

impl Default for RC4 {
    fn default() -> Self {
        RC4 {
            i: 0,
            j: 0,
            main_val: VAL_INIT,
            #[cfg(not(feature = "no_debug"))]
            byte_count: 0,
        }
    }
}

#[allow(unused)]
impl RC4 {
    #[inline]
    pub const fn val_len() -> usize { VAL_LEN }

    #[inline]
    pub fn get_val(&self, index: u8) -> u8 { self.main_val[index as usize] }

    #[inline]
    /// 不做边界检查地读取 S-box，不推进随机状态。
    ///
    /// # Safety
    ///
    /// `index` 为 u8，天然小于固定的 256 字节数组长度，没有额外的下标前置条件。
    pub unsafe fn get_val_unchecked(&self, index: u8) -> u8 { unsafe { *self.main_val.get_unchecked(index as usize) } }

    #[inline]
    pub fn set_val(&mut self, index: u8, value: u8) { self.main_val[index as usize] = value; }

    /// 从恒等排列开始执行 `round` 轮密钥调度，并将 i/j 设为 0。
    ///
    /// `round == 0` 不读取密钥，返回未调度状态；正轮数要求密钥非空，否则 panic。
    /// 密钥按传入字节原样使用，不在此处转换字符编码或混入额外随机材料。
    ///
    /// ```dart
    /// RC4(List<int> key, [int round = 1]) {
    ///   val = new List<int>(256);
    ///   for (int x = 0; x < 256; ++x) {
    ///     val[x] = x;
    ///   }
    ///   int keylen = key.length;
    ///   for (int r = 0; r < round; ++r) {
    ///     int j = 0;
    ///     for (int i = 0; i < 256; ++i) {
    ///       int keyv = key[i % keylen];
    ///       j = (j + val[i] + keyv) & 255;
    ///       int t = val[i];
    ///       val[i] = val[j];
    ///       val[j] = t;
    ///     }
    ///   }
    ///   i = j = 0;
    /// }
    /// ```
    pub fn new(keys: &[u8], round: usize) -> Self {
        let mut val = VAL_INIT;
        for _ in 0..round {
            apply_key_scheduling(&mut val, keys);
        }
        RC4 {
            i: 0,
            j: 0,
            main_val: val,
            #[cfg(not(feature = "no_debug"))]
            byte_count: 0,
        }
    }

    /// 尝试从第一轮 KSA 前缀检查点构造状态。
    ///
    /// 密钥前缀不匹配时返回 `None`，调用方必须回退到完整调度。
    #[inline(never)]
    pub fn new_with_key_schedule_prefix(keys: &[u8], prefix: &Rc4KeySchedulePrefix) -> Option<Self> {
        if keys.is_empty() || !keys.starts_with(&prefix.key_prefix) {
            return None;
        }

        let mut main_val = prefix.main_val;
        let mut j = prefix.j;
        let mut key_index = prefix.len();
        if key_index == keys.len() {
            key_index = 0;
        }
        for x in prefix.len()..VAL_LEN {
            apply_key_scheduling_step(&mut main_val, keys, x, key_index, &mut j);
            key_index += 1;
            if key_index == keys.len() {
                key_index = 0;
            }
        }
        Some(Self {
            i: 0,
            j: 0,
            main_val,
            #[cfg(not(feature = "no_debug"))]
            byte_count: 0,
        })
    }

    /// 在当前 S-box 上继续执行指定轮数的密钥调度，保留 i/j 和调试计数。
    ///
    /// `round == 0` 不改变状态；正轮数要求密钥非空，否则 panic。
    /// 需要重置 PRGA 游标时使用 [`Self::round`]，而不是此方法。
    pub fn update(&mut self, keys: &[u8], round: usize) {
        for _ in 0..round {
            apply_key_scheduling(&mut self.main_val, keys);
        }
    }

    /// 对 2～4 个独立状态交错执行相同轮数的 KSA；其他宽度回退到普通路径。
    pub(crate) fn update_interleaved(states: &mut [Self], keys: &[&[u8]], round: usize) {
        assert_eq!(states.len(), keys.len(), "RC4 交错 KSA 的状态和密钥数量必须一致");
        assert!(keys.iter().all(|key| !key.is_empty()), "RC4 密钥不能为空");
        let same_key_len = keys.windows(2).all(|pair| pair[0].len() == pair[1].len());
        macro_rules! run_width {
            ($width:literal, $same_len_fn:ident) => {{
                let states: &mut [RC4; $width] = states.try_into().expect("RC4 交错 KSA 状态宽度错误");
                let keys: &[&[u8]; $width] = keys.try_into().expect("RC4 交错 KSA 密钥宽度错误");
                for _ in 0..round {
                    if same_key_len {
                        $same_len_fn(states, keys);
                    } else {
                        apply_key_scheduling_interleaved_variable_keys(states, keys);
                    }
                }
            }};
        }
        match states.len() {
            2 => run_width!(2, apply_key_scheduling_interleaved_2),
            3 => run_width!(3, apply_key_scheduling_interleaved_3),
            4 => run_width!(4, apply_key_scheduling_interleaved_4),
            _ => {
                for (state, key) in states.iter_mut().zip(keys) {
                    state.update(key, round);
                }
            }
        }
    }

    /// 推进一步 PRGA。`i`、`j` 只取低 8 位，三个状态下标因此天然位于 256 字节 S-box 内。
    #[inline(always)]
    fn next_u8_untracked(&mut self) -> u8 {
        let i = (self.i as u8).wrapping_add(1);
        let state = self.main_val.as_mut_ptr();
        // SAFETY: i 与 j 均为 u8；si、sj 也来自 u8 S-box，因此三个 add 下标都在 0..256。
        // 先读取交换前的 si/sj 后直接写回，i == j 时仍与普通 swap 完全等价。
        unsafe {
            let i_ptr = state.add(i as usize);
            let si = *i_ptr;
            let j = (self.j as u8).wrapping_add(si);
            let j_ptr = state.add(j as usize);
            let sj = *j_ptr;
            *i_ptr = sj;
            *j_ptr = si;
            self.i = u32::from(i);
            self.j = u32::from(j);
            *state.add(si.wrapping_add(sj) as usize)
        }
    }

    /// 依次以随机流异或并原地覆盖输入，每个字节推进一次 PRGA。
    ///
    /// 不将变换结果反馈给 j；空切片不改变状态，也不分配输出缓冲。
    /// ```dart
    /// void xorBytes(List<int> bytes) {
    ///    int t, len = bytes.length;
    ///    for (int x = 0; x < len; ++x) {
    ///      i = (i + 1) & 255;
    ///      j = (j + S[i]) & 255;
    ///      t = S[i];
    ///      S[i] = S[j];
    ///      S[j] = t;
    ///      bytes[x] ^= S[(S[i] + S[j]) & 255];
    ///    }
    ///  }
    /// ```
    #[inline]
    pub fn xor_bytes(&mut self, bytes: &mut [u8]) {
        for byte in bytes.iter_mut() {
            *byte ^= self.next_u8_untracked();
        }
    }

    /// 原地异或字节，并把每个变换后的字节累加到 j，对齐游戏的反馈混入规则。
    ///
    /// 每个输入字节推进一次 PRGA；与 [`Self::encrypt_bytes`] 使用相同状态变换，
    /// 不能用没有反馈的 [`Self::xor_bytes`] 替换。空切片不改变状态。
    #[inline]
    pub fn js_xor_bytes(&mut self, bytes: &mut [u8]) {
        for byte in bytes.iter_mut() {
            *byte ^= self.next_u8_untracked();
            self.j = u32::from((self.j as u8).wrapping_add(*byte));
        }
    }

    /// 按 UTF-8 字节推进 PRGA，再将 j 设为输入字节与随机字节的异或结果。
    ///
    /// 输入字符串保持不变，不分配或返回变换文本；这不是 [`Self::xor_bytes`] 的字符串包装。
    #[inline]
    pub fn xor_str(&mut self, bytes: &str) {
        for byte in bytes.as_bytes() {
            self.j = u32::from(*byte ^ self.next_u8_untracked());
        }
    }

    /// 按 UTF-8 字节应用游戏反馈混入规则，只修改随机状态。
    ///
    /// 状态变化等价于将字符串复制为字节缓冲后调用 [`Self::js_xor_bytes`]，
    /// 但不创建缓冲，也不保留变换后的字节；空字符串不改变状态。
    #[inline]
    pub fn js_xor_str(&mut self, bytes: &str) {
        for byte in bytes.as_bytes() {
            let val = *byte ^ self.next_u8_untracked();
            self.j = u32::from((self.j as u8).wrapping_add(val));
        }
    }

    /// 原地执行游戏兼容的反馈异或，每个密文字节再累加到 j。
    ///
    /// 解码需从相同初始状态调用 [`Self::decrypt_bytes`]；此处用于兼容游戏算法，
    /// 不提供认证或现代加密安全保证。每个输入字节推进一次 PRGA。
    /// ```dart
    /// 自定义加密流程
    /// void encryptBytes(List<int> bytes) {
    ///   int t, len = bytes.length;
    ///   for (int x = 0; x < len; ++x) {
    ///     i = (i + 1) & 255;
    ///     j = (j + S[i]) & 255;
    ///     t = S[i];
    ///     S[i] = S[j];
    ///     S[j] = t;
    ///     bytes[x] ^= S[(S[i] + S[j]) & 255];
    ///     j = (j + bytes[x]) & 255;
    ///   }
    /// }
    /// ```
    #[inline]
    pub fn encrypt_bytes(&mut self, bytes: &mut [u8]) {
        for byte in bytes.iter_mut() {
            *byte ^= self.next_u8_untracked();
            self.j = u32::from((self.j as u8).wrapping_add(*byte));
        }
    }

    /// 以 UTF-8 字节执行反馈异或的状态推进，不修改输入也不返回密文。
    ///
    /// 状态变化等价于复制字节后调用 [`Self::encrypt_bytes`]；用于初始化时混入身份，
    /// 无需为仅关心随机状态的调用创建临时字节缓冲。
    pub fn encrypt_bytes_no_change(&mut self, bytes: &str) {
        for byte in bytes.as_bytes() {
            let encrypted = *byte ^ self.next_u8_untracked();
            self.j = u32::from((self.j as u8).wrapping_add(encrypted));
        }
    }

    /// 原地解码反馈异或字节，并将覆盖前的密文字节累加到 j。
    ///
    /// 调用前状态应与编码前一致；每个输入字节推进一次 PRGA，空切片不改变状态。
    /// ```dart
    /// 自定义解密流程
    /// void decryptBytes(List<int> bytes) {
    ///   int t, len = bytes.length;
    ///   for (int x = 0; x < len; ++x) {
    ///     i = (i + 1) & 255;
    ///     j = (j + S[i]) & 255;
    ///     t = S[i];
    ///     S[i] = S[j];
    ///     S[j] = t;
    ///     int byte = bytes[x];
    ///     bytes[x] ^= S[(S[i] + S[j]) & 255];
    ///     j = (j + byte) & 255;
    ///   }
    /// }
    /// ```
    #[inline]
    pub fn decrypt_bytes(&mut self, bytes: &mut [u8]) {
        for byte in bytes.iter_mut() {
            let byte_v = *byte;
            *byte ^= self.next_u8_untracked();
            self.j = u32::from((self.j as u8).wrapping_add(byte_v));
        }
    }

    /// 推进一次 PRGA 并返回一个 0 到 255 的字节。
    ///
    /// 开启调试探针时额外记录计数和调用位置；`no_debug` 不改变随机输出。
    /// ```dart
    /// int nextByte() {
    ///  i = (i + 1) & 255; // 255
    ///  j = (j + S[i]) & 255; // 255
    ///  int t = S[i];
    ///  S[i] = S[j];
    ///  S[j] = t;
    ///  return S[(S[i] + S[j]) & 255];
    /// }
    /// ```
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn next_u8(&mut self) -> u8 {
        let val = self.next_u8_untracked();
        #[cfg(not(feature = "no_debug"))]
        {
            if crate::debug::probe_rc4().is_some() {
                self.byte_count += 1;
                let loc = std::panic::Location::caller();
                let file = loc.file();
                let short = file.rsplit_once(['\\', '/']).map(|(_, f)| f).unwrap_or(file);
                eprintln!(
                    "[rc4] n={} i={} j={} val={} at {}:{}",
                    self.byte_count,
                    self.i,
                    self.j,
                    val,
                    short,
                    loc.line()
                );
            }
        }
        val
    }

    /// 返回下一次 [`Self::next_u8`] 将产生的字节，不改变排列、游标或调试计数。
    ///
    /// 输出下标可能与本步交换的 i/j 重合，因此观察值也必须反映交换后的排列。
    /// 连续观察返回相同结果，只有真正消费随机字节才会推进状态。
    #[inline]
    pub fn peek_next_u8(&self) -> u8 {
        let i = (self.i as u8).wrapping_add(1);
        let si = self.main_val[i as usize];
        let j = (self.j as u8).wrapping_add(si);
        let sj = self.main_val[j as usize];
        let output = si.wrapping_add(sj);
        if output == i {
            sj
        } else if output == j {
            si
        } else {
            self.main_val[output as usize]
        }
    }

    /// 按游戏的逐字节拼接和取模规则生成 `[0, max)` 的整数。
    ///
    /// 兼容使用范围为 `0..=0x0080_0000`：零上界直接返回 0，不消费字节；
    /// 正上界先取一个字节，再按上界的有效字节数追加字节，至少共消费两个。
    /// 负上界的算术右移不会终止，过大上界可能使 i32 拼接变为负数，
    /// 因而此入口不是覆盖完整 i32 范围的通用采样器。
    /// ```javascript
    /// ax(a) {
    ///     // nextInt
    ///     var n, round
    ///     if (a === 0) return 0
    ///     n = this.n()
    ///     round = a
    ///     do {
    ///         n = (n << 8 | this.n()) >>> 0
    ///         if (n >= a) n = C.JsInt.V(n, a)
    ///         round = C.JsInt.am(round, 8)
    ///     } while (round !== 0)
    ///     return n
    /// }
    /// ```
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn next_i32(&mut self, max: i32) -> i32 {
        if max == 0 {
            return 0;
        }
        let mut round = max;
        let mut v = self.next_u8() as i32;
        loop {
            v = (v << 8) | self.next_u8() as i32;
            if v >= max {
                v %= max;
            }
            round >>= 8;
            // rc4 lib 里是 6, md5.js( 继承的 R ) 里是 8
            if round == 0 {
                break;
            }
        }
        v
    }

    /// 对当前排列执行密钥调度，然后将 i/j 游标都重置为 0。
    ///
    /// `None` 表示一轮；`Some(0)` 不调度排列，但仍重置游标。正轮数要求
    /// 密钥非空，否则 panic；调试计数不重置。需要保留游标时使用 [`Self::update`]。
    ///
    /// ```dart
    /// void round(List<int> key, [int round = 1]) {
    ///  int keylen = key.length;
    ///  for (int r = 0; r < round; ++r) {
    ///    int j = 0;
    ///    for (int i = 0; i < 256; ++i) {
    ///      int keyv = key[i % keylen];
    ///      j = (j + val[i] + keyv) & 255;
    ///      int t = val[i];
    ///      val[i] = val[j];
    ///      val[j] = t;
    ///    }
    ///  }
    ///  i = j = 0;
    ///}
    /// ```
    #[inline]
    pub fn round(&mut self, keys: &[u8], round: Option<usize>) {
        for _ in 0..round.unwrap_or(1) {
            apply_key_scheduling(&mut self.main_val, keys);
        }
        self.i = 0;
        self.j = 0;
    }

    /// 按游戏的两轮索引置换原地打乱列表，不要求元素实现 Clone。
    ///
    /// 空列表和单元素列表不推进随机流；其他情况按固定顺序调用 `2 * len` 次
    /// [`Self::next_i32`]，使用 O(len) 的临时索引空间。列表长度应落在该采样方法的范围内。
    /// ```dart
    /// List<T> sortList<T>(List<T> list) {
    ///   if (list.length <= 1) {
    ///       return list;
    ///     }
    ///     int n = list.length;
    ///     List<int> X = [];
    ///     X.length = n;
    ///     for (int i = 0; i < n; ++i) {
    ///       X[i] = i;
    ///     }
    ///     int b = 0;
    ///     for (int i = 0; i < 2; ++i) {
    ///         for (int a = 0; a < n; ++a) {
    ///             int keyv = nextInt(n);
    ///             b = (b + X[a] + keyv) % n;
    ///             int t = X[a];
    ///             X[a] = X[b];
    ///             X[b] = t;
    ///         }
    ///     }
    ///     return X.map((e) => list[e]).toList();
    /// }
    /// ```
    #[inline]
    pub fn sort_list<T>(&mut self, list: &mut [T]) {
        if list.len() <= 1 {
            return;
        }
        let n = list.len();
        let mut order: Vec<usize> = (0..n).collect();
        let mut b = 0usize;
        for _ in 0..2 {
            for a in 0..n {
                let key_v = self.next_i32(n as i32) as usize;
                let t = order[a];
                b = (b + t + key_v) % n;
                order[a] = order[b];
                order[b] = t;
            }
        }
        let mut old_to_new = vec![0usize; n];
        for (new_idx, old_idx) in order.into_iter().enumerate() {
            old_to_new[old_idx] = new_idx;
        }
        for i in 0..n {
            while old_to_new[i] != i {
                let next = old_to_new[i];
                list.swap(i, next);
                old_to_new.swap(i, next);
            }
        }
    }

    /// 返回选中元素的下标，不访问或移动元素本身。
    ///
    /// 空列表返回 `None`，单元素列表返回 `Some(0)`，两者都不消费随机字节；
    /// 更长列表调用 [`Self::next_i32`]，长度须处于其兼容范围内。
    /// ```dart
    /// T pick<T>(List<T> list) {
    ///   if (list != null) {
    ///     if (list.length == 1) {
    ///       return list[0];
    ///     } else if (list.length > 1) {
    ///       return list[nextInt(list.length)];
    ///     }
    ///   }
    ///   return null;
    /// }
    /// ```
    #[inline]
    pub fn pick<T>(&mut self, list: &[T]) -> Option<usize> {
        match list.len() {
            1 => Some(0),
            n if n > 1 => Some(self.next_i32(n as i32) as usize),
            _ => None,
        }
    }

    /// 返回避开指定位置的元素下标，不移动元素。
    ///
    /// 多元素列表要求 `skip_after_index < list.len()`，不在此处校验；
    /// 先从少一个位置的范围采样，再将落在排除位置之后的下标加一。
    /// 空列表返回 `None`；单元素列表在排除 0 时返回 `None`，否则返回 `Some(0)`，
    /// 这两种长度都不消费字节。双元素列表仍会调用上界为 1 的采样并推进随机流。
    ///
    /// ```dart
    ///  T pickSkip<T>(List<T> list, T obj) {
    ///    if (list != null) {
    ///      if (list.length == 1) {
    ///        if (list[0] != obj) {
    ///          return list[0];
    ///        }
    ///      } else if (list.length > 1) {
    ///        int pos = list.indexOf(obj);
    ///        if (pos < 0) {
    ///          return list[nextInt(list.length)];
    ///        }
    ///        int n = nextInt(list.length - 1);
    ///        if (n >= pos) {
    ///          ++n;
    ///        }
    ///        return list[n];
    ///      }
    ///    }
    ///    return null;
    ///  }
    /// ```
    #[inline]
    pub fn pick_skip<T>(&mut self, list: &[T], skip_after_index: usize) -> Option<usize> {
        match list.len() {
            1 => {
                if skip_after_index == 0 {
                    None
                } else {
                    Some(0)
                }
            }
            n if n > 1 => {
                let n = self.next_i32((n - 1) as i32) as usize;
                if n >= skip_after_index { Some(n + 1) } else { Some(n) }
            }
            _ => None,
        }
    }

    /// 从列表中排除一个连续区间后返回选中下标。
    ///
    /// `skips` 必须描述有效、升序、连续的下标区间；实现仅使用首项与长度，
    /// 不读取后续下标，也不验证连续性，不能传入任意离散集合。
    /// 空 `skips` 委托 [`Self::pick`]；排除长度不少于列表长度时返回 `None` 且不消费字节，
    /// 否则按剩余长度采样，再将跨过排除区间的下标整体后移。
    ///
    /// ```dart
    /// T pickSkipRange<T>(List<T> list, List<T> skips) {
    ///   if (skips == null || skips.isEmpty) {
    ///       return pick(list);
    ///     }
    ///     T first = skips.first;
    ///     int skiplen = skips.length;
    ///     if (list != null) {
    ///       if (list.length > skiplen) {
    ///         int pos = list.indexOf(first);
    ///         int n = nextInt(list.length - skiplen);
    ///         if (n >= pos) {
    ///           n += skiplen;
    ///         }
    ///         return list[n];
    ///       }
    ///     }
    ///     return null;
    /// }
    /// ```
    #[inline]
    pub fn pick_skip_range<T>(&mut self, list: &[T], skips: &[usize]) -> Option<usize> {
        if skips.is_empty() {
            return self.pick(list);
        }
        let first = skips[0];
        let skip_len = skips.len();
        if list.len() > skip_len {
            let n = self.next_i32((list.len() - skip_len) as i32) as usize;
            if n >= first { Some(n + skip_len) } else { Some(n) }
        } else {
            None
        }
    }

    // 一大堆判定是否小于指定数字的函数

    /// next_u8 是否小于 240
    #[inline]
    pub fn c94(&mut self) -> bool { self.next_u8() < 240 }

    /// next_u8 是否小于 192
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn c75(&mut self) -> bool { self.next_u8() < 192 }

    /// next_u8 是否小于 128
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn c50(&mut self) -> bool { self.next_u8() < 128 }

    /// next_u8 是否小于 64
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn c25(&mut self) -> bool { self.next_u8() < 64 }

    /// next_u8 是否小于 32
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn c12(&mut self) -> bool { self.next_u8() < 32 }

    /// next_u8 是否小于 84
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn c33(&mut self) -> bool { self.next_u8() < 84 }

    /// next_u8 是否小于 171
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn c66(&mut self) -> bool { self.next_u8() < 171 }

    // 两个颜色拼接

    /// 顺序消费三个字节，高字节在前，拼接为 24 位 RGB 值或无符号整数。
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    #[allow(non_snake_case)]
    pub fn rFFFFFF(&mut self) -> u32 { ((self.next_u8() as u32) << 16) | ((self.next_u8() as u32) << 8) | self.next_u8() as u32 }

    /// 顺序消费两个字节，高字节在前，拼接为 0 到 65535 的无符号整数。
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    #[allow(non_snake_case)]
    pub fn rFFFF(&mut self) -> u32 { ((self.next_u8() as u32) << 8) | self.next_u8() as u32 }

    // 一些指定范围的随机数

    /// 生成一个 1-256 的随机数
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn r256(&mut self) -> u32 { self.next_u8() as u32 + 1 }

    /// 生成一个 1-64 的随机数
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn r64(&mut self) -> u32 { (self.next_u8() as u32 & 63) + 1 }

    /// 生成一个 1-16 的随机数
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn r16(&mut self) -> u32 { (self.next_u8() as u32 & 15) + 1 }

    /// 生成一个 0-255 的随机数
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn r255(&mut self) -> u32 { self.next_u8() as u32 }

    /// 生成一个 0-127 的随机数
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn r127(&mut self) -> u32 { self.next_u8() as u32 & 127 }

    /// 生成一个 0-63 的随机数
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn r63(&mut self) -> u32 { self.next_u8() as u32 & 63 }

    /// 生成一个 0-31 的随机数
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn r31(&mut self) -> u32 { self.next_u8() as u32 & 31 }

    /// 生成一个 0-15 的随机数
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn r15(&mut self) -> u32 { self.next_u8() as u32 & 15 }

    /// 生成一个 0-7 的随机数
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn r7(&mut self) -> u32 { self.next_u8() as u32 & 7 }

    /// 生成一个 0-3 的随机数
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn r3(&mut self) -> u32 { self.next_u8() as u32 & 3 }

    /// 用于 `req mp` 判定的非均匀 1 到 9 随机值。
    ///
    /// 只消费一个字节，将高低半字节分别加一后相乘，再右移五位并加一；
    /// 不能用均匀的范围采样替代，否则结果分布与后续随机流都会变化。
    /// ```dart
    /// int get r3x3 {
    ///   int b = nextByte();
    ///   int b1 = (b & 15) + 1;
    ///   int b2 = ((b >> 4) & 15) + 1;
    ///
    ///   return ((b1 * b2) >> 5) + 1;
    /// }
    /// ```
    #[inline]
    #[cfg_attr(not(feature = "no_debug"), track_caller)]
    pub fn r3x3(&mut self) -> u32 {
        let b = self.next_u8();
        let b1 = (b & 15) + 1;
        let b2 = ((b >> 4) & 15) + 1;
        ((b1 as u32 * b2 as u32) >> 5) + 1
    }
}

#[cfg(test)]
#[path = "rc4_tests.rs"]
mod tests;
