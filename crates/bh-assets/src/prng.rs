/// PRNG de 512 bits (estilo WELL512) usado pela cifra dos `.swz`.
pub struct Prng {
    st: [u32; 16],
    idx: usize,
}

impl Prng {
    pub fn new(seed: u32) -> Self {
        let mut st = [0u32; 16];
        st[0] = seed;
        for i in 1..16 {
            let p = st[i - 1];
            st[i] = (i as u32).wrapping_add(1812433253u32.wrapping_mul(p ^ (p >> 30)));
        }
        Self { st, idx: 0 }
    }

    pub fn next_u32(&mut self) -> u32 {
        let i = self.idx;
        let a = self.st[i];
        let b = self.st[i.wrapping_sub(3) & 15];
        let c = a ^ b ^ ((b ^ a.wrapping_mul(2)) << 15);
        let e = self.st[i.wrapping_sub(7) & 15];
        let d = (e >> 11) ^ e;
        let ni = i.wrapping_sub(1) & 15;

        self.st[i] = d ^ c;
        let old = self.st[ni];
        self.st[ni] = old
            ^ d
            ^ ((d ^ c) & 0xfed2_2169).wrapping_mul(32)
            ^ (old ^ ((c ^ (d << 10)) << 16)).wrapping_mul(4);
        self.idx = ni;
        self.st[ni]
    }
}
