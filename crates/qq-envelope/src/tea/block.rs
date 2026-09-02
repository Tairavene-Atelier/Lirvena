const DELTA: u32 = 0x9e37_79b9;
const ROUNDS: usize = 16;
const ROUNDS_U32: u32 = 16;

pub(crate) fn encrypt(value: u64, key: [u32; 4]) -> u64 {
    let (mut left, mut right) = split(value);
    let mut sum = 0_u32;
    for _round in 0..ROUNDS {
        sum = sum.wrapping_add(DELTA);
        left = left.wrapping_add(
            right.wrapping_add(sum)
                ^ (right.wrapping_shl(4)).wrapping_add(key[0])
                ^ (right.wrapping_shr(5)).wrapping_add(key[1]),
        );
        right = right.wrapping_add(
            left.wrapping_add(sum)
                ^ (left.wrapping_shl(4)).wrapping_add(key[2])
                ^ (left.wrapping_shr(5)).wrapping_add(key[3]),
        );
    }
    (u64::from(left) << 32) | u64::from(right)
}

pub(crate) fn decrypt(value: u64, key: [u32; 4]) -> u64 {
    let (mut left, mut right) = split(value);
    let mut sum = DELTA.wrapping_mul(ROUNDS_U32);
    for _round in 0..ROUNDS {
        right = right.wrapping_sub(
            left.wrapping_add(sum)
                ^ (left.wrapping_shl(4)).wrapping_add(key[2])
                ^ (left.wrapping_shr(5)).wrapping_add(key[3]),
        );
        left = left.wrapping_sub(
            right.wrapping_add(sum)
                ^ (right.wrapping_shl(4)).wrapping_add(key[0])
                ^ (right.wrapping_shr(5)).wrapping_add(key[1]),
        );
        sum = sum.wrapping_sub(DELTA);
    }
    (u64::from(left) << 32) | u64::from(right)
}

fn split(value: u64) -> (u32, u32) {
    let bytes = value.to_be_bytes();
    (
        u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
    )
}
