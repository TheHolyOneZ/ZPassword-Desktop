use std::path::Path;
use std::sync::OnceLock;

use proptest::prelude::*;
use zpass_core::crypto::KdfParams;
use zpass_core::format::Container;
use zpass_core::vault::{Credential, LockedVault, OpenVault};

const PW: &[u8] = b"robustness password";

fn valid_file() -> &'static Vec<u8> {
    static FILE: OnceLock<Vec<u8>> = OnceLock::new();
    FILE.get_or_init(|| {
        let mut v = OpenVault::create(
            Path::new("r.zpv"),
            "R",
            PW,
            Default::default(),
            KdfParams::insecure_test(),
        )
        .unwrap();
        v.regenerate_recovery_codes().unwrap();
        v.to_bytes().unwrap()
    })
}

fn unlocks(bytes: &[u8]) -> bool {
    LockedVault::from_bytes(Path::new("r.zpv"), bytes)
        .and_then(|l| {
            l.unlock(&Credential::Password {
                password: PW,
                keyfile_digest: None,
                yubikey_response: None,
            })
        })
        .is_ok()
}

#[test]
fn baseline_unlocks() {
    assert!(unlocks(valid_file()));
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 400, ..ProptestConfig::default() })]

    #[test]
    fn random_bytes_never_panic(data in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let _ = Container::parse(&data);
    }

    #[test]
    fn random_bytes_with_valid_magic_never_panic(tail in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let mut data = zpass_core::format::MAGIC.to_vec();
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&tail);
        let _ = Container::parse(&data);
    }

    #[test]
    fn any_mutation_is_rejected(pos in any::<prop::sample::Index>(), flip in 1u8..=255) {
        let mut bytes = valid_file().clone();
        let i = pos.index(bytes.len());
        bytes[i] ^= flip;
        prop_assert!(!unlocks(&bytes), "mutation at byte {} was accepted", i);
    }

    #[test]
    fn any_truncation_is_rejected(len in 0usize..4096) {
        let bytes = valid_file();
        let len = len.min(bytes.len() - 1);
        prop_assert!(!unlocks(&bytes[..len]));
    }
}
