use a2fuse::prodos::path::{decode_filename, decode_filename_with_case, host_filename};
use a2fuse::prodos::{AccessFlags, DirectoryEntry, MetadataMode, StorageType};

#[test]
fn strips_high_bits_and_replaces_host_separators() {
    let raw = [b'H' | 0x80, b'I' | 0x80, b'/', b':', 0x01];

    assert_eq!(decode_filename(&raw), "HI___");
}

#[test]
fn applies_prodos_lowercase_flags() {
    // The top flag marks the case bitmap as valid; subsequent bits map to characters.
    let case_bits = 0x8000 | (1 << 14) | (1 << 12) | (1 << 10);

    assert_eq!(decode_filename_with_case(b"README", case_bits), "rEaDmE");
}

#[test]
fn filename_metadata_mode_is_explicit() {
    let entry = entry("HELLO");

    assert_eq!(host_filename(&entry, MetadataMode::Xattr), "HELLO");
    assert_eq!(
        host_filename(&entry, MetadataMode::Filename),
        "HELLO,t$06,a$2000"
    );
}

fn entry(name: &str) -> DirectoryEntry {
    DirectoryEntry {
        name: name.to_owned(),
        storage_type: StorageType::Seedling,
        file_type: 0x06,
        key_pointer: 8,
        blocks_used: 1,
        eof: 12,
        creation: None,
        modification: None,
        access: AccessFlags(0xe3),
        aux_type: 0x2000,
        header_pointer: 2,
    }
}
