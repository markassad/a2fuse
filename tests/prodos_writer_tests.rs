use a2fuse::A2FuseError;
use a2fuse::prodos::{AccessFlags, CreateOptions, Image, MetadataMode, PutOptions, StorageType};

#[test]
fn creates_an_empty_standard_volume() {
    let image = Image::create(&CreateOptions {
        name: "TestDisk".to_owned(),
        blocks: 280,
    })
    .unwrap();
    let volume = image.volume().unwrap();

    assert_eq!(image.bytes().len(), 280 * 512);
    assert_eq!(volume.header.name, "TestDisk");
    assert_eq!(volume.header.file_count, 0);
    assert_eq!(volume.header.bitmap_pointer, 6);
    assert!(volume.root.is_empty());
}

#[test]
fn imports_seedling_sapling_and_tree_files() {
    let mut image = Image::create(&CreateOptions {
        name: "FILES".to_owned(),
        blocks: 800,
    })
    .unwrap();
    let fixtures = [
        ("Small", vec![0x11; 17], StorageType::Seedling),
        ("Medium", vec![0x22; 513], StorageType::Sapling),
        ("Large", vec![0x33; 256 * 512 + 1], StorageType::Tree),
    ];

    for (name, data, _) in &fixtures {
        let mut options = PutOptions::new(*name);
        options.file_type = 0x06;
        options.aux_type = 0x2000;
        options.access = AccessFlags(0xe3);
        image.put_file(data, &options).unwrap();
    }

    let volume = image.volume().unwrap();
    assert_eq!(volume.header.file_count, 3);
    for (name, expected, storage_type) in fixtures {
        let node = volume.find(name, MetadataMode::Xattr).unwrap();
        assert_eq!(node.entry.storage_type, storage_type);
        assert_eq!(node.entry.file_type, 0x06);
        assert_eq!(node.entry.aux_type, 0x2000);
        assert_eq!(volume.read_entry(&node.entry).unwrap(), expected);
    }
}

#[test]
fn rejects_duplicate_and_invalid_names() {
    let mut image = Image::create(&CreateOptions {
        name: "TEST".to_owned(),
        blocks: 280,
    })
    .unwrap();
    image
        .put_file(b"first", &PutOptions::new("ReadMe"))
        .unwrap();

    assert!(matches!(
        image
            .put_file(b"second", &PutOptions::new("README"))
            .unwrap_err(),
        A2FuseError::FileExists(_)
    ));
    assert!(matches!(
        image
            .put_file(b"bad", &PutOptions::new("NOT VALID"))
            .unwrap_err(),
        A2FuseError::InvalidName { .. }
    ));
}

#[test]
fn reports_a_full_volume_without_partially_adding_the_file() {
    let mut image = Image::create(&CreateOptions {
        name: "TINY".to_owned(),
        blocks: 8,
    })
    .unwrap();

    assert!(matches!(
        image
            .put_file(&vec![0xaa; 1024], &PutOptions::new("TOOBIG"))
            .unwrap_err(),
        A2FuseError::DiskFull
    ));
    assert_eq!(image.volume().unwrap().header.file_count, 0);
}

#[test]
fn rejects_an_out_of_range_bitmap_before_mutation() {
    let image = Image::create(&CreateOptions {
        name: "BROKEN".to_owned(),
        blocks: 280,
    })
    .unwrap();
    let mut bytes = image.bytes().to_vec();
    let bitmap_pointer_offset = 2 * 512 + 4 + 0x23;
    bytes[bitmap_pointer_offset..bitmap_pointer_offset + 2].copy_from_slice(&280_u16.to_le_bytes());

    let path = std::env::temp_dir().join(format!("a2fuse-bad-bitmap-{}.po", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    let error = Image::open(&path).unwrap_err();
    std::fs::remove_file(path).unwrap();

    assert!(matches!(error, A2FuseError::InvalidVolumeSize(_)));
}
