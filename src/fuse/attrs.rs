use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fuser::{FileAttr, FileType, INodeNo};

use crate::prodos::{DirectoryEntry, ProdosTimestamp};

use super::inode::Inode;

pub fn file_attr(inode: &Inode, uid: u32, gid: u32) -> FileAttr {
    let entry = inode.entry.as_ref();
    let kind = if inode.is_directory() {
        FileType::Directory
    } else {
        FileType::RegularFile
    };
    let size = entry.map_or(0, |entry| u64::from(entry.eof));
    let blocks = entry.map_or(0, |entry| u64::from(entry.blocks_used));
    let creation = entry
        .and_then(|entry| entry.creation)
        .and_then(timestamp_to_system_time)
        .unwrap_or(UNIX_EPOCH);
    let modification = entry
        .and_then(|entry| entry.modification)
        .and_then(timestamp_to_system_time)
        .unwrap_or(creation);

    FileAttr {
        ino: INodeNo(inode.number),
        size,
        blocks,
        atime: modification,
        mtime: modification,
        ctime: modification,
        crtime: creation,
        kind,
        perm: if inode.is_directory() { 0o555 } else { 0o444 },
        nlink: if inode.is_directory() { 2 } else { 1 },
        uid,
        gid,
        rdev: 0,
        blksize: 512,
        flags: 0,
    }
}

pub fn xattr(entry: &DirectoryEntry, name: &str) -> Option<String> {
    Some(match name {
        "prodos.type" => format!("{:#04x}", entry.file_type),
        "prodos.aux_type" => format!("{:#06x}", entry.aux_type),
        "prodos.access" => format!("{:#04x}", entry.access.0),
        "prodos.storage_type" => format!("{:#04x}", entry.storage_type as u8),
        _ => return None,
    })
}

pub const XATTR_NAMES: &[u8] =
    b"prodos.type\0prodos.aux_type\0prodos.access\0prodos.storage_type\0";

fn timestamp_to_system_time(timestamp: ProdosTimestamp) -> Option<SystemTime> {
    let days = days_from_civil(
        i32::from(timestamp.year),
        u32::from(timestamp.month),
        u32::from(timestamp.day),
    );
    let seconds = days
        .checked_mul(86_400)?
        .checked_add(i64::from(timestamp.hour) * 3_600 + i64::from(timestamp.minute) * 60)?;

    if seconds >= 0 {
        Some(UNIX_EPOCH + Duration::from_secs(seconds as u64))
    } else {
        Some(UNIX_EPOCH - Duration::from_secs(seconds.unsigned_abs()))
    }
}

// Howard Hinnant's civil-date conversion, with 1970-01-01 as day zero.
fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let adjusted_month =
        i32::try_from(month).expect("month fits i32") + if month > 2 { -3 } else { 9 };
    let day_of_year =
        (153 * adjusted_month + 2) / 5 + i32::try_from(day).expect("day fits i32") - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    i64::from(era * 146_097 + day_of_era - 719_468)
}
