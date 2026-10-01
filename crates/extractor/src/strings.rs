pub fn scan_ascii_strings(bytes: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = Vec::new();
    for &byte in bytes {
        if byte.is_ascii_graphic() || byte == b' ' {
            current.push(byte);
        } else {
            push_ascii_candidate(&mut out, &mut current);
        }
    }
    push_ascii_candidate(&mut out, &mut current);
    out
}

fn push_ascii_candidate(out: &mut Vec<String>, current: &mut Vec<u8>) {
    if current.len() >= 4
        && let Ok(value) = String::from_utf8(std::mem::take(current))
    {
        out.push(value);
        return;
    }
    current.clear();
}

pub fn scan_utf16le_strings(bytes: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = Vec::new();
    for chunk in bytes.chunks_exact(2) {
        let unit = u16::from_le_bytes([chunk[0], chunk[1]]);
        if (0x20..=0x7e).contains(&unit) {
            current.push(unit);
        } else {
            push_utf16_candidate(&mut out, &mut current);
        }
    }
    push_utf16_candidate(&mut out, &mut current);
    out
}

fn push_utf16_candidate(out: &mut Vec<String>, current: &mut Vec<u16>) {
    if current.len() >= 4
        && let Ok(value) = String::from_utf16(current)
    {
        out.push(value);
    }
    current.clear();
}
