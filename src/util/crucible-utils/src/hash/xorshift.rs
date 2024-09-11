use std::sync::Mutex;

// === Core === //

// Adapted from: https://en.wikipedia.org/w/index.php?title=Xorshift&oldid=1123949358
pub const fn xorshift64(state: u64) -> u64 {
    let state = state ^ (state << 13);
    let state = state ^ (state >> 7);
    let state = state ^ (state << 17);
    state
}

pub fn xorshift_mut(state: &mut u64) -> u64 {
    let old = *state;
    *state = xorshift64(old);
    old
}

mod skip {
    use std::{
        array, fmt,
        ops::{Add, AddAssign, Mul, MulAssign},
        sync::OnceLock,
        u64,
    };

    // === Linear Algebra over `Z_2` === //

    #[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
    pub struct Scalar(bool);

    impl Add for Scalar {
        type Output = Self;

        fn add(self, rhs: Self) -> Self::Output {
            Self(self.0 ^ rhs.0)
        }
    }

    impl Mul for Scalar {
        type Output = Self;

        fn mul(self, rhs: Self) -> Self::Output {
            Self(self.0 & rhs.0)
        }
    }

    impl AddAssign for Scalar {
        fn add_assign(&mut self, rhs: Self) {
            *self = *self + rhs;
        }
    }

    impl MulAssign for Scalar {
        fn mul_assign(&mut self, rhs: Self) {
            *self = *self * rhs;
        }
    }

    #[derive(Copy, Clone, Hash, Eq, PartialEq)]
    pub struct Vector(u64);

    impl fmt::Debug for Vector {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{:0>64b}", self.0)
        }
    }

    impl Vector {
        pub const ZERO: Self = Self(0);

        pub const fn one_hot(i: usize) -> Self {
            Self(1u64 << i)
        }
    }

    impl Add for Vector {
        type Output = Self;

        fn add(self, rhs: Self) -> Self::Output {
            Self(self.0 ^ rhs.0)
        }
    }

    impl Mul<Vector> for Scalar {
        type Output = Vector;

        fn mul(self, rhs: Vector) -> Self::Output {
            if self.0 {
                rhs
            } else {
                Vector(0)
            }
        }
    }

    impl AddAssign for Vector {
        fn add_assign(&mut self, rhs: Self) {
            *self = *self + rhs;
        }
    }

    #[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
    pub struct Matrix([Vector; 64]);

    impl Matrix {
        pub fn identity() -> Self {
            Matrix(array::from_fn(|i| Vector::one_hot(i)))
        }

        pub fn exp(self, mut count: u64) -> Self {
            let mut accum = Matrix::identity();
            let mut place = self;

            while count > 0 {
                if count & 1 != 0 {
                    accum *= place;
                }

                place *= place;
                count >>= 1;
            }

            accum
        }
    }

    impl Add<Matrix> for Matrix {
        type Output = Matrix;

        fn add(self, rhs: Matrix) -> Self::Output {
            Matrix(array::from_fn(|i| self.0[i] + rhs.0[i]))
        }
    }

    impl Mul<Matrix> for Matrix {
        type Output = Matrix;

        fn mul(self, rhs: Matrix) -> Self::Output {
            // We can uniquely determine the behavior of a linear transformation by mapping a basis.
            Matrix(array::from_fn(|i| self * (rhs * Vector::one_hot(i))))
        }
    }

    impl AddAssign<Matrix> for Matrix {
        fn add_assign(&mut self, rhs: Matrix) {
            *self = *self + rhs;
        }
    }

    impl MulAssign<Matrix> for Matrix {
        fn mul_assign(&mut self, rhs: Matrix) {
            *self = *self * rhs;
        }
    }

    impl Mul<Vector> for Matrix {
        type Output = Vector;

        fn mul(self, rhs: Vector) -> Self::Output {
            let mut accum = Vector(0);
            let mut remaining = rhs.0;

            loop {
                let i = remaining.trailing_zeros();
                if i == 64 {
                    break;
                }
                remaining ^= 1 << i;

                accum += self.0[i as usize];
            }

            accum
        }
    }

    // === Main Generation Logic === //

    pub fn shr_mat(s: usize) -> Matrix {
        Matrix(array::from_fn(|i| {
            if let Some(i) = i.checked_sub(s) {
                Vector::one_hot(i)
            } else {
                Vector::ZERO
            }
        }))
    }

    pub fn shl_mat(s: usize) -> Matrix {
        Matrix(array::from_fn(|i| {
            if let Some(i) = i.checked_add(s).filter(|&i| i < 64) {
                Vector::one_hot(i)
            } else {
                Vector::ZERO
            }
        }))
    }

    pub fn xorshift_mat() -> Matrix {
        let state = Matrix::identity();
        let state = state + (shl_mat(13) * state);
        let state = state + (shr_mat(7) * state);
        let state = state + (shl_mat(17) * state);
        state
    }

    pub const XORSHIFT_SKIP_SIZE: u64 = 1 << 16;

    pub fn xorshift_skip(state: u64) -> u64 {
        static SKIP_MATRIX: OnceLock<Matrix> = OnceLock::new();
        let matrix = *SKIP_MATRIX.get_or_init(|| xorshift_mat().exp(XORSHIFT_SKIP_SIZE));
        (matrix * Vector(state)).0
    }
}

pub use skip::{xorshift_skip, XORSHIFT_SKIP_SIZE};

// === XorshiftPool === //

#[derive(Debug)]
pub struct XorshiftPool {
    state: Mutex<u64>,
    thread_states: Mutex<Vec<Xorshift>>,
}

impl Default for XorshiftPool {
    fn default() -> Self {
        Self::new()
    }
}

impl XorshiftPool {
    pub const fn new() -> Self {
        Self {
            state: Mutex::new(xorshift64(1)),
            thread_states: Mutex::new(Vec::new()),
        }
    }

    pub fn gen_sync(&mut self) -> u64 {
        xorshift_mut(self.state.get_mut().unwrap())
    }

    pub fn alloc_local(&self) -> Xorshift {
        // If we have a `LocalXorshift` generator in the pool, reuse it.
        let mut pool = self.thread_states.lock().unwrap();
        if let Some(existing) = pool.pop() {
            return existing;
        }

        // Otherwise, we have to create a new one.
        let mut state = self.state.lock().unwrap();
        let start = *state;
        *state = xorshift_skip(*state);
        Xorshift::new(start, *state)
    }

    pub fn dealloc_local(&self, shift: Xorshift) {
        self.thread_states.lock().unwrap().push(shift);
    }

    pub fn gen_local(&self, local: &mut Xorshift) -> u64 {
        // Attempt to generate a number within the pre-reserved range for this generator.
        let next = xorshift_mut(&mut local.state);
        if next != local.end_excl {
            return next;
        }

        // If that fails, allocate a new `LocalXorshift` generator and generate from it.
        *local = self.alloc_local();
        xorshift_mut(&mut local.state)
    }
}

#[derive(Debug, Clone)]
pub struct Xorshift {
    pub state: u64,
    pub end_excl: u64,
}

impl Xorshift {
    pub const fn new_empty() -> Self {
        Self {
            state: 0,
            end_excl: 0,
        }
    }

    pub const fn new(state: u64, end_excl: u64) -> Self {
        Self { state, end_excl }
    }

    pub const fn new_full(state: u64) -> Self {
        Self::new(xorshift64(state), state)
    }
}

impl Iterator for Xorshift {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        (self.state != self.end_excl).then(|| xorshift_mut(&mut self.state))
    }
}
