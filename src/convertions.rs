//! CP850/PT-BR → ASCII conversions applied to every converted file.
//!
//! Split into two tables for a single-pass byte rewriter:
//! * [`BYTE_MAP`] — identity table with 33 single-byte substitutions (e.g.
//!   CP850 `á` (0xA0) → ASCII `a`). Hot path: one lookup per input byte.
//! * [`MULTIBYTE`] — substitutions where the input pattern is more than one
//!   byte (CRLF → LF, literal text `chr(251)` → `chr(42)`, etc.).

/// Multibyte input → multibyte output substitutions, applied in order before
/// [`BYTE_MAP`] is consulted for any single byte that doesn't start a pattern.
pub const MULTIBYTE: &[(&[u8], &[u8])] = &[
    (b"\r\n", b"\n"),
    (b"chr(251)", b"chr(42)"),
    (b"chr(24)", b"chr(94)"),
    (b"chr(30)", b"chr(94)"),
    (b"chr(31)", b"chr(86)"),
];

/// Per-byte translation table: `BYTE_MAP[b]` is the byte that replaces `b`.
/// Most entries are identity; the `build_byte_map` body lists the 33
/// CP850/PT-BR overrides applied at conversion time.
pub const BYTE_MAP: [u8; 256] = build_byte_map();

const fn build_byte_map() -> [u8; 256] {
    let mut map = [0u8; 256];
    let mut i: u8 = 0;
    loop {
        map[i as usize] = i;
        if i == 255 {
            break;
        }
        i += 1;
    }

    // Control characters mapped to ASCII printables (Harbour/Compex-era
    // conventions from the original pass-list).
    map[26] = 32;
    map[30] = 94;
    map[31] = 86;

    // CP850 accented uppercase → ASCII letters.
    map[128] = b'C';
    map[142] = b'A';
    map[144] = b'E';
    map[153] = b'O';
    map[181] = b'A';
    map[183] = b'A';
    map[199] = b'A';

    // CP850 accented lowercase → ASCII letters.
    map[129] = b'u';
    map[130] = b'e';
    map[131] = b'a';
    map[132] = b'a';
    map[133] = b'a';
    map[135] = b'c';
    map[136] = b'e';
    map[137] = b'e';
    map[147] = b'o';
    map[148] = b'o';
    map[160] = b'a';
    map[161] = b'i';
    map[162] = b'o';
    map[163] = b'u';
    map[198] = b'a';
    map[225] = b'a';

    // Box-drawing glyphs → ASCII punctuation.
    map[166] = b'.';
    map[167] = b'.';
    map[180] = 179;
    map[193] = 196;
    map[194] = 196;
    map[195] = 179;

    // Miscellaneous.
    map[251] = 42;

    map
}
