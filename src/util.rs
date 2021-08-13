/// Finds the smallest multiple of base that contains value.
pub const fn smallest_multiple_containing(value: u32, base: u32) -> u32 {
    (value + base - 1) / base * base
}

// Unit Tests.

#[cfg(test)]
mod tests {
    use crate::util::smallest_multiple_containing;

    #[test]
    fn smallest_multiple_containing_below() {
        assert_eq!(smallest_multiple_containing(63, 64), 64);
    }

    #[test]
    fn smallest_multiple_containing_equal() {
        assert_eq!(smallest_multiple_containing(64, 64), 64);
    }

    #[test]
    fn smallest_multiple_containing_above() {
        assert_eq!(smallest_multiple_containing(65, 64), 128);
    }
}
