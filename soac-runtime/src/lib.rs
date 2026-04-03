#![no_std]

#[inline(always)]
#[unsafe(no_mangle)]
pub extern "C" fn soac_runtime_add1_i64(value: i64) -> i64 {
    value.wrapping_add(1)
}
