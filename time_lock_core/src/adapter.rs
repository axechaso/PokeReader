//! Transaction contract for future, individually verified game adapters.
//! No retail game profile or guessed address is included in this development build.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    pub title_id: u64,
    pub remaster_version: u16,
    pub fingerprint: [u8; 32],
}

pub struct Patch<'a> {
    pub address: u32,
    pub expected: &'a [u8],
    pub replacement: &'a [u8],
}
pub struct Profile<'a> {
    pub identity: Identity,
    pub hardware_report: &'a str,
    pub first_read_verified: bool,
    pub code_start: u32,
    pub code_end: u32,
    pub patches: &'a [Patch<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterError {
    Unverified,
    Identity,
    Layout,
    OriginalBytes,
    Read,
    Write,
    Cache,
    RollbackFailed,
}

pub trait PatchMemory {
    fn read(&self, address: u32, out: &mut [u8]) -> Result<(), AdapterError>;
    fn write(&mut self, address: u32, bytes: &[u8]) -> Result<(), AdapterError>;
    /// Must flush data and invalidate instructions before allowing game execution.
    fn sync_code(&mut self) -> Result<(), AdapterError>;
}

fn validate(
    profile: &Profile<'_>,
    actual: Identity,
    memory: &impl PatchMemory,
) -> Result<(), AdapterError> {
    if !profile.first_read_verified
        || profile.hardware_report.is_empty()
        || profile.patches.is_empty()
    {
        return Err(AdapterError::Unverified);
    }
    if actual != profile.identity {
        return Err(AdapterError::Identity);
    }
    // Exclude loader-modified process entry bytes, and require bounded ARM patches.
    if profile.code_start < 0x100010 || profile.code_start >= profile.code_end {
        return Err(AdapterError::Layout);
    }
    for (i, patch) in profile.patches.iter().enumerate() {
        let len = patch.expected.len();
        let end = patch
            .address
            .checked_add(len as u32)
            .ok_or(AdapterError::Layout)?;
        if len == 0
            || len > 32
            || len % 4 != 0
            || patch.address % 4 != 0
            || patch.replacement.len() != len
            || patch.address < profile.code_start
            || end > profile.code_end
        {
            return Err(AdapterError::Layout);
        }
        for other in &profile.patches[..i] {
            let other_end = other.address + other.expected.len() as u32;
            if patch.address < other_end && other.address < end {
                return Err(AdapterError::Layout);
            }
        }
        let mut original = [0u8; 32];
        memory.read(patch.address, &mut original[..len])?;
        if &original[..len] != patch.expected {
            return Err(AdapterError::OriginalBytes);
        }
    }
    Ok(())
}

fn rollback(
    profile: &Profile<'_>,
    memory: &mut impl PatchMemory,
    count: usize,
) -> Result<(), AdapterError> {
    let mut failed = false;
    for patch in profile.patches[..count].iter().rev() {
        if memory.write(patch.address, patch.expected).is_err() {
            failed = true;
        }
    }
    if memory.sync_code().is_err() {
        failed = true;
    }
    if failed {
        Err(AdapterError::RollbackFailed)
    } else {
        Ok(())
    }
}

/// Caller must hold game execution at its startup barrier throughout this operation.
/// RollbackFailed is fatal: the caller must not resume a partially modified game.
pub fn install(
    profile: &Profile<'_>,
    actual: Identity,
    memory: &mut impl PatchMemory,
) -> Result<(), AdapterError> {
    validate(profile, actual, memory)?;
    for (index, patch) in profile.patches.iter().enumerate() {
        if memory.write(patch.address, patch.replacement).is_err() {
            rollback(profile, memory, index + 1)?; // Include a possibly partial failed write.
            return Err(AdapterError::Write);
        }
    }
    if memory.sync_code().is_err() {
        rollback(profile, memory, profile.patches.len())?;
        return Err(AdapterError::Cache);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Memory {
        data: [u8; 16],
        writes: usize,
        fail_write: usize,
        fail_sync: bool,
        rollback_fails: bool,
    }
    impl Memory {
        fn new() -> Self {
            Self {
                data: [1; 16],
                writes: 0,
                fail_write: 0,
                fail_sync: false,
                rollback_fails: false,
            }
        }
    }
    impl PatchMemory for Memory {
        fn read(&self, address: u32, out: &mut [u8]) -> Result<(), AdapterError> {
            let i = (address - 0x100010) as usize;
            out.copy_from_slice(&self.data[i..i + out.len()]);
            Ok(())
        }
        fn write(&mut self, address: u32, bytes: &[u8]) -> Result<(), AdapterError> {
            self.writes += 1;
            let i = (address - 0x100010) as usize;
            self.data[i..i + bytes.len()].copy_from_slice(bytes);
            if self.writes == self.fail_write
                || (self.rollback_fails && self.writes > self.fail_write)
            {
                Err(AdapterError::Write)
            } else {
                Ok(())
            }
        }
        fn sync_code(&mut self) -> Result<(), AdapterError> {
            if self.fail_sync {
                self.fail_sync = false;
                Err(AdapterError::Cache)
            } else {
                Ok(())
            }
        }
    }
    const ID: Identity = Identity {
        title_id: 0x00040000001B5000,
        remaster_version: 2,
        fingerprint: [9; 32],
    };
    fn profile<'a>(patches: &'a [Patch<'a>]) -> Profile<'a> {
        Profile {
            identity: ID,
            hardware_report: "synthetic test only",
            first_read_verified: true,
            code_start: 0x100010,
            code_end: 0x100020,
            patches,
        }
    }
    #[test]
    fn validates_every_site_before_first_write() {
        let patches = [
            Patch {
                address: 0x100010,
                expected: &[1; 4],
                replacement: &[2; 4],
            },
            Patch {
                address: 0x100014,
                expected: &[3; 4],
                replacement: &[2; 4],
            },
        ];
        let mut memory = Memory::new();
        assert_eq!(
            install(&profile(&patches), ID, &mut memory),
            Err(AdapterError::OriginalBytes)
        );
        assert_eq!(memory.writes, 0);
        let foreign = Identity {
            remaster_version: 0,
            ..ID
        };
        assert_eq!(
            install(&profile(&patches), foreign, &mut memory),
            Err(AdapterError::Identity)
        );
        assert_eq!(memory.writes, 0);
    }
    #[test]
    fn partial_write_and_cache_failure_restore_originals() {
        let patches = [
            Patch {
                address: 0x100010,
                expected: &[1; 4],
                replacement: &[2; 4],
            },
            Patch {
                address: 0x100014,
                expected: &[1; 4],
                replacement: &[3; 4],
            },
        ];
        let mut memory = Memory::new();
        memory.fail_write = 2;
        assert_eq!(
            install(&profile(&patches), ID, &mut memory),
            Err(AdapterError::Write)
        );
        assert_eq!(memory.data, [1; 16]);
        let mut memory = Memory::new();
        memory.fail_sync = true;
        assert_eq!(
            install(&profile(&patches), ID, &mut memory),
            Err(AdapterError::Cache)
        );
        assert_eq!(memory.data, [1; 16]);
        assert_eq!(install(&profile(&patches), ID, &mut memory), Ok(()));
        assert_eq!(&memory.data[..8], &[2, 2, 2, 2, 3, 3, 3, 3]);
    }
    #[test]
    fn unverified_overlapping_and_rollback_failure_are_explicit() {
        let patches = [
            Patch {
                address: 0x100010,
                expected: &[1; 8],
                replacement: &[2; 8],
            },
            Patch {
                address: 0x100014,
                expected: &[1; 4],
                replacement: &[2; 4],
            },
        ];
        let mut memory = Memory::new();
        assert_eq!(
            install(&profile(&patches), ID, &mut memory),
            Err(AdapterError::Layout)
        );
        let mut p = profile(&patches[..1]);
        p.first_read_verified = false;
        assert_eq!(install(&p, ID, &mut memory), Err(AdapterError::Unverified));
        assert_eq!(memory.writes, 0);
        p.first_read_verified = true;
        memory.fail_write = 1;
        memory.rollback_fails = true;
        assert_eq!(
            install(&p, ID, &mut memory),
            Err(AdapterError::RollbackFailed)
        );
    }
}
