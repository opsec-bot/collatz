//! # collatz
//!
//! A dependency-free toolkit for exploring and verifying the Collatz (3n+1)
//! conjecture.
//!
//! The conjecture says that iterating
//!
//! ```text
//! C(n) = n / 2      if n is even
//!        3n + 1     if n is odd
//! ```
//!
//! from any positive integer eventually reaches 1. It is unproven; this crate
//! is about checking it quickly and correctly, and about looking at the data.

pub mod bigint;
pub mod cycles;
pub mod expr;
pub mod map;
pub mod orbit;
pub mod par;
pub mod records;
pub mod sieve;
pub mod verify;

pub use bigint::BigUint;
