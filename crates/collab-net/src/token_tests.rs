//! Session secrets from the system's random numbers.

use crate::token::secret_256_hex;

#[test]
fn a_256_bit_secret_is_64_hex_digits_and_never_repeats() {
    let (a, b) = (secret_256_hex().unwrap(), secret_256_hex().unwrap());
    assert_eq!(a.len(), 64);
    assert!(a.chars().all(|c| c.is_ascii_hexdigit()), "{a}");
    assert_ne!(a, b);
}
