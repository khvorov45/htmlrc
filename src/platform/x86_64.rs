pub(crate) fn last_cycle_count() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}
