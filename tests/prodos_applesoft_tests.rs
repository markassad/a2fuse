use a2fuse::A2FuseError;
use a2fuse::prodos::{detokenize_program, tokenize_program};

#[test]
fn round_trips_simple_program() {
    let source = include_str!("../testdata/applesoft/hello_loop.bas");

    let tokenized = tokenize_program(source).unwrap();
    let detokenized = detokenize_program(&tokenized).unwrap();
    let reparsed = tokenize_program(&detokenized).unwrap();
    assert_eq!(reparsed, tokenized);
    assert_eq!(
        detokenized,
        "10 PRINT \"HELLO, APPLESOFT\"\n20 FOR I=1 TO 3\n30 PRINT I\n40 NEXT I\n50 GOTO 10"
    );
}

#[test]
fn rem_line_preserves_literal_text() {
    let source = include_str!("../testdata/applesoft/rem_and_strings.bas");
    let tokenized = tokenize_program(source).unwrap();
    let detokenized = detokenize_program(&tokenized).unwrap();
    let reparsed = tokenize_program(&detokenized).unwrap();
    assert_eq!(reparsed, tokenized);
    assert!(
        detokenized.starts_with(
            "10 REM TOKEN NAMES IN THIS COMMENT MUST STAY LITERAL: PRINT IF THEN GOTO"
        )
    );
    assert!(detokenized.contains("20 A$=\"FOR I=1 TO 10 : NEXT I\""));
}

#[test]
fn rejects_program_without_line_numbers() {
    assert!(matches!(
        tokenize_program("PRINT \"HELLO\"").unwrap_err(),
        A2FuseError::InvalidApplesoft(_)
    ));
}

#[test]
fn detokenizer_inserts_spacing_for_compact_keywords() {
    let bytes = [
        0x0e, 0x08, 0x0a, 0x00, 0x81, b'I', 0xd0, b'1', 0xc1, b'1', b'0', b'0', 0x00, 0x15, 0x08,
        0x14, 0x00, 0xba, b'I', 0x00, 0x1c, 0x08, 0x1e, 0x00, 0x82, b'I', 0x00, 0x00, 0x00,
    ];
    let detokenized = detokenize_program(&bytes).unwrap();
    assert_eq!(detokenized, "10 FOR I=1 TO 100\n20 PRINT I\n30 NEXT I");
}

#[test]
fn compact_tokens_round_trip_back_to_identical_bytes() {
    let original = [
        0x0e, 0x08, 0x0a, 0x00, 0x81, b'I', 0xd0, b'1', 0xc1, b'1', b'0', b'0', 0x00, 0x15, 0x08,
        0x14, 0x00, 0xba, b'I', 0x00, 0x1c, 0x08, 0x1e, 0x00, 0x82, b'I', 0x00, 0x00, 0x00,
    ];
    let text = detokenize_program(&original).unwrap();
    let reparsed = tokenize_program(&text).unwrap();
    assert_eq!(reparsed, original);
}

#[test]
fn sample_compact_source_round_trips() {
    let source = include_str!("../testdata/applesoft/compact_keywords.bas");
    let tokenized = tokenize_program(source).unwrap();
    let detokenized = detokenize_program(&tokenized).unwrap();
    let reparsed = tokenize_program(&detokenized).unwrap();
    assert_eq!(reparsed, tokenized);
}
#[test]
fn tokenizer_strips_non_rem_spaces() {
    let tokenized = tokenize_program("10 W = 2: REM X").unwrap();
    assert_eq!(
        tokenized,
        vec![
            0x0d, 0x08, 0x0a, 0x00, b'W', 0xd0, b'2', b':', 0xb2, b' ', b'X', 0x00, 0x00, 0x00
        ]
    );
}
