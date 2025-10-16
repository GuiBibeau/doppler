const ADMIN_HEADER: usize = 0x0008;
const ADMIN_KEY: usize = 0x0010;

/// Trusted admin signer (base58: crate::ADMIN_BASE58)
pub const ADMIN: [u8; 32] = crate::ADMIN_BYTES;

// Account flags
pub const NO_DUP_SIGNER: u16 = 0x01 << 8 | 0xff; // SIGNER | NO_DUP

pub struct Admin;

impl Admin {
    #[inline(always)]
    /// # Check
    /// Performs the following checks on the Admin account:
    /// - Checks Admin is a non-duplicate signer (2 CUs)
    /// - Checks Admin address matches ADMIN (12 CUs)
    ///
    /// # Safety
    /// - The caller must ensure that `ptr` is a valid pointer to a memory region
    ///   that can be safely read from.
    /// - The memory region must be properly aligned and large enough to hold the
    ///   data being read.
    pub unsafe fn check(ptr: *mut u8) {
        if crate::read::<u16>(ptr, ADMIN_HEADER) != NO_DUP_SIGNER
            || crate::read::<u64>(ptr, ADMIN_KEY) != *ADMIN.as_ptr().cast::<u64>()
            || crate::read::<u64>(ptr, ADMIN_KEY + 0x08) != *ADMIN.as_ptr().add(8).cast::<u64>()
            || crate::read::<u64>(ptr, ADMIN_KEY + 0x10) != *ADMIN.as_ptr().add(16).cast::<u64>()
            || crate::read::<u64>(ptr, ADMIN_KEY + 0x18) != *ADMIN.as_ptr().add(24).cast::<u64>()
        {
            #[cfg(target_os = "solana")]
            unsafe {
                core::arch::asm!("lddw r0, 1\nexit");
            }
        }
    }
}
