use std::{
    array,
    ops::{Add, AddAssign, Mul, MulAssign},
};

// === Standard Linear Algebra === //

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

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct Vector(u64);

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

// === Generation Script === //

fn shr_mat(s: usize) -> Matrix {
    Matrix(array::from_fn(|i| {
        if let Some(i) = i.checked_sub(s) {
            Vector::one_hot(i)
        } else {
            Vector::ZERO
        }
    }))
}

fn shl_mat(s: usize) -> Matrix {
    Matrix(array::from_fn(|i| {
        if let Some(i) = i.checked_add(s).filter(|&i| i < 64) {
            Vector::one_hot(i)
        } else {
            Vector::ZERO
        }
    }))
}

fn xorshift64_regular_one(state: u64) -> u64 {
    // Adapted from: https://en.wikipedia.org/w/index.php?title=Xorshift&oldid=1123949358
    let state = state ^ (state << 13);
    let state = state ^ (state >> 7);
    let state = state ^ (state << 17);
    state
}

fn xorshift64_regular(mut state: u64, times: u64) -> u64 {
    for _ in 0..times {
        state = xorshift64_regular_one(state);
    }
    state
}

fn xorshift_mat() -> Matrix {
    let state = Matrix::identity();
    let state = state + (shl_mat(13) * state);
    let state = state + (shr_mat(7) * state);
    let state = state + (shl_mat(17) * state);
    state
}

fn main() {
    assert_eq!(
        xorshift64_regular(1, 100),
        (xorshift_mat().exp(100) * Vector(1)).0
    );
}
