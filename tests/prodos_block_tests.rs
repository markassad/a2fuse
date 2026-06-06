use a2fuse::A2FuseError;
use a2fuse::prodos::{BLOCK_SIZE, BlockDevice};

#[test]
fn reads_individual_blocks() {
    let mut image = vec![0_u8; BLOCK_SIZE * 2];
    image[BLOCK_SIZE] = 0x42;
    image[BLOCK_SIZE * 2 - 1] = 0x99;

    let device = BlockDevice::from_bytes(image).unwrap();

    assert_eq!(device.block_count(), 2);
    assert_eq!(device.read_block(1).unwrap()[0], 0x42);
    assert_eq!(device.read_block(1).unwrap()[BLOCK_SIZE - 1], 0x99);
}

#[test]
fn rejects_partial_blocks() {
    let error = BlockDevice::from_bytes(vec![0_u8; BLOCK_SIZE + 1]).unwrap_err();

    assert!(matches!(
        error,
        A2FuseError::InvalidImageLength { length } if length == BLOCK_SIZE + 1
    ));
}

#[test]
fn reports_out_of_range_blocks() {
    let device = BlockDevice::from_bytes(vec![0_u8; BLOCK_SIZE]).unwrap();

    assert!(matches!(
        device.read_block(1).unwrap_err(),
        A2FuseError::BlockOutOfRange {
            block: 1,
            block_count: 1
        }
    ));
}
