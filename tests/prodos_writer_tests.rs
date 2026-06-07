use a2fuse::A2FuseError;
use a2fuse::prodos::{
    AccessFlags, BootFile, CreateOptions, Image, MetadataMode, MkdirOptions, PutOptions,
    RemoveOptions, StorageType,
};

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
    let original = image.bytes().to_vec();

    assert!(matches!(
        image
            .put_file(&vec![0xaa; 1024], &PutOptions::new("TOOBIG"))
            .unwrap_err(),
        A2FuseError::DiskFull
    ));
    assert_eq!(image.volume().unwrap().header.file_count, 0);
    assert_eq!(image.bytes(), original);
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

#[test]
fn creates_root_and_nested_directories() {
    let mut image = Image::create(&CreateOptions {
        name: "DIRS".to_owned(),
        blocks: 280,
    })
    .unwrap();

    image.create_directory(&MkdirOptions::new("Games")).unwrap();
    image
        .create_directory(&MkdirOptions::new("Games/Arcade"))
        .unwrap();

    let volume = image.volume().unwrap();
    let games = volume.find("Games", MetadataMode::Xattr).unwrap();
    let arcade = volume.find("Games/Arcade", MetadataMode::Xattr).unwrap();
    assert!(games.is_directory());
    assert!(arcade.is_directory());
    assert_eq!(games.entry.storage_type, StorageType::Subdirectory);
    assert_eq!(games.entry.file_type, 0x0f);
    assert_eq!(games.entry.blocks_used, 1);
    assert_eq!(games.entry.eof, 512);
    assert_eq!(games.entry.header_pointer, 2);
    assert_eq!(arcade.entry.header_pointer, games.entry.key_pointer);
    assert_eq!(volume.header.file_count, 1);

    let games_header = usize::from(games.entry.key_pointer) * 512 + 4;
    assert_eq!(image.bytes()[games_header] >> 4, 0x0e);
    assert_eq!(
        u16::from_le_bytes([
            image.bytes()[games_header + 0x21],
            image.bytes()[games_header + 0x22],
        ]),
        1
    );
    assert_eq!(
        u16::from_le_bytes([
            image.bytes()[games_header + 0x23],
            image.bytes()[games_header + 0x24],
        ]),
        2
    );
    assert_eq!(image.bytes()[games_header + 0x25], 2);
    assert_eq!(image.bytes()[games_header + 0x26], 0x27);

    let arcade_header = usize::from(arcade.entry.key_pointer) * 512 + 4;
    assert_eq!(
        u16::from_le_bytes([
            image.bytes()[arcade_header + 0x23],
            image.bytes()[arcade_header + 0x24],
        ]),
        games.entry.key_pointer
    );
    assert_eq!(image.bytes()[arcade_header + 0x25], 2);
}

#[test]
fn imports_and_reads_a_file_in_a_nested_directory() {
    let mut image = Image::create(&CreateOptions {
        name: "DIRS".to_owned(),
        blocks: 280,
    })
    .unwrap();
    let mut mkdir = MkdirOptions::new("Games/Arcade");
    mkdir.parents = true;
    image.create_directory(&mkdir).unwrap();

    image
        .put_file(
            b"nested file contents",
            &PutOptions::new("Games/Arcade/Hello"),
        )
        .unwrap();

    let volume = image.volume().unwrap();
    let arcade = volume.find("Games/Arcade", MetadataMode::Xattr).unwrap();
    let file = volume
        .find("Games/Arcade/Hello", MetadataMode::Xattr)
        .unwrap();
    assert_eq!(file.entry.header_pointer, arcade.entry.key_pointer);
    assert_eq!(
        volume.read_entry(&file.entry).unwrap(),
        b"nested file contents"
    );
    assert_eq!(volume.header.file_count, 1);

    let arcade_header = usize::from(arcade.entry.key_pointer) * 512 + 4;
    assert_eq!(
        u16::from_le_bytes([
            image.bytes()[arcade_header + 0x21],
            image.bytes()[arcade_header + 0x22],
        ]),
        1
    );
}

#[test]
fn put_requires_an_existing_parent_without_modifying_the_image() {
    let mut image = Image::create(&CreateOptions {
        name: "DIRS".to_owned(),
        blocks: 280,
    })
    .unwrap();
    let original = image.bytes().to_vec();

    assert!(matches!(
        image
            .put_file(b"contents", &PutOptions::new("Games/Hello"))
            .unwrap_err(),
        A2FuseError::PathNotFound(_)
    ));
    assert_eq!(image.bytes(), original);
}

#[test]
fn put_rejects_a_file_as_parent_without_modifying_the_image() {
    let mut image = Image::create(&CreateOptions {
        name: "DIRS".to_owned(),
        blocks: 280,
    })
    .unwrap();
    image
        .put_file(b"not a directory", &PutOptions::new("Games"))
        .unwrap();
    let original = image.bytes().to_vec();

    assert!(matches!(
        image
            .put_file(b"contents", &PutOptions::new("Games/Hello"))
            .unwrap_err(),
        A2FuseError::NotADirectory(_)
    ));
    assert_eq!(image.bytes(), original);
}

#[test]
fn mkdir_requires_existing_parents_without_parents_flag() {
    let mut image = Image::create(&CreateOptions {
        name: "DIRS".to_owned(),
        blocks: 280,
    })
    .unwrap();
    let original = image.bytes().to_vec();

    assert!(matches!(
        image
            .create_directory(&MkdirOptions::new("Games/Arcade"))
            .unwrap_err(),
        A2FuseError::PathNotFound(_)
    ));
    assert_eq!(image.bytes(), original);
}

#[test]
fn mkdir_parents_accepts_existing_directories() {
    let mut image = Image::create(&CreateOptions {
        name: "DIRS".to_owned(),
        blocks: 280,
    })
    .unwrap();
    let mut options = MkdirOptions::new("Games/Arcade");
    options.parents = true;

    image.create_directory(&options).unwrap();
    image.create_directory(&options).unwrap();

    assert!(
        image
            .volume()
            .unwrap()
            .find("Games/Arcade", MetadataMode::Xattr)
            .unwrap()
            .is_directory()
    );
}

#[test]
fn mkdir_rejects_a_file_as_parent_without_modifying_the_image() {
    let mut image = Image::create(&CreateOptions {
        name: "DIRS".to_owned(),
        blocks: 280,
    })
    .unwrap();
    image
        .put_file(b"not a directory", &PutOptions::new("Games"))
        .unwrap();
    let original = image.bytes().to_vec();
    let mut options = MkdirOptions::new("Games/Arcade");
    options.parents = true;

    assert!(matches!(
        image.create_directory(&options).unwrap_err(),
        A2FuseError::NotADirectory(_)
    ));
    assert_eq!(image.bytes(), original);
}

#[test]
fn mkdir_rolls_back_when_the_volume_is_full() {
    let mut image = Image::create(&CreateOptions {
        name: "TINY".to_owned(),
        blocks: 8,
    })
    .unwrap();
    image
        .put_file(b"uses the last block", &PutOptions::new("FULL"))
        .unwrap();
    let original = image.bytes().to_vec();

    assert!(matches!(
        image
            .create_directory(&MkdirOptions::new("Games"))
            .unwrap_err(),
        A2FuseError::DiskFull
    ));
    assert_eq!(image.bytes(), original);
}

#[test]
fn installs_boot_blocks_and_prodos_system_file() {
    let mut image = Image::create(&CreateOptions {
        name: "BOOT".to_owned(),
        blocks: 280,
    })
    .unwrap();
    let boot_blocks = vec![0xa5; 1024];
    let prodos_system = BootFile {
        data: vec![0x5a; 1200],
        file_type: 0xff,
        aux_type: 0x2000,
        access: AccessFlags(0xe3),
    };
    let basic_system = BootFile {
        data: vec![0x77; 900],
        file_type: 0xff,
        aux_type: 0x0801,
        access: AccessFlags(0xe3),
    };

    image
        .install_bootable_components(&boot_blocks, &prodos_system, &basic_system)
        .unwrap();

    assert_eq!(&image.bytes()[..1024], boot_blocks);
    let volume = image.volume().unwrap();
    let prodos = volume.find("PRODOS", MetadataMode::Xattr).unwrap();
    let basic = volume.find("BASIC.SYSTEM", MetadataMode::Xattr).unwrap();
    assert_eq!(prodos.entry.file_type, 0xff);
    assert_eq!(prodos.entry.aux_type, 0x2000);
    assert_eq!(
        volume.read_entry(&prodos.entry).unwrap(),
        prodos_system.data
    );
    assert_eq!(basic.entry.file_type, 0xff);
    assert_eq!(basic.entry.aux_type, 0x0801);
    assert_eq!(volume.read_entry(&basic.entry).unwrap(), basic_system.data);
}

#[test]
fn install_bootable_components_rejects_invalid_boot_block_length() {
    let mut image = Image::create(&CreateOptions {
        name: "BOOT".to_owned(),
        blocks: 280,
    })
    .unwrap();
    let original = image.bytes().to_vec();

    assert!(matches!(
        image
            .install_bootable_components(
                &vec![0; 1000],
                &BootFile {
                    data: b"prodos".to_vec(),
                    file_type: 0xff,
                    aux_type: 0x2000,
                    access: AccessFlags(0xe3),
                },
                &BootFile {
                    data: b"basic".to_vec(),
                    file_type: 0xff,
                    aux_type: 0x0801,
                    access: AccessFlags(0xe3),
                },
            )
            .unwrap_err(),
        A2FuseError::InvalidBootBlocks(_)
    ));
    assert_eq!(image.bytes(), original);
}

#[test]
fn install_bootable_components_rolls_back_when_prodos_exists() {
    let mut image = Image::create(&CreateOptions {
        name: "BOOT".to_owned(),
        blocks: 280,
    })
    .unwrap();
    image
        .put_file(b"existing", &PutOptions::new("PRODOS"))
        .unwrap();
    let original = image.bytes().to_vec();

    assert!(matches!(
        image
            .install_bootable_components(
                &vec![0xa5; 1024],
                &BootFile {
                    data: b"prodos".to_vec(),
                    file_type: 0xff,
                    aux_type: 0x2000,
                    access: AccessFlags(0xe3),
                },
                &BootFile {
                    data: b"basic".to_vec(),
                    file_type: 0xff,
                    aux_type: 0x0801,
                    access: AccessFlags(0xe3),
                },
            )
            .unwrap_err(),
        A2FuseError::FileExists(_)
    ));
    assert_eq!(image.bytes(), original);
}

#[test]
fn removes_regular_files_and_reuses_freed_blocks() {
    let mut image = Image::create(&CreateOptions {
        name: "FILES".to_owned(),
        blocks: 16,
    })
    .unwrap();
    image.put_file(b"abc", &PutOptions::new("A")).unwrap();
    let before_remove = image.bytes().to_vec();

    image.remove_file(&RemoveOptions::new("A")).unwrap();
    assert!(matches!(
        image.volume().unwrap().find("A", MetadataMode::Xattr),
        Err(A2FuseError::PathNotFound(_))
    ));

    image.put_file(b"def", &PutOptions::new("B")).unwrap();
    assert_eq!(
        image
            .volume()
            .unwrap()
            .read_entry(
                &image
                    .volume()
                    .unwrap()
                    .find("B", MetadataMode::Xattr)
                    .unwrap()
                    .entry
            )
            .unwrap(),
        b"def"
    );
    assert_ne!(image.bytes(), before_remove);
}

#[test]
fn remove_rejects_directories_and_rolls_back() {
    let mut image = Image::create(&CreateOptions {
        name: "DIRS".to_owned(),
        blocks: 280,
    })
    .unwrap();
    image.create_directory(&MkdirOptions::new("Games")).unwrap();
    let original = image.bytes().to_vec();

    assert!(matches!(
        image.remove_file(&RemoveOptions::new("Games")).unwrap_err(),
        A2FuseError::NotAFile(_)
    ));
    assert_eq!(image.bytes(), original);
}
