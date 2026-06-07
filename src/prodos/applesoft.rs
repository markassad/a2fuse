use crate::error::{A2FuseError, Result};
use tracing::debug;

const APPLESOFT_LOAD_ADDRESS: u16 = 0x0801;

const TOKENS: &[(u8, &str)] = &[
    (0xea, "MID$"),
    (0xe9, "RIGHT$"),
    (0xe8, "LEFT$"),
    (0xe7, "CHR$"),
    (0xe4, "STR$"),
    (0xd7, "SCRN("),
    (0xc3, "SPC("),
    (0xc0, "TAB("),
    (0xa4, "LOMEM:"),
    (0xa3, "HIMEM:"),
    (0xa2, "VTAB"),
    (0xa0, "COLOR="),
    (0x99, "SCALE="),
    (0x98, "ROT="),
    (0x96, "HTAB"),
    (0x92, "HCOLOR="),
    (0x8f, "VLIN"),
    (0x8e, "HLIN"),
    (0x8d, "PLOT"),
    (0x8b, "IN#"),
    (0x8a, "PR#"),
    (0x90, "HGR2"),
    (0xdc, "LOG"),
    (0xdd, "EXP"),
    (0xde, "COS"),
    (0xdf, "SIN"),
    (0xe0, "TAN"),
    (0xe1, "ATN"),
    (0xda, "SQR"),
    (0xdb, "RND"),
    (0xd9, "POS"),
    (0xd8, "PDL"),
    (0xd6, "FRE"),
    (0xd5, "USR"),
    (0xd4, "ABS"),
    (0xd3, "INT"),
    (0xd2, "SGN"),
    (0xe6, "ASC"),
    (0xe5, "VAL"),
    (0xe3, "LEN"),
    (0xe2, "PEEK"),
    (0xd1, "<"),
    (0xd0, "="),
    (0xcf, ">"),
    (0xce, "OR"),
    (0xcd, "AND"),
    (0xcc, "^"),
    (0xcb, "/"),
    (0xca, "*"),
    (0xc9, "-"),
    (0xc8, "+"),
    (0xc7, "STEP"),
    (0xc6, "NOT"),
    (0xc5, "AT"),
    (0xc4, "THEN"),
    (0xc2, "FN"),
    (0xc1, "TO"),
    (0xbf, "NEW"),
    (0xbe, "GET"),
    (0xbd, "CLEAR"),
    (0xbc, "LIST"),
    (0xbb, "CONT"),
    (0xba, "PRINT"),
    (0xba, "?"),
    (0xb9, "POKE"),
    (0xb8, "DEF"),
    (0xb7, "SAVE"),
    (0xb6, "LOAD"),
    (0xb5, "WAIT"),
    (0xb4, "ON"),
    (0xb3, "STOP"),
    (0xb2, "REM"),
    (0xb1, "RETURN"),
    (0xb0, "GOSUB"),
    (0xaf, "&"),
    (0xae, "RESTORE"),
    (0xad, "IF"),
    (0xac, "RUN"),
    (0xab, "GOTO"),
    (0xaa, "LET"),
    (0xa9, "SPEED="),
    (0xa8, "STORE"),
    (0xa7, "RECALL"),
    (0xa6, "RESUME"),
    (0xa5, "ONERR"),
    (0xa1, "POP"),
    (0x9f, "FLASH"),
    (0x9e, "INVERSE"),
    (0x9d, "NORMAL"),
    (0x9c, "NOTRACE"),
    (0x9b, "TRACE"),
    (0x9a, "SHLOAD"),
    (0x97, "HOME"),
    (0x95, "XDRAW"),
    (0x94, "DRAW"),
    (0x93, "HPLOT"),
    (0x91, "HGR"),
    (0x89, "TEXT"),
    (0x88, "GR"),
    (0x87, "READ"),
    (0x86, "DIM"),
    (0x85, "DEL"),
    (0x84, "INPUT"),
    (0x83, "DATA"),
    (0x82, "NEXT"),
    (0x81, "FOR"),
    (0x80, "END"),
    (0x8c, "CALL"),
];

const DETOKENS: [&str; 0x6b] = [
    "END", "FOR", "NEXT", "DATA", "INPUT", "DEL", "DIM", "READ", "GR", "TEXT", "PR#", "IN#",
    "CALL", "PLOT", "HLIN", "VLIN", "HGR2", "HGR", "HCOLOR=", "HPLOT", "DRAW", "XDRAW", "HTAB",
    "HOME", "ROT=", "SCALE=", "SHLOAD", "TRACE", "NOTRACE", "NORMAL", "INVERSE", "FLASH", "COLOR=",
    "POP", "VTAB", "HIMEM:", "LOMEM:", "ONERR", "RESUME", "RECALL", "STORE", "SPEED=", "LET",
    "GOTO", "RUN", "IF", "RESTORE", "&", "GOSUB", "RETURN", "REM", "STOP", "ON", "WAIT", "LOAD",
    "SAVE", "DEF", "POKE", "PRINT", "CONT", "LIST", "CLEAR", "GET", "NEW", "TAB(", "TO", "FN",
    "SPC(", "THEN", "AT", "NOT", "STEP", "+", "-", "*", "/", "^", "AND", "OR", ">", "=", "<",
    "SGN", "INT", "ABS", "USR", "FRE", "SCRN(", "PDL", "POS", "SQR", "RND", "LOG", "EXP", "COS",
    "SIN", "TAN", "ATN", "PEEK", "LEN", "STR$", "VAL", "ASC", "CHR$", "LEFT$", "RIGHT$", "MID$",
];

pub fn tokenize_program(text: &str) -> Result<Vec<u8>> {
    debug!("tokenizing AppleSoft BASIC text");
    let mut parsed = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            continue;
        }
        let (number, body) = parse_line_number(line)?;
        debug!(
            line_number = number,
            text = body,
            "parsed BASIC source line"
        );
        parsed.push((number, tokenize_line(body)?));
    }
    if parsed.is_empty() {
        return Err(A2FuseError::InvalidApplesoft(
            "expected at least one BASIC line".to_owned(),
        ));
    }

    let mut output = Vec::new();
    let mut address = APPLESOFT_LOAD_ADDRESS;
    for (index, (number, body)) in parsed.iter().enumerate() {
        let line_size = 5_usize
            .checked_add(body.len())
            .ok_or_else(|| A2FuseError::InvalidApplesoft("line is too long".to_owned()))?;
        let line_delta = u16::try_from(line_size)
            .map_err(|_| A2FuseError::InvalidApplesoft("line address overflow".to_owned()))?;
        let next_address = address
            .checked_add(line_delta)
            .ok_or_else(|| A2FuseError::InvalidApplesoft("line address overflow".to_owned()))?;
        debug!(
            line_number = *number,
            current_address = address,
            next_address,
            token_bytes = body.len(),
            "encoding BASIC line"
        );
        output.extend_from_slice(&next_address.to_le_bytes());
        output.extend_from_slice(&number.to_le_bytes());
        output.extend_from_slice(body);
        output.push(0);
        address = next_address;
        if index + 1 == parsed.len() {
            output.extend_from_slice(&0_u16.to_le_bytes());
        }
    }
    debug!(
        bytes = output.len(),
        "finished tokenizing AppleSoft BASIC program"
    );
    Ok(output)
}

pub fn detokenize_program(bytes: &[u8]) -> Result<String> {
    debug!(bytes = bytes.len(), "detokenizing AppleSoft BASIC bytes");
    let mut output = String::new();
    let mut offset = 0_usize;
    let mut first = true;

    while offset < bytes.len() {
        if bytes.len() - offset >= 2 && bytes[offset] == 0 && bytes[offset + 1] == 0 {
            if offset + 2 != bytes.len() {
                return Err(A2FuseError::InvalidApplesoft(
                    "bytes remain after BASIC terminator".to_owned(),
                ));
            }
            debug!(offset, "found BASIC end-of-program marker");
            break;
        }
        if bytes.len() - offset < 4 {
            return Err(A2FuseError::InvalidApplesoft(format!(
                "truncated line header at byte {offset}"
            )));
        }
        let next_pointer = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        let line_number = u16::from_le_bytes([bytes[offset + 2], bytes[offset + 3]]);
        debug!(
            offset,
            next_pointer, line_number, "decoding BASIC line header"
        );
        offset += 4;
        let mut line = Vec::new();
        while offset < bytes.len() && bytes[offset] != 0 {
            line.push(bytes[offset]);
            offset += 1;
        }
        if offset >= bytes.len() {
            return Err(A2FuseError::InvalidApplesoft(format!(
                "line {line_number} is missing a terminator"
            )));
        }
        offset += 1;

        if !first {
            output.push('\n');
        }
        first = false;
        output.push_str(&line_number.to_string());
        output.push(' ');
        output.push_str(&detokenize_line(&line)?);
        debug!(
            line_number,
            next_pointer,
            line_bytes = line.len(),
            next_offset = offset,
            "decoded BASIC line"
        );

        if next_pointer == 0 {
            debug!(line_number, "line header points to end-of-program");
            break;
        }
    }

    if output.is_empty() {
        return Err(A2FuseError::InvalidApplesoft(
            "expected at least one BASIC line".to_owned(),
        ));
    }
    debug!("finished detokenizing AppleSoft BASIC program");
    Ok(output)
}

fn parse_line_number(line: &str) -> Result<(u16, &str)> {
    let digits: String = line.chars().take_while(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        return Err(A2FuseError::InvalidApplesoft(format!(
            "line is missing a line number: {line:?}"
        )));
    }
    let number = digits
        .parse::<u16>()
        .map_err(|_| A2FuseError::InvalidApplesoft(format!("invalid line number: {line:?}")))?;
    let body = line[digits.len()..].trim_start();
    Ok((number, body))
}

fn tokenize_line(line: &str) -> Result<Vec<u8>> {
    let bytes = line.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0_usize;
    let mut in_string = false;
    let mut in_rem = false;

    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'"' {
            in_string = !in_string;
            output.push(byte);
            index += 1;
            continue;
        }
        if in_string || in_rem {
            output.push(byte);
            index += 1;
            continue;
        }
        if byte == b' ' {
            index += 1;
            continue;
        }
        if let Some((token, consumed, rem)) = match_token(bytes, index) {
            output.push(token);
            index += consumed;
            if rem {
                in_rem = true;
            }
            continue;
        }
        output.push(byte);
        index += 1;
    }
    Ok(output)
}

fn detokenize_line(line: &[u8]) -> Result<String> {
    let mut output = String::new();
    let mut in_rem = false;

    for (position, byte) in line.iter().enumerate() {
        if in_rem || *byte < 0x80 {
            output.push(char::from(*byte));
            continue;
        }
        let token_index = usize::from(*byte - 0x80);
        let token = DETOKENS
            .get(token_index)
            .ok_or_else(|| A2FuseError::InvalidApplesoft(format!("unknown token ${byte:02X}")))?;
        if should_insert_space_before(&output, token) {
            output.push(' ');
        }
        output.push_str(token);
        let next_byte = line.get(position + 1).copied();
        if should_insert_space_after(token, next_byte) {
            output.push(' ');
        }
        if *byte == 0xb2 {
            in_rem = true;
        }
    }
    Ok(output)
}

fn match_token(bytes: &[u8], index: usize) -> Option<(u8, usize, bool)> {
    for (token, keyword) in TOKENS {
        let keyword_bytes = keyword.as_bytes();
        let end = index + keyword_bytes.len();
        if end > bytes.len() {
            continue;
        }
        if !eq_ignore_ascii_case(&bytes[index..end], keyword_bytes) {
            continue;
        }
        return Some((*token, keyword_bytes.len(), *token == 0xb2));
    }
    None
}

fn eq_ignore_ascii_case(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

fn should_insert_space_before(current: &str, token: &str) -> bool {
    if !is_word_like_token_text(token) {
        return false;
    }
    matches!(
        current.chars().next_back(),
        Some(ch) if ch != ' ' && (ch.is_ascii_alphanumeric() || ch == '"' || ch == ')' || ch == '$')
    )
}

fn should_insert_space_after(token: &str, next_byte: Option<u8>) -> bool {
    if !is_word_like_token_text(token) {
        return false;
    }
    match next_byte {
        Some(b' ') => false,
        Some(byte) if byte < 0x80 => byte.is_ascii_alphanumeric() || byte == b'$' || byte == b'"',
        Some(byte) => is_word_like_token_byte(byte),
        None => false,
    }
}

fn is_word_like_token_text(token: &str) -> bool {
    !token.ends_with('(')
        && !token.ends_with(':')
        && token
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() || byte == b'$')
}

fn is_word_like_token_byte(token: u8) -> bool {
    if token < 0x80 {
        return false;
    }
    DETOKENS
        .get(usize::from(token - 0x80))
        .is_some_and(|token_text| is_word_like_token_text(token_text))
}
