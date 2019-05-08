/// Divide `n` by `divisor`, and round up to the nearest integer
/// if not evenly divisable.
#[inline]
pub(super) fn div_round_up(n: usize, divisor: usize) -> usize {
    debug_assert!(divisor != 0, "Division by zero!");
    if n == 0 {
        0
    } else {
        (n - 1) / divisor + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_div_round_up() {
        assert_eq!(0, div_round_up(0, 5));
        assert_eq!(1, div_round_up(5, 5));
        assert_eq!(1, div_round_up(1, 5));
        assert_eq!(2, div_round_up(3, 2));
        assert_eq!(
            usize::max_value() / 2 + 1,
            div_round_up(usize::max_value(), 2)
        );
    }
}
