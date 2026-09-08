/// Per-login `WtLogin` packet sequence retained across QR fetch, polling and exchange.
#[derive(Debug, Default)]
pub struct WtLoginSequence(u16);

impl WtLoginSequence {
    /// Returns the current sequence and advances with the frozen implementation's wrapping rule.
    #[must_use]
    pub const fn take(&mut self) -> u16 {
        let current = self.0;
        self.0 = self.0.wrapping_add(1);
        current
    }
}

#[cfg(test)]
mod tests {
    use super::WtLoginSequence;

    #[test]
    fn starts_at_zero_and_advances_per_packet() {
        let mut sequence = WtLoginSequence::default();
        assert_eq!(sequence.take(), 0);
        assert_eq!(sequence.take(), 1);
        assert_eq!(sequence.take(), 2);
    }
}
