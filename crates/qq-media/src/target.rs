/// Recipient scene shared by QQ rich-media metadata negotiation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaTarget<'a> {
    /// Direct friend UID resolved from QQ's authenticated directory.
    Direct(&'a str),
    /// Numeric QQ group identifier.
    Group(u32),
}
