/// 用于初始化类似下面那个 VAL_init 的大号数组的宏
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
/// 我是不是应该用宏来写这个玩意
const VAL_INIT: [u8; 256] = val!();

/// RC4 类
/// 名竞的核心~
#[allow(unused)]
pub struct RC4 {
    i: u32,
    j: u32,
    /// [u8, 256]
    main_val: Vec<u8>,
}

#[allow(unused)]
impl RC4 {
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
    pub fn new(keys: Vec<u8>, round: Option<usize>) -> Self {
        let mut val = VAL_INIT;
        let mut j = 0;

        let key_len = keys.len();
        for _ in 0..round.unwrap_or(1) {
            j = 0;
            for x in 0..256 {
                let key_v = keys[x % key_len];
                j = (j + val[x] as u32 + key_v as u32) & 255;
                val.swap(x, j as usize);
            }
        }
        RC4 {
            i: 0,
            j: 0,
            main_val: VAL_INIT.to_vec(),
        }
    }

    /// 异或字节
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
    pub fn xor_bytes(&mut self, bytes: &mut [u8]) {
        for byte in bytes.iter_mut() {
            self.i = (self.i + 1) & 255;
            self.j = (self.j + self.main_val[self.i as usize] as u32) & 255;
            self.main_val.swap(self.i as usize, self.j as usize);
            *byte ^=
                self.main_val[(self.main_val[self.i as usize] as u32 + self.main_val[self.j as usize] as u32) as usize & 255];
        }
    }

    /// 加密字节
    /// ```dart
    /// custom encryption
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
    pub fn encrypt_bytes(&mut self, bytes: &mut [u8]) {
        for byte in bytes.iter_mut() {
            self.i = (self.i + 1) & 255;
            self.j = (self.j + self.main_val[self.i as usize] as u32) & 255;
            self.main_val.swap(self.i as usize, self.j as usize);
            *byte ^=
                self.main_val[(self.main_val[self.i as usize] as u32 + self.main_val[self.j as usize] as u32) as usize & 255];
            self.j = (self.j + *byte as u32) & 255;
        }
    }

    /// 解密字节
    /// ```dart
    /// custom decryption
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
    pub fn decrypt_bytes(&mut self, bytes: &mut [u8]) {
        for byte in bytes.iter_mut() {
            self.i = (self.i + 1) & 255;
            self.j = (self.j + self.main_val[self.i as usize] as u32) & 255;
            self.main_val.swap(self.i as usize, self.j as usize);
            let byte_v = *byte;
            *byte ^=
                self.main_val[(self.main_val[self.i as usize] as u32 + self.main_val[self.j as usize] as u32) as usize & 255];
            self.j = (self.j + byte_v as u32) & 255;
        }
    }

    /// 生成 u8 随机数
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
    pub fn next_u8(&mut self) -> u8 {
        self.i = (self.i + 1) & 255;
        self.j = (self.j + self.main_val[self.i as usize] as u32) & 255;
        self.main_val.swap(self.i as usize, self.j as usize);
        self.main_val[(self.main_val[self.i as usize] as u32 + self.main_val[self.j as usize] as u32) as usize & 255]
    }

    /// 生成 i32 随机数
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
    pub fn next_i32(&mut self, max: i32) -> i32 {
        if max == 0 {
            return 0;
        }
        let mut round = max;
        let mut v = self.next_u8() as i32;
        loop {
            v = v << 8 | self.next_u8() as i32;
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

    /// round?
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
    /// ``
    pub fn round(&mut self, keys: Vec<u8>, round: Option<usize>) {
        let key_len = keys.len();
        for _ in 0..round.unwrap_or(1) {
            let mut j = 0;
            for i in 0..256 {
                let key_v = keys[i % key_len];
                j = (j + self.main_val[i] as u32 + key_v as u32) & 255;
                self.main_val.swap(i, j as usize);
            }
        }
        self.i = 0;
        self.j = 0;
    }

    /// 打乱列表
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
    pub fn sort_list<T>(&mut self, list: &mut [T]) {
        if list.len() <= 1 {
            return;
        }
        let n = list.len();
        // 直接对输入列表进行操作
        let mut b = 0;
        for _ in 0..2 {
            for a in 0..n {
                let key_v = self.next_i32(n as i32);
                b = (b + a + key_v as usize) % n;
                list.swap(a, b);
            }
        }
    }

    /// 从列表里选一个
    ///
    /// # note: 实际上不会选, 只会返回一个 index
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
    pub fn pick<T>(&mut self, list: &[T]) -> Option<usize> {
        match list.len() {
            1 => Some(0),
            n if n > 1 => Some(self.next_i32(n as i32) as usize),
            _ => None,
        }
    }

    /// 从列表里选一个
    /// 但是跳过指定的 index
    ///
    /// 虽然但是, 我也没懂这玩意到底是在干啥
    ///
    /// # note: 实际上不会选, 只会返回一个 index
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
                if n >= skip_after_index {
                    Some(n + 1)
                } else {
                    Some(n)
                }
            }
            _ => None,
        }
    }

    /// 从列表里选一个
    /// 但是跳过指定一些 index
    ///
    /// # 输入:
    /// - list: 原始列表
    /// - skips: 要跳过的 index
    ///
    /// # note: 实际上不会选, 只会返回一个 index
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
    pub fn pick_skip_range<T>(&mut self, list: &[T], skips: Vec<usize>) -> Option<usize> {
        if skips.is_empty() {
            return self.pick(list);
        }
        let first = skips[0];
        let skip_len = skips.len();
        if list.len() > skip_len {
            let n = self.next_i32((list.len() - skip_len) as i32) as usize;
            if n >= first {
                Some(n + skip_len)
            } else {
                Some(n)
            }
        } else {
            None
        }
    }

    // 一大堆判定是否小于指定数字的函数

    /// next_u8 是否小于 240
    pub fn c94(&mut self) -> bool { self.next_u8() < 240 }

    /// next_u8 是否小于 192
    pub fn c75(&mut self) -> bool { self.next_u8() < 192 }

    /// next_u8 是否小于 128
    pub fn c50(&mut self) -> bool { self.next_u8() < 128 }

    /// next_u8 是否小于 64
    pub fn c25(&mut self) -> bool { self.next_u8() < 64 }

    /// next_u8 是否小于 32
    pub fn c12(&mut self) -> bool { self.next_u8() < 32 }

    /// next_u8 是否小于 84
    pub fn c33(&mut self) -> bool { self.next_u8() < 84 }

    /// next_u8 是否小于 171
    pub fn c66(&mut self) -> bool { self.next_u8() < 171 }

    // 两个颜色拼接

    /// 生成一个 RGB 颜色
    #[allow(non_snake_case)]
    pub fn rFFFFFF(&mut self) -> u32 { (self.next_u8() as u32) << 16 | (self.next_u8() as u32) << 8 | self.next_u8() as u32 }

    /// 生成一个 RGB 颜色
    #[allow(non_snake_case)]
    pub fn rFFFF(&mut self) -> u32 { (self.next_u8() as u32) << 8 | self.next_u8() as u32 }

    // 一些指定范围的随机数

    /// 生成一个 1-256 的随机数
    pub fn r256(&mut self) -> u32 { self.next_u8() as u32 + 1 }

    /// 生成一个 1-64 的随机数
    pub fn r64(&mut self) -> u32 { (self.next_u8() as u32 & 63) + 1 }

    /// 生成一个 1-16 的随机数
    pub fn r16(&mut self) -> u32 { (self.next_u8() as u32 & 15) + 1 }

    /// 生成一个 0-255 的随机数
    pub fn r255(&mut self) -> u32 { self.next_u8() as u32 }

    /// 生成一个 0-127 的随机数
    pub fn r127(&mut self) -> u32 { self.next_u8() as u32 & 127 }

    /// 生成一个 0-63 的随机数
    pub fn r63(&mut self) -> u32 { self.next_u8() as u32 & 63 }

    /// 生成一个 0-31 的随机数
    pub fn r31(&mut self) -> u32 { self.next_u8() as u32 & 31 }

    /// 生成一个 0-15 的随机数
    pub fn r15(&mut self) -> u32 { self.next_u8() as u32 & 15 }

    /// 生成一个 0-7 的随机数
    pub fn r7(&mut self) -> u32 { self.next_u8() as u32 & 7 }

    /// 生成一个 0-3 的随机数
    pub fn r3(&mut self) -> u32 { self.next_u8() as u32 & 3 }

    /// ```dart
    /// used by req mp
    /// int get r3x3 {
    ///   int b = nextByte();
    ///   int b1 = (b & 15) + 1;
    ///   int b2 = ((b >> 4) & 15) + 1;
    ///
    ///   return ((b1 * b2) >> 5) + 1;
    /// }
    /// ```
    pub fn r3x3(&mut self) -> u32 {
        let b = self.next_u8();
        let b1 = (b & 15) + 1;
        let b2 = ((b >> 4) & 15) + 1;
        ((b1 as u32 * b2 as u32) >> 5) + 1
    }
}
