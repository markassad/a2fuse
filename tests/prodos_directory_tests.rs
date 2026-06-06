use a2fuse::A2FuseError;
use a2fuse::prodos::directory::{PRODOS_ENTRIES_PER_BLOCK, PRODOS_ENTRY_LENGTH};
use a2fuse::prodos::file::{data_block_numbers, read_file};
use a2fuse::prodos::{
    AccessFlags, BLOCK_SIZE, BlockDevice, Directory, DirectoryEntry, MetadataMode, StorageType,
    Volume,
};

#[test]
fn parses_directory_entry_metadata() {
    let mut bytes = [0_u8; PRODOS_ENTRY_LENGTH];
    bytes[0] = 0x17;
    bytes[1..8].copy_from_slice(b"README!");
    bytes[0x10] = 0x06;
    put_u16(&mut bytes, 0x11, 8);
    put_u16(&mut bytes, 0x13, 1);
    put_u24(&mut bytes, 0x15, 123);
    bytes[0x1e] = 0xe3;
    put_u16(&mut bytes, 0x1f, 0x2000);
    put_u16(&mut bytes, 0x25, 2);

    let entry = DirectoryEntry::parse(&bytes).unwrap().unwrap();

    assert_eq!(entry.name, "README!");
    assert_eq!(entry.storage_type, StorageType::Seedling);
    assert_eq!(entry.file_type, 0x06);
    assert_eq!(entry.key_pointer, 8);
    assert_eq!(entry.blocks_used, 1);
    assert_eq!(entry.eof, 123);
    assert_eq!(entry.access, AccessFlags(0xe3));
    assert_eq!(entry.aux_type, 0x2000);
    assert_eq!(entry.header_pointer, 2);
}

#[test]
fn reads_entries_across_directory_block_chain() {
    let mut image = vec![0_u8; BLOCK_SIZE * 6];
    directory_links(&mut image, 2, 0, 5);
    directory_links(&mut image, 5, 2, 0);
    put_entry(
        &mut image,
        2,
        1,
        &entry_bytes("FIRST", StorageType::Seedling, 3, 1),
    );
    put_entry(
        &mut image,
        5,
        0,
        &entry_bytes("SECOND", StorageType::Seedling, 4, 1),
    );

    let directory = Directory::read(&BlockDevice::from_bytes(image).unwrap(), 2).unwrap();

    assert_eq!(
        directory
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["FIRST", "SECOND"]
    );
}

#[test]
fn reads_seedling_file_to_eof() {
    let mut image = vec![0_u8; BLOCK_SIZE * 4];
    image[3 * BLOCK_SIZE..4 * BLOCK_SIZE].fill(b'A');
    let device = BlockDevice::from_bytes(image).unwrap();
    let entry = entry("ONE", StorageType::Seedling, 3, 17);

    assert_eq!(data_block_numbers(&device, &entry).unwrap(), [Some(3)]);
    assert_eq!(read_file(&device, &entry).unwrap(), vec![b'A'; 17]);
}

#[test]
fn rejects_impossible_seedling_length() {
    let device = BlockDevice::from_bytes(vec![0_u8; BLOCK_SIZE * 4]).unwrap();
    let entry = entry("TOO.BIG", StorageType::Seedling, 3, (BLOCK_SIZE + 1) as u32);

    let error = data_block_numbers(&device, &entry).unwrap_err();
    assert!(error.to_string().contains("too large for one block"));
}

#[test]
fn resolves_sapling_blocks_and_sparse_holes() {
    let mut image = vec![0_u8; BLOCK_SIZE * 8];
    set_index_pointer(&mut image, 4, 0, 6);
    set_index_pointer(&mut image, 4, 2, 7);
    image[6 * BLOCK_SIZE..7 * BLOCK_SIZE].fill(b'A');
    image[7 * BLOCK_SIZE..8 * BLOCK_SIZE].fill(b'C');
    let device = BlockDevice::from_bytes(image).unwrap();
    let entry = entry("THREE", StorageType::Sapling, 4, (BLOCK_SIZE * 3) as u32);

    assert_eq!(
        data_block_numbers(&device, &entry).unwrap(),
        [Some(6), None, Some(7)]
    );
    let data = read_file(&device, &entry).unwrap();
    assert_eq!(&data[..BLOCK_SIZE], vec![b'A'; BLOCK_SIZE]);
    assert_eq!(&data[BLOCK_SIZE..BLOCK_SIZE * 2], vec![0; BLOCK_SIZE]);
    assert_eq!(&data[BLOCK_SIZE * 2..], vec![b'C'; BLOCK_SIZE]);
}

#[test]
fn resolves_tree_file_through_master_and_sapling_indexes() {
    let mut image = vec![0_u8; BLOCK_SIZE * 10];
    set_index_pointer(&mut image, 4, 0, 5);
    set_index_pointer(&mut image, 5, 0, 8);
    set_index_pointer(&mut image, 5, 1, 9);
    image[8 * BLOCK_SIZE..9 * BLOCK_SIZE].fill(b'A');
    image[9 * BLOCK_SIZE..10 * BLOCK_SIZE].fill(b'B');
    let device = BlockDevice::from_bytes(image).unwrap();
    let entry = entry("TREE", StorageType::Tree, 4, (BLOCK_SIZE * 2) as u32);

    assert_eq!(
        data_block_numbers(&device, &entry).unwrap(),
        [Some(8), Some(9)]
    );
}

#[test]
fn opens_volume_directory_and_reads_a_file() {
    let mut image = vec![0_u8; BLOCK_SIZE * 8];
    directory_links(&mut image, 2, 0, 0);

    let mut header = [0_u8; PRODOS_ENTRY_LENGTH];
    header[0] = (StorageType::VolumeHeader as u8) << 4 | 6;
    header[1..7].copy_from_slice(b"MYDISK");
    header[0x1e] = 0xe3;
    header[0x1f] = PRODOS_ENTRY_LENGTH as u8;
    header[0x20] = PRODOS_ENTRIES_PER_BLOCK as u8;
    put_u16(&mut header, 0x21, 1);
    put_u16(&mut header, 0x23, 6);
    put_u16(&mut header, 0x25, 8);
    put_entry(&mut image, 2, 0, &header);
    put_entry(
        &mut image,
        2,
        1,
        &entry_bytes("HELLO", StorageType::Seedling, 7, 5),
    );
    image[7 * BLOCK_SIZE..7 * BLOCK_SIZE + 5].copy_from_slice(b"hello");

    let volume = Volume::from_device(BlockDevice::from_bytes(image).unwrap()).unwrap();
    let node = volume.find("hello", MetadataMode::Xattr).unwrap();

    assert_eq!(volume.header.name, "MYDISK");
    assert_eq!(volume.header.file_count, 1);
    assert_eq!(volume.read_entry(&node.entry).unwrap(), b"hello");
}

#[test]
fn reads_extended_data_and_resource_forks() {
    let mut image = vec![0_u8; BLOCK_SIZE * 12];
    directory_links(&mut image, 2, 0, 0);

    let mut header = [0_u8; PRODOS_ENTRY_LENGTH];
    header[0] = (StorageType::VolumeHeader as u8) << 4 | 6;
    header[1..7].copy_from_slice(b"FORKS ");
    header[0x1e] = 0xe3;
    header[0x1f] = PRODOS_ENTRY_LENGTH as u8;
    header[0x20] = PRODOS_ENTRIES_PER_BLOCK as u8;
    put_u16(&mut header, 0x21, 1);
    put_u16(&mut header, 0x23, 6);
    put_u16(&mut header, 0x25, 12);
    put_entry(&mut image, 2, 0, &header);
    put_entry(
        &mut image,
        2,
        1,
        &entry_bytes("Read.Me", StorageType::Extended, 8, 0),
    );

    extended_fork(&mut image, 8, 0, 1, 9, 4);
    extended_fork(&mut image, 8, 256, 1, 10, 5);
    image[9 * BLOCK_SIZE..9 * BLOCK_SIZE + 4].copy_from_slice(b"DATA");
    image[10 * BLOCK_SIZE..10 * BLOCK_SIZE + 5].copy_from_slice(b"RSRC!");

    let volume = Volume::from_device(BlockDevice::from_bytes(image).unwrap()).unwrap();
    let node = volume.find("Read.Me", MetadataMode::Xattr).unwrap();

    assert_eq!(node.effective_storage_type(), StorageType::Seedling);
    assert_eq!(node.effective_eof(), 4);
    assert_eq!(node.effective_blocks_used(), 1);
    assert_eq!(volume.read_entry(&node.entry).unwrap(), b"DATA");
    assert_eq!(volume.read_resource_fork(&node.entry).unwrap(), b"RSRC!");
}

#[test]
fn allows_zero_length_files_without_a_key_block() {
    let mut image = vec![0_u8; BLOCK_SIZE * 8];
    directory_links(&mut image, 2, 0, 0);

    let mut header = [0_u8; PRODOS_ENTRY_LENGTH];
    header[0] = (StorageType::VolumeHeader as u8) << 4 | 6;
    header[1..7].copy_from_slice(b"EMPTY ");
    header[0x1e] = 0xe3;
    header[0x1f] = PRODOS_ENTRY_LENGTH as u8;
    header[0x20] = PRODOS_ENTRIES_PER_BLOCK as u8;
    put_u16(&mut header, 0x21, 1);
    put_u16(&mut header, 0x23, 6);
    put_u16(&mut header, 0x25, 8);
    put_entry(&mut image, 2, 0, &header);

    let file = entry_bytes("EMPTY", StorageType::Seedling, 0, 0);
    put_entry(&mut image, 2, 1, &file);

    let volume = Volume::from_device(BlockDevice::from_bytes(image).unwrap()).unwrap();
    let node = volume.find("EMPTY", MetadataMode::Xattr).unwrap();

    assert_eq!(volume.read_entry(&node.entry).unwrap(), Vec::<u8>::new());
}

#[test]
fn rejects_extended_files_without_a_key_block() {
    let mut image = vec![0_u8; BLOCK_SIZE * 8];
    directory_links(&mut image, 2, 0, 0);

    let mut header = [0_u8; PRODOS_ENTRY_LENGTH];
    header[0] = (StorageType::VolumeHeader as u8) << 4 | 6;
    header[1..7].copy_from_slice(b"BROKEN");
    header[0x1e] = 0xe3;
    header[0x1f] = PRODOS_ENTRY_LENGTH as u8;
    header[0x20] = PRODOS_ENTRIES_PER_BLOCK as u8;
    put_u16(&mut header, 0x21, 1);
    put_u16(&mut header, 0x23, 6);
    put_u16(&mut header, 0x25, 8);
    put_entry(&mut image, 2, 0, &header);
    put_entry(
        &mut image,
        2,
        1,
        &entry_bytes("Read.Me", StorageType::Extended, 0, 0),
    );

    let error = Volume::from_device(BlockDevice::from_bytes(image).unwrap()).unwrap_err();
    assert!(matches!(error, A2FuseError::InvalidDirectoryEntry(_)));
}

fn entry(name: &str, storage_type: StorageType, key_pointer: u16, eof: u32) -> DirectoryEntry {
    DirectoryEntry {
        name: name.to_owned(),
        storage_type,
        file_type: 0x06,
        key_pointer,
        blocks_used: 1,
        eof,
        creation: None,
        modification: None,
        access: AccessFlags(0xe3),
        aux_type: 0,
        header_pointer: 2,
    }
}

fn entry_bytes(
    name: &str,
    storage_type: StorageType,
    key_pointer: u16,
    eof: u32,
) -> [u8; PRODOS_ENTRY_LENGTH] {
    let mut bytes = [0_u8; PRODOS_ENTRY_LENGTH];
    bytes[0] = (storage_type as u8) << 4 | name.len() as u8;
    bytes[1..1 + name.len()].copy_from_slice(name.as_bytes());
    bytes[0x10] = 0x06;
    put_u16(&mut bytes, 0x11, key_pointer);
    put_u16(&mut bytes, 0x13, 1);
    put_u24(&mut bytes, 0x15, eof);
    bytes[0x1e] = 0xe3;
    bytes
}

fn directory_links(image: &mut [u8], block: usize, previous: u16, next: u16) {
    let start = block * BLOCK_SIZE;
    image[start..start + 2].copy_from_slice(&previous.to_le_bytes());
    image[start + 2..start + 4].copy_from_slice(&next.to_le_bytes());
}

fn put_entry(image: &mut [u8], block: usize, slot: usize, entry: &[u8; PRODOS_ENTRY_LENGTH]) {
    assert!(slot < PRODOS_ENTRIES_PER_BLOCK);
    let start = block * BLOCK_SIZE + 4 + slot * PRODOS_ENTRY_LENGTH;
    image[start..start + PRODOS_ENTRY_LENGTH].copy_from_slice(entry);
}

fn set_index_pointer(image: &mut [u8], block: usize, position: usize, pointer: u16) {
    let start = block * BLOCK_SIZE;
    image[start + position] = pointer as u8;
    image[start + 256 + position] = (pointer >> 8) as u8;
}

fn extended_fork(
    image: &mut [u8],
    block: usize,
    offset: usize,
    storage_type: u8,
    key_block: u16,
    eof: u32,
) {
    let start = block * BLOCK_SIZE + offset;
    image[start] = storage_type;
    put_u16(image, start + 1, key_block);
    put_u16(image, start + 3, 1);
    image[start + 5] = eof as u8;
    image[start + 6] = (eof >> 8) as u8;
    image[start + 7] = (eof >> 16) as u8;
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u24(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset] = value as u8;
    bytes[offset + 1] = (value >> 8) as u8;
    bytes[offset + 2] = (value >> 16) as u8;
}
