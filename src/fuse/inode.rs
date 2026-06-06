use std::collections::BTreeMap;

use crate::prodos::{DirectoryEntry, MetadataMode, Node, Volume};

pub const ROOT_INODE: u64 = 1;

#[derive(Clone, Debug)]
pub struct Inode {
    pub number: u64,
    pub parent: u64,
    pub name: String,
    pub entry: Option<DirectoryEntry>,
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
        nodes
            .iter()
            .map(|node| {
                let number = self.inodes.len() as u64 + 1;
                self.inodes.insert(
                    number,
                    Inode {
                        number,
                        parent,
                        name: node.host_name(metadata_mode),
                        entry: Some(node.entry.clone()),
                        children: Vec::new(),
                    },
                );
                let children = self.add_nodes(number, &node.children, metadata_mode);
                self.inodes
                    .get_mut(&number)
                    .expect("new inode exists")
                    .children = children;
                number
            })
            .collect()
    }
}
