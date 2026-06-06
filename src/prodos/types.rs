#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum StorageType {
    Deleted = 0x0,
    Seedling = 0x1,
    Sapling = 0x2,
    Tree = 0x3,
    PascalArea = 0x4,
    Extended = 0x5,
    Subdirectory = 0xd,
    SubdirectoryHeader = 0xe,
    VolumeHeader = 0xf,
}

impl StorageType {
    pub fn from_nibble(value: u8) -> Option<Self> {
        Some(match value {
            0x0 => Self::Deleted,
            0x1 => Self::Seedling,
            0x2 => Self::Sapling,
            0x3 => Self::Tree,
            0x4 => Self::PascalArea,
            0x5 => Self::Extended,
            0xd => Self::Subdirectory,
            0xe => Self::SubdirectoryHeader,
            0xf => Self::VolumeHeader,
            _ => return None,
        })
    }

    pub fn is_regular_file(self) -> bool {
        matches!(self, Self::Seedling | Self::Sapling | Self::Tree)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AccessFlags(pub u8);

impl AccessFlags {
    pub const DESTROY: u8 = 0x80;
    pub const RENAME: u8 = 0x40;
    pub const BACKUP: u8 = 0x20;
    pub const INVISIBLE: u8 = 0x04;
    pub const WRITE: u8 = 0x02;
    pub const READ: u8 = 0x01;

    pub fn readable(self) -> bool {
        self.0 & Self::READ != 0
    }

    pub fn writable(self) -> bool {
        self.0 & Self::WRITE != 0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProdosTimestamp {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
}

impl ProdosTimestamp {
    pub fn decode(date: u16, time: u16) -> Option<Self> {
        if date == 0 && time == 0 {
            return None;
        }

        let short_year = (date >> 9) & 0x7f;
        let year = if short_year < 40 {
            2000 + short_year
        } else {
            1900 + short_year
        };
        let month = ((date >> 5) & 0x0f) as u8;
        let day = (date & 0x1f) as u8;
        let hour = ((time >> 8) & 0x1f) as u8;
        let minute = (time & 0x3f) as u8;

        if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
            return None;
        }

        Some(Self {
            year,
            month,
            day,
            hour,
            minute,
        })
    }
}
