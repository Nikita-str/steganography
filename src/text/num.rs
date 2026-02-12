
use std::f32::consts::LOG2_10;

pub struct Num10ToBits {
    bits: u64,
    mul: u64,
    rest_n: u8,
}

impl Num10ToBits {
    pub const fn new(chunk_bit_sz: u8) -> Self {
        debug_assert!(chunk_bit_sz <= 64);

        Self {
            bits: 0,
            mul: 1,
            rest_n: f32::ceil(chunk_bit_sz as f32 / LOG2_10) as u8,
        }
    }
    
    pub const fn new_const<const CHUNK_SZ: u8>() -> Self {
        debug_assert!(CHUNK_SZ <= 64);

        Self {
            bits: 0,
            mul: 1,
            rest_n: f32::ceil(CHUNK_SZ as f32 / LOG2_10) as u8,
        }
    }

    pub const fn new_u32() -> Self {
        Self::new_const::<32>()
    }

    /// The loss is about `~3.8%` ~ `2.43` bits
    pub const fn new_u64() -> Self {
        Self::new_const::<64>()
    }

    /// The loss is about `~0.185%` ~ `0.12` bits
    pub const fn new_low_loss() -> Self {
        Self::new_const::<63>()
    }

    pub const fn is_done(&self) -> bool {
        self.rest_n == 0
    }
    
    pub fn try_take(&self) -> Option<u64> {
        self.is_done().then_some(self.bits)
    }

    /// Handle num char, ignores all others.
    /// 
    /// # Panic
    /// * if `self.is_done()` & `ch` is num char
    pub fn next_any_char(&mut self, ch: char) -> Option<u64> {
        match ch {
            '0'..='9' => self.next_n10_char(ch),
            _ => None
        }
    }

    /// # Panic
    /// * if `self.is_done()`
    pub fn next_n10_char(&mut self, num_ch: char) -> Option<u64> {
        let n = num_ch as u8 - b'0';
        self.next_n10(n)
    }

    /// # Panic
    /// * if `self.is_done()`
    pub fn next_n10(&mut self, n: u8) -> Option<u64> {
        if self.is_done() {
            panic!("Num10ToBits: you lost a number ({n})")
        }

        self.rest_n -= 1;
        self.bits = (n as u64 * self.mul) + self.bits;
        self.mul = self.mul.wrapping_mul(10);

        self.try_take()
    }
}


pub struct BitsToNum10 {
    bits: u64,
    rest_n: u8,
}

impl BitsToNum10 {
    pub const fn new(chunk: u64, chunk_bit_sz: u8) -> Self {
        debug_assert!(chunk_bit_sz <= 64);
        debug_assert!((64 - chunk.leading_zeros()) as u8 <= chunk_bit_sz);

        Self {
            bits: chunk,
            rest_n: f32::ceil(chunk_bit_sz as f32 / LOG2_10) as u8,
        }
    }

    pub const fn new_const<const CHUNK_SZ: u8>(chunk: u64) -> Self {
        debug_assert!(CHUNK_SZ <= 64);
        debug_assert!((64 - chunk.leading_zeros()) as u8 <= CHUNK_SZ);

        Self {
            bits: chunk,
            rest_n: f32::ceil(CHUNK_SZ as f32 / LOG2_10) as u8,
        }
    }

    pub const fn new_u32(chunk: u64) -> Self {
        Self::new_const::<32>(chunk)
    }

    /// The loss is about `~3.8%` ~ `2.43` bits
    pub const fn new_u64(chunk: u64) -> Self {
        Self::new_const::<64>(chunk)
    }

    /// The loss is about `~0.185%` ~ `0.12` bits
    pub const fn new_low_loss(chunk: u64) -> Self {
        Self::new_const::<63>(chunk)
    }

    pub const fn is_done(&self) -> bool {
        self.rest_n == 0
    }

    pub fn next_n10_char(&mut self) -> Option<char> {
        self.next_n10().map(|x| (b'0' + x) as char)
    }

    pub fn next_n10(&mut self) -> Option<u8> {
        (!self.is_done()).then(||{
            self.rest_n -= 1;
            let ret = (self.bits % 10) as u8;
            self.bits /= 10;
            ret
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_num_conv() {
        let chunks = vec![
            0b_010_1010_1101,
            0b_110_1010_1101,
            0b_001_0011_0001,
            0b_111_1111_1111,
            0b_100_1000_1000,
            0b_111_1100_1111,
        ];
        let expects = vec![
            "5860",
            "9071",
            "5030",
            "7402",
            "0611",
            "9991",
        ];

        let chunk_bit_sz = 11;
        let mut nums = String::with_capacity(4);

        for (chunk, expect) in chunks.into_iter().zip(expects) {
            nums.clear();

            let mut b2n = BitsToNum10::new(chunk, chunk_bit_sz);
            while let Some(ch) = b2n.next_n10_char() {
                nums.push(ch);
            }
            assert_eq!(&nums, expect);

            let mut n2b = Num10ToBits::new(chunk_bit_sz);
            for num_ch in expect.chars() {
                n2b.next_n10_char(num_ch);
            }
            assert_eq!(n2b.try_take(), Some(chunk));
        }
    }

    #[test]
    fn test_bit_loss() {
        let bit_loss_proc = |bits: u32| {
            let bits = bits as f32;
            let ceil_bits = f32::ceil(bits / LOG2_10) * LOG2_10;
            let delta_bits = ceil_bits - bits;
            let proc = 100.00 * delta_bits / bits;
            return proc
        };

        let mut best_loss_proc = bit_loss_proc(1);
        let mut best_bits = vec![1];
        let mut best_n = vec![1];

        for bits in 2..=128 {
            let loss_proc = bit_loss_proc(bits);
            if loss_proc < best_loss_proc {
                best_loss_proc = loss_proc;
                let n = f32::ceil(bits as f32 / LOG2_10) as u32;
                if best_n.last() != Some(&n) {
                    best_n.push(n);
                }
                best_bits.push(bits);
                println!("bits = {bits:2} <--> n = {n:2} (loss = {best_loss_proc:.3}%)");
            }
        }

        assert!(best_loss_proc < 0.1);
        assert_eq!(&best_bits, &[1, 2, 3, 13, 23, 33, 43, 53, 63, 73, 83, 93]);
        assert_eq!(&best_n, &[1, 4, 7, 10, 13, 16, 19, 22, 25, 28]);
    }
}
