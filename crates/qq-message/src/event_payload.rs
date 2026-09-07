pub(super) fn split(input: &[u8]) -> Option<(u32, &[u8])> {
    let group = u32::from_be_bytes(input.get(..4)?.try_into().ok()?);
    let length = usize::from(u16::from_be_bytes(input.get(5..7)?.try_into().ok()?));
    let end = 7_usize.checked_add(length)?;
    (end == input.len()).then(|| (group, &input[7..end]))
}
