//! A small baseline JPEG decoder. It uses the integer maths of the classic
//! reference decoder (slow-integer IDCT and fixed point YCbCr -> RGB), which
//! is what the original program's runtime uses, so pictures convert to the
//! same pixel values. Files it doesn't handle (progressive, subsampled
//! colour...) return None and go to the general image loader instead.

const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

#[derive(Clone, Default)]
struct Huff {
    maxcode: [i32; 18],
    mincode: [i32; 17],
    valptr: [i32; 17],
    vals: Vec<u8>,
}

impl Huff {
    fn new(bits: &[u8; 17], vals: Vec<u8>) -> Huff {
        let mut h = Huff { vals, ..Default::default() };
        let mut code = 0i32;
        let mut p = 0i32;
        for l in 1..=16 {
            h.valptr[l] = p;
            h.mincode[l] = code;
            code += bits[l] as i32;
            p += bits[l] as i32;
            h.maxcode[l] = if bits[l] > 0 { code - 1 } else { -1 };
            code <<= 1;
        }
        h.maxcode[17] = i32::MAX;
        h
    }
}

struct Bits<'a> {
    d: &'a [u8],
    p: usize,
    buf: u32,
    n: u32,
}

impl<'a> Bits<'a> {
    fn fill(&mut self) {
        while self.n <= 24 {
            let mut b = 0u32;
            if self.p < self.d.len() {
                let x = self.d[self.p];
                if x == 0xff {
                    let nx = self.d.get(self.p + 1).copied().unwrap_or(0);
                    if nx == 0 {
                        self.p += 2;
                        b = 0xff;
                    }
                    // a marker: feed zeros and stay in front of it
                } else {
                    self.p += 1;
                    b = x as u32;
                }
            }
            self.buf |= b << (24 - self.n);
            self.n += 8;
        }
    }
    fn bits(&mut self, k: u32) -> i32 {
        if k == 0 {
            return 0;
        }
        self.fill();
        let v = self.buf >> (32 - k);
        self.buf <<= k;
        self.n -= k;
        v as i32
    }
    fn bit(&mut self) -> i32 {
        self.bits(1)
    }
    fn decode(&mut self, h: &Huff) -> Option<u8> {
        let mut l = 1usize;
        let mut code = self.bit();
        while code > h.maxcode[l] {
            code = (code << 1) | self.bit();
            l += 1;
            if l > 16 {
                return None;
            }
        }
        h.vals.get((h.valptr[l] + code - h.mincode[l]) as usize).copied()
    }
    fn extend(v: i32, s: u32) -> i32 {
        if s == 0 {
            0
        } else if v < (1 << (s - 1)) {
            v + (-1 << s) + 1
        } else {
            v
        }
    }
    /// skips to the next restart marker
    fn restart(&mut self) {
        self.buf = 0;
        self.n = 0;
        while self.p + 1 < self.d.len() {
            if self.d[self.p] == 0xff && (0xd0..=0xd7).contains(&self.d[self.p + 1]) {
                self.p += 2;
                return;
            }
            self.p += 1;
        }
    }
}

fn range(v: i32) -> u8 {
    // the reference decoder's post-IDCT range limit table (with its wrap around)
    let idx = v & 1023;
    if idx < 128 {
        (idx + 128) as u8
    } else if idx < 512 {
        255
    } else if idx < 896 {
        0
    } else {
        (idx - 896) as u8
    }
}

const CB: i32 = 13;
const P1: i32 = 2;
fn descale(x: i32, n: i32) -> i32 {
    (x + (1 << (n - 1))) >> n
}

/// slow-but-accurate integer IDCT; `c` is dequantized, natural order
fn idct(c: &[i32; 64], out: &mut [u8; 64]) {
    let mut ws = [0i32; 64];
    let one = |i: [i32; 8]| -> [i32; 8] {
        let (mut z2, mut z3) = (i[2], i[6]);
        let mut z1 = (z2 + z3) * 4433;
        let mut tmp2 = z1 + z3 * -15137;
        let mut tmp3 = z1 + z2 * 6270;
        z2 = i[0];
        z3 = i[4];
        let mut tmp0 = (z2 + z3) << CB;
        let mut tmp1 = (z2 - z3) << CB;
        let tmp10 = tmp0 + tmp3;
        let tmp13 = tmp0 - tmp3;
        let tmp11 = tmp1 + tmp2;
        let tmp12 = tmp1 - tmp2;
        tmp0 = i[7];
        tmp1 = i[5];
        tmp2 = i[3];
        tmp3 = i[1];
        z1 = tmp0 + tmp3;
        z2 = tmp1 + tmp2;
        z3 = tmp0 + tmp2;
        let mut z4 = tmp1 + tmp3;
        let z5 = (z3 + z4) * 9633;
        tmp0 *= 2446;
        tmp1 *= 16819;
        tmp2 *= 25172;
        tmp3 *= 12299;
        z1 *= -7373;
        z2 *= -20995;
        z3 *= -16069;
        z4 *= -3196;
        z3 += z5;
        z4 += z5;
        tmp0 += z1 + z3;
        tmp1 += z2 + z4;
        tmp2 += z2 + z3;
        tmp3 += z1 + z4;
        [tmp10 + tmp3, tmp11 + tmp2, tmp12 + tmp1, tmp13 + tmp0, tmp13 - tmp0, tmp12 - tmp1, tmp11 - tmp2, tmp10 - tmp3]
    };
    for col in 0..8 {
        let i: [i32; 8] = std::array::from_fn(|r| c[r * 8 + col]);
        if i[1..].iter().all(|x| *x == 0) {
            for r in 0..8 {
                ws[r * 8 + col] = i[0] << P1;
            }
            continue;
        }
        let o = one(i);
        for r in 0..8 {
            ws[r * 8 + col] = descale(o[r], CB - P1);
        }
    }
    for row in 0..8 {
        let i: [i32; 8] = std::array::from_fn(|k| ws[row * 8 + k]);
        if i[1..].iter().all(|x| *x == 0) {
            let v = range(descale(i[0], P1 + 3));
            for k in 0..8 {
                out[row * 8 + k] = v;
            }
            continue;
        }
        let o = one(i);
        for k in 0..8 {
            out[row * 8 + k] = range(descale(o[k], CB + P1 + 3));
        }
    }
}

struct Comp {
    id: u8,
    h: u8,
    v: u8,
    tq: usize,
    td: usize,
    ta: usize,
    plane: Vec<u8>,
}

pub fn decode(d: &[u8]) -> Option<(usize, usize, Vec<u32>)> {
    if d.len() < 4 || d[0] != 0xff || d[1] != 0xd8 {
        return None;
    }
    let mut p = 2;
    let mut qt = [[0i32; 64]; 4];
    let mut dc: Vec<Huff> = vec![Huff::default(); 4];
    let mut ac: Vec<Huff> = vec![Huff::default(); 4];
    let mut comps: Vec<Comp> = vec![];
    let (mut w, mut h) = (0usize, 0usize);
    let mut ri = 0usize;
    let mut adobe: Option<u8> = None;
    let mut done = false;
    while p + 4 <= d.len() && !done {
        if d[p] != 0xff {
            p += 1;
            continue;
        }
        let m = d[p + 1];
        if m == 0xff {
            p += 1;
            continue;
        }
        if m == 0xd8 || (0xd0..=0xd7).contains(&m) || m == 0x01 {
            p += 2;
            continue;
        }
        if m == 0xd9 {
            break;
        }
        let len = (d[p + 2] as usize) << 8 | d[p + 3] as usize;
        let seg = d.get(p + 4..p + 2 + len)?;
        match m {
            0xc0 | 0xc1 => {
                if seg[0] != 8 {
                    return None;
                }
                h = (seg[1] as usize) << 8 | seg[2] as usize;
                w = (seg[3] as usize) << 8 | seg[4] as usize;
                let n = seg[5] as usize;
                for k in 0..n {
                    let b = &seg[6 + k * 3..9 + k * 3];
                    comps.push(Comp { id: b[0], h: b[1] >> 4, v: b[1] & 15, tq: (b[2] & 3) as usize, td: 0, ta: 0, plane: vec![] });
                }
            }
            0xc2..=0xcf if m != 0xc4 && m != 0xc8 && m != 0xcc => return None,
            0xc4 => {
                let mut q = 0;
                while q < seg.len() {
                    let tc = seg[q] >> 4;
                    let th = (seg[q] & 3) as usize;
                    let mut bits = [0u8; 17];
                    bits[1..17].copy_from_slice(seg.get(q + 1..q + 17)?);
                    let total: usize = bits.iter().map(|b| *b as usize).sum();
                    let vals = seg.get(q + 17..q + 17 + total)?.to_vec();
                    let t = Huff::new(&bits, vals);
                    if tc == 0 {
                        dc[th] = t;
                    } else {
                        ac[th] = t;
                    }
                    q += 17 + total;
                }
            }
            0xdb => {
                let mut q = 0;
                while q < seg.len() {
                    let pq = seg[q] >> 4;
                    let tq = (seg[q] & 3) as usize;
                    for k in 0..64 {
                        let v = if pq == 0 { *seg.get(q + 1 + k)? as i32 } else { (*seg.get(q + 1 + 2 * k)? as i32) << 8 | *seg.get(q + 2 + 2 * k)? as i32 };
                        qt[tq][ZIGZAG[k]] = v;
                    }
                    q += 1 + if pq == 0 { 64 } else { 128 };
                }
            }
            0xdd => ri = (seg[0] as usize) << 8 | seg[1] as usize,
            0xee => {
                if seg.len() >= 12 && &seg[0..5] == b"Adobe" {
                    adobe = Some(seg[11]);
                }
            }
            0xda => {
                let n = seg[0] as usize;
                if comps.is_empty() || n != comps.len() || comps.iter().any(|c| c.h != 1 || c.v != 1) {
                    return None;
                }
                for k in 0..n {
                    let id = seg[1 + k * 2];
                    let t = seg[2 + k * 2];
                    let c = comps.iter_mut().find(|c| c.id == id)?;
                    c.td = (t >> 4) as usize & 3;
                    c.ta = (t & 3) as usize;
                }
                let start = p + 2 + len;
                let bx = w.div_ceil(8);
                let by = h.div_ceil(8);
                for c in comps.iter_mut() {
                    c.plane = vec![0; bx * 8 * by * 8];
                }
                let mut bits = Bits { d, p: start, buf: 0, n: 0 };
                let mut pred = vec![0i32; comps.len()];
                let total = bx * by;
                let mut block = [0u8; 64];
                for mcu in 0..total {
                    if ri > 0 && mcu > 0 && mcu % ri == 0 {
                        bits.restart();
                        pred.iter_mut().for_each(|x| *x = 0);
                    }
                    let (mx, my) = (mcu % bx, mcu / bx);
                    for (ci, c) in comps.iter_mut().enumerate() {
                        let mut coef = [0i32; 64];
                        let t = bits.decode(&dc[c.td])? as u32;
                        let diff = Bits::extend(bits.bits(t), t);
                        pred[ci] += diff;
                        coef[0] = pred[ci];
                        let mut k = 1;
                        while k < 64 {
                            let rs = bits.decode(&ac[c.ta])?;
                            let r = (rs >> 4) as usize;
                            let s = (rs & 15) as u32;
                            if s == 0 {
                                if r == 15 {
                                    k += 16;
                                    continue;
                                }
                                break;
                            }
                            k += r;
                            if k > 63 {
                                break;
                            }
                            coef[ZIGZAG[k]] = Bits::extend(bits.bits(s), s);
                            k += 1;
                        }
                        let q = &qt[c.tq];
                        for i in 0..64 {
                            coef[i] *= q[i];
                        }
                        idct(&coef, &mut block);
                        let stride = bx * 8;
                        for yy in 0..8 {
                            let o = (my * 8 + yy) * stride + mx * 8;
                            c.plane[o..o + 8].copy_from_slice(&block[yy * 8..yy * 8 + 8]);
                        }
                    }
                }
                done = true;
            }
            _ => {}
        }
        p += 2 + len;
    }
    if !done || w == 0 || h == 0 {
        return None;
    }
    let stride = w.div_ceil(8) * 8;
    let mut px = Vec::with_capacity(w * h);
    match comps.len() {
        1 => {
            for y in 0..h {
                for x in 0..w {
                    let g = comps[0].plane[y * stride + x] as u32;
                    px.push(g << 16 | g << 8 | g);
                }
            }
        }
        3 => {
            let ycc = adobe != Some(0) && !(comps[0].id == b'R' && comps[1].id == b'G' && comps[2].id == b'B');
            let fix = |x: f64| (x * 65536.0 + 0.5) as i32;
            let clamp = |v: i32| v.clamp(0, 255) as u32;
            for y in 0..h {
                for x in 0..w {
                    let o = y * stride + x;
                    let (a, b, c) = (comps[0].plane[o] as i32, comps[1].plane[o] as i32, comps[2].plane[o] as i32);
                    if ycc {
                        let (cb, cr) = (b - 128, c - 128);
                        let r = a + ((fix(1.40200) * cr + 32768) >> 16);
                        let g = a + ((-fix(0.34414) * cb + 32768 - fix(0.71414) * cr) >> 16);
                        let bl = a + ((fix(1.77200) * cb + 32768) >> 16);
                        px.push(clamp(r) << 16 | clamp(g) << 8 | clamp(bl));
                    } else {
                        px.push((a as u32) << 16 | (b as u32) << 8 | c as u32);
                    }
                }
            }
        }
        _ => return None,
    }
    Some((w, h, px))
}
