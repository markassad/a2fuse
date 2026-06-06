use std::collections::BTreeMap;

use crate::prodos::{DirectoryEntry, FileFork, MetadataMode, Node, Volume};

pub const ROOT_INODE: u64 = 1;

#[derive(Clone, Debug)]
pub struct Inode {
    pub number: u64,
    pub parent: u64,
    pub name: String,
    pub entry: Option<DirectoryEntry>,
    pub fork: Option<FileFork>,
    pub children: Vec<u64>,
}

impl Inode {
    pub fn is_directory(&self) -> bool {
        self.entry.as_ref().is_none_or(DirectoryEntry::is_directory)
    }
}

#[derive(Debug)]
pub struct InodeTable {
    pub inodes: BTreeMap<u64, Inode>,
}

impl InodeTable {
    pub fn build(volume: &Volume, metadata_mode: MetadataMode) -> Self {
        let mut table = Self {
            inodes: BTreeMap::new(),
        };
        table.inodes.insert(
            ROOT_INODE,
            Inode {
                number: ROOT_INODE,
                parent: ROOT_INODE,
                name: volume.header.name.clone(),
                entry: None,
                fork: None,
                children: Vec::new(),
            },
        );

        let children = table.add_nodes(ROOT_INODE, &volume.root, metadata_mode);
        table
            .inodes
            .get_mut(&ROOT_INODE)
            .expect("root inode exists")
            .children = children;
        table
    }

    pub fn get(&self, number: u64) -> Option<&Inode> {
        self.inodes.get(&number)
    }

    pub fn lookup(&self, parent: u64, name: &str) -> Option<&Inode> {
        let parent = self.get(parent)?;
        parent.children.iter().find_map(|number| {
            let inode = self.get(*number)?;
            inode.name.eq_ignore_ascii_case(name).then_some(inode)
        })
    }

    fn add_nodes(&mut self, parent: u64, nodes: &[Node], metadata_mode: MetadataMode) -> Vec<u64> {
        let mut child_numbers = Vec::new();

        for node in nodes {
            let number = self.inodes.len() as u64 + 1;
            self.inodes.insert(
                number,
                Inode {
                    number,
                    parent,
                    name: node.host_name(metadata_mode),
                    entry: Some(node.entry.clone()),
                    fork: node.data_fork.clone(),
                    children: Vec::new(),
                },
            );
            let children = self.add_nodes(number, &node.children, metadata_mode);
            self.inodes
                .get_mut(&number)
                .expect("new inode exists")
                .children = children;
            child_numbers.push(number);

            if node.is_extended_file() {
                let number = self.inodes.len() as u64 + 1;
                self.inodes.insert(
                    number,
                    Inode {
                        number,
                        parent,
                        name: format!("._{}", node.host_name(metadata_mode)),
                        entry: Some(node.entry.clone()),
                        fork: node.resource_fork.clone(),
                        children: Vec::new(),
                    },
                );
                child_numbers.push(number);
            }
        }

        child_numbers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::prodos::directory::{PRODOS_ENTRIES_PER_BLOCK, PRODOS_ENTRY_LENGTH};
    use crate::prodos::{BlockDevice, MetadataMode, StorageType, Volume};

    #[test]
    fn builds_appledouble_sidecars_for_extended_files() {
        let volume = volume_with_extended_file();
        let table = InodeTable::build(&volume, MetadataMode::Xattr);
        let root = table.get(ROOT_INODE).expect("root inode exists");

        let names: Vec<_> = root
            .children
            .iter()
            .map(|number| table.get(*number).expect("inode exists").name.as_str())
            .collect();

        assert_eq!(names, ["Read.Me", "._Read.Me"]);

        let data = table.lookup(ROOT_INODE, "Read.Me").expect("data fork inode");
        let rsrc = table.lookup(ROOT_INODE, "._Read.Me").expect("resource fork inode");
        assert_eq!(data.fork.as_ref().unwrap().eof, 4);
        assert_eq!(rsrc.fork.as_ref().unwrap().eof, 5);
    }

    fn volume_with_extended_file() -> Volume {
        let mut image = vec![0_u8; crate::prodos::BLOCK_SIZE * 12];
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
        image[9 * crate::prodos::BLOCK_SIZE..9 * crate::prodos::BLOCK_SIZE + 4]
            .copy_from_slice(b"DATA");
        image[10 * crate::prodos::BLOCK_SIZE..10 * crate::prodos::BLOCK_SIZE + 5]
            .copy_from_slice(b"RSRC!");

        Volume::from_device(BlockDevice::from_bytes(image).unwrap()).unwrap()
    }

    fn directory_links(image: &mut [u8], block: usize, previous: u16, next: u16) {
        let start = block * crate::prodos::BLOCK_SIZE;
        image[start..start + 2].copy_from_slice(&previous.to_le_bytes());
        image[start + 2..start + 4].copy_from_slice(&next.to_le_bytes());
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

    fn extended_fork(
        image: &mut [u8],
        block: usize,
        offset: usize,
        storage_type: u8,
        key_block: u16,
        eof: u32,
    ) {
        let start = block * crate::prodos::BLOCK_SIZE + offset;
        image[start] = storage_type;
        put_u16(image, start + 1, key_block);
        put_u16(image, start + 3, 1);
        image[start + 5] = eof as u8;
        image[start + 6] = (eof >> 8) as u8;
        image[start + 7] = (eof >> 16) as u8;
    }

    fn put_entry(image: &mut [u8], block: usize, slot: usize, entry: &[u8; PRODOS_ENTRY_LENGTH]) {
        let start = block * crate::prodos::BLOCK_SIZE + 4 + slot * PRODOS_ENTRY_LENGTH;
        image[start..start + PRODOS_ENTRY_LENGTH].copy_from_slice(entry);
    }

    fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u24(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset] = value as u8;
        bytes[offset + 1] = (value >> 8) as u8;
        bytes[offset + 2] = (value >> 16) as u8;
    }
}
