use std::ffi::OsStr;
use std::time::Duration;

use fuser::{
    BsdFileFlags, Errno, FileHandle, Filesystem, FopenFlags, Generation, INodeNo, LockOwner,
    OpenAccMode, OpenFlags, RenameFlags, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty,
    ReplyEntry, ReplyOpen, ReplyStatfs, ReplyWrite, ReplyXattr, Request, TimeOrNow, WriteFlags,
};

use crate::prodos::{MetadataMode, Volume};

use super::attrs::{XATTR_NAMES, file_attr, xattr};
use super::inode::{Inode, InodeTable};

const ATTRIBUTE_TTL: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub struct ReadOnlyFilesystem {
    volume: Volume,
    inodes: InodeTable,
    metadata_mode: MetadataMode,
}

impl ReadOnlyFilesystem {
    pub fn new(volume: Volume, metadata_mode: MetadataMode) -> Self {
        let inodes = InodeTable::build(&volume, metadata_mode);
        Self {
            volume,
            inodes,
            metadata_mode,
        }
    }

    fn inode(&self, number: INodeNo) -> Option<&Inode> {
        self.inodes.get(number.0)
    }

    fn reply_read_only(reply: ReplyEmpty) {
        reply.error(Errno::EROFS);
    }
}

impl Filesystem for ReadOnlyFilesystem {
    fn lookup(&self, req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        let Some(name) = name.to_str() else {
            reply.error(Errno::ENOENT);
            return;
        };
        match self.inodes.lookup(parent.0, name) {
            Some(inode) => reply.entry(
                &ATTRIBUTE_TTL,
                &file_attr(inode, req.uid(), req.gid()),
                Generation(0),
            ),
            None => reply.error(Errno::ENOENT),
        }
    }

    fn getattr(&self, req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        match self.inode(ino) {
            Some(inode) => reply.attr(&ATTRIBUTE_TTL, &file_attr(inode, req.uid(), req.gid())),
            None => reply.error(Errno::ENOENT),
        }
    }

    fn open(&self, _req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
        let Some(inode) = self.inode(ino) else {
            reply.error(Errno::ENOENT);
            return;
        };
        if inode.is_directory() {
            reply.error(Errno::EISDIR);
            return;
        }
        if flags.acc_mode() != OpenAccMode::O_RDONLY || flags.0 & libc::O_TRUNC != 0 {
            reply.error(Errno::EROFS);
            return;
        }
        reply.opened(FileHandle(0), FopenFlags::empty());
    }

    fn read(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyData,
    ) {
        let Some(fork) = self.inode(ino).and_then(|inode| inode.fork.as_ref()) else {
            reply.error(Errno::ENOENT);
            return;
        };
        match self.volume.read_fork(fork) {
            Ok(data) => {
                let start = usize::try_from(offset)
                    .unwrap_or(usize::MAX)
                    .min(data.len());
                let end = start.saturating_add(size as usize).min(data.len());
                reply.data(&data[start..end]);
            }
            Err(error) => {
                tracing::warn!(%error, inode = ino.0, "could not read ProDOS file");
                reply.error(Errno::EIO);
            }
        }
    }

    fn readdir(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    ) {
        let Some(directory) = self.inode(ino) else {
            reply.error(Errno::ENOENT);
            return;
        };
        if !directory.is_directory() {
            reply.error(Errno::ENOTDIR);
            return;
        }

        let entries = std::iter::once((directory.number, "."))
            .chain(std::iter::once((directory.parent, "..")))
            .chain(directory.children.iter().filter_map(|number| {
                self.inodes
                    .get(*number)
                    .map(|inode| (inode.number, inode.name.as_str()))
            }));

        for (index, (number, name)) in entries.skip(offset as usize).enumerate() {
            let inode = self
                .inodes
                .get(number)
                .expect("directory entry inode exists");
            if reply.add(
                INodeNo(number),
                offset + index as u64 + 1,
                if inode.is_directory() {
                    fuser::FileType::Directory
                } else {
                    fuser::FileType::RegularFile
                },
                name,
            ) {
                break;
            }
        }
        reply.ok();
    }

    fn statfs(&self, _req: &Request, _ino: INodeNo, reply: ReplyStatfs) {
        let blocks = u64::from(self.volume.header.total_blocks);
        reply.statfs(
            blocks,
            0,
            0,
            self.inodes.inodes.len() as u64,
            0,
            512,
            15,
            512,
        );
    }

    fn getxattr(&self, _req: &Request, ino: INodeNo, name: &OsStr, size: u32, reply: ReplyXattr) {
        if self.metadata_mode != MetadataMode::Xattr {
            reply.error(Errno::ENOATTR);
            return;
        }
        let Some(value) = self
            .inode(ino)
            .and_then(|inode| inode.entry.as_ref())
            .and_then(|entry| name.to_str().and_then(|name| xattr(entry, name)))
        else {
            reply.error(Errno::ENOATTR);
            return;
        };
        if size == 0 {
            reply.size(value.len() as u32);
        } else if (size as usize) < value.len() {
            reply.error(Errno::ERANGE);
        } else {
            reply.data(value.as_bytes());
        }
    }

    fn listxattr(&self, _req: &Request, ino: INodeNo, size: u32, reply: ReplyXattr) {
        if self.metadata_mode != MetadataMode::Xattr
            || self
                .inode(ino)
                .and_then(|inode| inode.entry.as_ref())
                .is_none()
        {
            reply.size(0);
        } else if size == 0 {
            reply.size(XATTR_NAMES.len() as u32);
        } else if (size as usize) < XATTR_NAMES.len() {
            reply.error(Errno::ERANGE);
        } else {
            reply.data(XATTR_NAMES);
        }
    }

    fn setattr(
        &self,
        _req: &Request,
        _ino: INodeNo,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        _size: Option<u64>,
        _atime: Option<TimeOrNow>,
        _mtime: Option<TimeOrNow>,
        _ctime: Option<std::time::SystemTime>,
        _fh: Option<FileHandle>,
        _crtime: Option<std::time::SystemTime>,
        _chgtime: Option<std::time::SystemTime>,
        _bkuptime: Option<std::time::SystemTime>,
        _flags: Option<BsdFileFlags>,
        reply: ReplyAttr,
    ) {
        reply.error(Errno::EROFS);
    }

    fn write(
        &self,
        _req: &Request,
        _ino: INodeNo,
        _fh: FileHandle,
        _offset: u64,
        _data: &[u8],
        _write_flags: WriteFlags,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyWrite,
    ) {
        reply.error(Errno::EROFS);
    }

    fn mkdir(
        &self,
        _req: &Request,
        _parent: INodeNo,
        _name: &OsStr,
        _mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        reply.error(Errno::EROFS);
    }

    fn unlink(&self, _req: &Request, _parent: INodeNo, _name: &OsStr, reply: ReplyEmpty) {
        Self::reply_read_only(reply);
    }

    fn rmdir(&self, _req: &Request, _parent: INodeNo, _name: &OsStr, reply: ReplyEmpty) {
        Self::reply_read_only(reply);
    }

    fn rename(
        &self,
        _req: &Request,
        _parent: INodeNo,
        _name: &OsStr,
        _newparent: INodeNo,
        _newname: &OsStr,
        _flags: RenameFlags,
        reply: ReplyEmpty,
    ) {
        Self::reply_read_only(reply);
    }

    fn setxattr(
        &self,
        _req: &Request,
        _ino: INodeNo,
        _name: &OsStr,
        _value: &[u8],
        _flags: i32,
        _position: u32,
        reply: ReplyEmpty,
    ) {
        Self::reply_read_only(reply);
    }

    fn removexattr(&self, _req: &Request, _ino: INodeNo, _name: &OsStr, reply: ReplyEmpty) {
        Self::reply_read_only(reply);
    }
}
