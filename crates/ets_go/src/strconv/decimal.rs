//! Go's `internal/strconv/decimal.go`: an arbitrary-precision decimal, used for correctly rounded
//! fixed-precision float formatting.

pub(super) struct Decimal {
    pub(super) d: [u8; 800], // digits, big-endian representation
    pub(super) nd: usize,    // number of digits used
    pub(super) dp: i32,      // decimal point
    pub(super) trunc: bool,  // discarded nonzero digits beyond d[:nd]
}

fn trim(a: &mut Decimal) {
    while a.nd > 0 && a.d[a.nd - 1] == b'0' {
        a.nd -= 1;
    }
    if a.nd == 0 {
        a.dp = 0;
    }
}

impl Decimal {
    pub(super) fn new() -> Box<Decimal> {
        Box::new(Decimal { d: [0; 800], nd: 0, dp: 0, trunc: false })
    }

    /// Assign v to a.
    pub(super) fn assign(&mut self, mut v: u64) {
        let mut buf = [0u8; 24];
        // Write reversed decimal in buf.
        let mut n = 0;
        while v > 0 {
            let v1 = v / 10;
            v -= 10 * v1;
            buf[n] = v as u8 + b'0';
            n += 1;
            v = v1;
        }
        // Reverse again to produce forward decimal in a.d.
        self.nd = 0;
        while n > 0 {
            n -= 1;
            self.d[self.nd] = buf[n];
            self.nd += 1;
        }
        self.dp = self.nd as i32;
        trim(self);
    }

    /// Binary shift left (k > 0) or right (k < 0).
    pub(super) fn shift(&mut self, mut k: i32) {
        if self.nd == 0 {
            // nothing to do: a == 0
        } else if k > 0 {
            while k > MAX_SHIFT {
                left_shift(self, MAX_SHIFT as u32);
                k -= MAX_SHIFT;
            }
            left_shift(self, k as u32);
        } else if k < 0 {
            while k < -MAX_SHIFT {
                right_shift(self, MAX_SHIFT as u32);
                k += MAX_SHIFT;
            }
            right_shift(self, (-k) as u32);
        }
    }

    /// Round a to nd digits (or fewer). If nd is zero, it means we're rounding just to the left of the
    /// digits, as in 0.09 -> 0.1.
    pub(super) fn round(&mut self, nd: i32) {
        if nd < 0 || nd >= self.nd as i32 {
            return;
        }
        if should_round_up(self, nd as usize) {
            self.round_up(nd as usize);
        } else {
            self.round_down(nd as usize);
        }
    }

    fn round_down(&mut self, nd: usize) {
        if nd >= self.nd {
            return;
        }
        self.nd = nd;
        trim(self);
    }

    fn round_up(&mut self, nd: usize) {
        if nd >= self.nd {
            return;
        }
        // round up
        for i in (0..nd).rev() {
            let c = self.d[i];
            if c < b'9' {
                // can stop after this digit
                self.d[i] += 1;
                self.nd = i + 1;
                return;
            }
        }
        // Number is all 9s. Change to single 1 with adjusted decimal point.
        self.d[0] = b'1';
        self.nd = 1;
        self.dp += 1;
    }
}

// Maximum shift that we can do in one pass without overflow. A uint has 64 bits; we need to be able to
// hold 9<<k.
const MAX_SHIFT: i32 = 64 - 4;

/// Binary shift right (/ 2) by k bits. k <= maxShift to avoid overflow.
fn right_shift(a: &mut Decimal, k: u32) {
    let mut r = 0; // read pointer
    let mut w = 0; // write pointer

    // Pick up enough leading digits to cover first shift.
    let mut n: u64 = 0;
    while n >> k == 0 {
        if r >= a.nd {
            if n == 0 {
                // a == 0; shouldn't get here, but handle anyway.
                a.nd = 0;
                return;
            }
            while n >> k == 0 {
                n *= 10;
                r += 1;
            }
            break;
        }
        let c = u64::from(a.d[r]);
        n = n * 10 + c - u64::from(b'0');
        r += 1;
    }
    a.dp -= r as i32 - 1;

    let mask: u64 = (1 << k) - 1;

    // Pick up a digit, put down a digit.
    while r < a.nd {
        let c = u64::from(a.d[r]);
        let dig = n >> k;
        n &= mask;
        a.d[w] = dig as u8 + b'0';
        w += 1;
        n = n * 10 + c - u64::from(b'0');
        r += 1;
    }

    // Put down extra digits.
    while n > 0 {
        let dig = n >> k;
        n &= mask;
        if w < a.d.len() {
            a.d[w] = dig as u8 + b'0';
            w += 1;
        } else if dig > 0 {
            a.trunc = true;
        }
        n *= 10;
    }

    a.nd = w;
    trim(a);
}

// Cheat sheet for left shift: table indexed by shift count giving number of new digits that will be
// introduced by that shift.
static LEFTCHEATS: [(i32, &str); 61] = [
    (0, ""),
    (1, "5"),
    (1, "25"),
    (1, "125"),
    (2, "625"),
    (2, "3125"),
    (2, "15625"),
    (3, "78125"),
    (3, "390625"),
    (3, "1953125"),
    (4, "9765625"),
    (4, "48828125"),
    (4, "244140625"),
    (4, "1220703125"),
    (5, "6103515625"),
    (5, "30517578125"),
    (5, "152587890625"),
    (6, "762939453125"),
    (6, "3814697265625"),
    (6, "19073486328125"),
    (7, "95367431640625"),
    (7, "476837158203125"),
    (7, "2384185791015625"),
    (7, "11920928955078125"),
    (8, "59604644775390625"),
    (8, "298023223876953125"),
    (8, "1490116119384765625"),
    (9, "7450580596923828125"),
    (9, "37252902984619140625"),
    (9, "186264514923095703125"),
    (10, "931322574615478515625"),
    (10, "4656612873077392578125"),
    (10, "23283064365386962890625"),
    (10, "116415321826934814453125"),
    (11, "582076609134674072265625"),
    (11, "2910383045673370361328125"),
    (11, "14551915228366851806640625"),
    (12, "72759576141834259033203125"),
    (12, "363797880709171295166015625"),
    (12, "1818989403545856475830078125"),
    (13, "9094947017729282379150390625"),
    (13, "45474735088646411895751953125"),
    (13, "227373675443232059478759765625"),
    (13, "1136868377216160297393798828125"),
    (14, "5684341886080801486968994140625"),
    (14, "28421709430404007434844970703125"),
    (14, "142108547152020037174224853515625"),
    (15, "710542735760100185871124267578125"),
    (15, "3552713678800500929355621337890625"),
    (15, "17763568394002504646778106689453125"),
    (16, "88817841970012523233890533447265625"),
    (16, "444089209850062616169452667236328125"),
    (16, "2220446049250313080847263336181640625"),
    (16, "11102230246251565404236316680908203125"),
    (17, "55511151231257827021181583404541015625"),
    (17, "277555756156289135105907917022705078125"),
    (17, "1387778780781445675529539585113525390625"),
    (18, "6938893903907228377647697925567626953125"),
    (18, "34694469519536141888238489627838134765625"),
    (18, "173472347597680709441192448139190673828125"),
    (19, "867361737988403547205962240695953369140625"),
];

/// Is the leading prefix of b lexicographically less than s?
fn prefix_is_less_than(b: &[u8], s: &str) -> bool {
    for (i, &c) in s.as_bytes().iter().enumerate() {
        if i >= b.len() {
            return true;
        }
        if b[i] != c {
            return b[i] < c;
        }
    }
    false
}

/// Binary shift left (* 2) by k bits. k <= maxShift to avoid overflow.
fn left_shift(a: &mut Decimal, k: u32) {
    let mut delta = LEFTCHEATS[k as usize].0;
    if prefix_is_less_than(&a.d[0..a.nd], LEFTCHEATS[k as usize].1) {
        delta -= 1;
    }

    let mut r = a.nd as isize; // read index
    let mut w = a.nd as isize + delta as isize; // write index

    // Pick up a digit, put down a digit.
    let mut n: u64 = 0;
    r -= 1;
    while r >= 0 {
        n += (u64::from(a.d[r as usize]) - u64::from(b'0')) << k;
        let quo = n / 10;
        let rem = n - 10 * quo;
        w -= 1;
        if (w as usize) < a.d.len() {
            a.d[w as usize] = rem as u8 + b'0';
        } else if rem != 0 {
            a.trunc = true;
        }
        n = quo;
        r -= 1;
    }

    // Put down extra digits.
    while n > 0 {
        let quo = n / 10;
        let rem = n - 10 * quo;
        w -= 1;
        if (w as usize) < a.d.len() {
            a.d[w as usize] = rem as u8 + b'0';
        } else if rem != 0 {
            a.trunc = true;
        }
        n = quo;
    }

    a.nd = (a.nd as i32 + delta) as usize;
    if a.nd >= a.d.len() {
        a.nd = a.d.len();
    }
    a.dp += delta;
    trim(a);
}

/// If we chop a at nd digits, should we round up?
fn should_round_up(a: &Decimal, nd: usize) -> bool {
    if nd >= a.nd {
        return false;
    }
    if a.d[nd] == b'5' && nd + 1 == a.nd {
        // exactly halfway - round to even
        // if we truncated, a little higher than what's recorded - always round up
        if a.trunc {
            return true;
        }
        return nd > 0 && !(a.d[nd - 1] - b'0').is_multiple_of(2);
    }
    // not halfway - digit tells all
    a.d[nd] >= b'5'
}
