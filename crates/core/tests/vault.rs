use sylvie_core::error::Error;
use sylvie_core::vault;

#[test]
fn seal_open_roundtrip() {
    let key = vault::root(b"secret", vault::VAULT).unwrap();
    let sealed = vault::seal(&key, b"hello sylvie").unwrap();
    assert_ne!(sealed, b"hello sylvie");
    assert_eq!(vault::open(&key, &sealed).unwrap(), b"hello sylvie");
}

#[test]
fn tampered_sealed_rejected() {
    let key = vault::root(b"secret", vault::VAULT).unwrap();
    let mut sealed = vault::seal(&key, b"hello").unwrap();
    let last = sealed.len() - 1;
    sealed[last] ^= 1;
    assert!(matches!(vault::open(&key, &sealed), Err(Error::Crypto)));
}

#[test]
fn wrong_key_rejected() {
    let key = vault::root(b"secret", vault::VAULT).unwrap();
    let other = vault::root(b"different", vault::VAULT).unwrap();
    let sealed = vault::seal(&key, b"hello").unwrap();
    assert!(matches!(vault::open(&other, &sealed), Err(Error::Crypto)));
}

#[test]
fn root_is_stable_and_label_bound() {
    let first = vault::root(b"secret", vault::VAULT).unwrap();
    let second = vault::root(b"secret", vault::VAULT).unwrap();
    let channel = vault::root(b"secret", vault::CHANNEL).unwrap();
    assert_eq!(first, second);
    assert_ne!(first, channel);
}

#[test]
fn truncated_sealed_rejected() {
    let key = vault::root(b"secret", vault::VAULT).unwrap();
    assert!(matches!(vault::open(&key, &[0u8; 10]), Err(Error::Crypto)));
}
