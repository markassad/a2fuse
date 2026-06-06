use clap::ValueEnum;

use super::DirectoryEntry;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum MetadataMode {
    #[default]
    Xattr,
    Filename,
}

pub fn decode_filename(raw: &[u8]) -> String {
    raw.iter()
        .map(|byte| {
            let ascii = byte & 0x7f;
            match ascii {
                b'/' | b':' | 0 => '_',
                0x20..=0x7e => char::from(ascii),
                _ => '_',
            }
        })
        .collect()
}

pub fn decode_filename_with_case(raw: &[u8], case_bits: u16) -> String {
    let mut name = decode_filename(raw);
    if case_bits & 0x8000 == 0 {
        return name;
    }

    name = name
        .chars()
        .enumerate()
        .map(|(index, character)| {
            let mask = 1_u16 << (14_usize.saturating_sub(index));
            if case_bits & mask != 0 {
                character.to_ascii_lowercase()
            } else {
                character
            }
        })
        .collect();
    name
}

pub fn host_filename(entry: &DirectoryEntry, mode: MetadataMode) -> String {
    match mode {
        MetadataMode::Xattr => entry.name.clone(),
        MetadataMode::Filename => format!(
            "{},t${:02x},a${:04x}",
            entry.name, entry.file_type, entry.aux_type
        ),
    }
}
