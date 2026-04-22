#![allow(missing_docs)]

use crate::convertions::{BYTE_MAP, MULTIBYTE};
use std::fs;
use std::io::{BufReader, BufWriter, Read, Result, Write};
use std::path::Path;

const STREAM_CHUNK: usize = 64 * 1024;

pub fn convert_file(entry_path: &Path, dest_path: &Path) -> Result<()> {
    let tmp_path = tmp_destination(dest_path);
    let source = fs::File::open(entry_path)?;
    let dest = fs::File::create(&tmp_path)?;
    let mut reader = BufReader::new(source);
    let mut writer = BufWriter::new(dest);
    if let Err(e) =
        convert_stream(&mut reader, &mut writer, STREAM_CHUNK).and_then(|()| writer.flush())
    {
        let _ = fs::remove_file(&tmp_path);
        return Err(e);
    }
    fs::rename(&tmp_path, dest_path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp_path);
    })
}

fn tmp_destination(dest_path: &Path) -> std::path::PathBuf {
    let mut name = dest_path.file_name().unwrap_or_default().to_os_string();
    name.push(".exportbranch.tmp");
    dest_path.with_file_name(name)
}

pub fn convert_buffer(buffer: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(buffer.len());
    convert_prefix(buffer, buffer.len(), &mut out);
    out
}

/// Streams `reader` → `writer`, applying conversions in `chunk_size`-sized
/// passes. A tail of `MAX_PATTERN-1` bytes is held back between chunks so a
/// multibyte pattern straddling a chunk boundary still matches.
pub fn convert_stream<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    chunk_size: usize,
) -> Result<()> {
    let max_pattern: usize = MULTIBYTE.iter().map(|(p, _)| p.len()).max().unwrap_or(1);
    let keep = max_pattern.saturating_sub(1);

    let mut buf: Vec<u8> = Vec::with_capacity(chunk_size + max_pattern);
    let mut scratch = vec![0u8; chunk_size];
    let mut out: Vec<u8> = Vec::with_capacity(chunk_size + max_pattern);

    loop {
        let n = reader.read(&mut scratch)?;
        if n == 0 {
            // EOF: process the rest in full so any pending multibyte pattern
            // gets matched (or its prefix becomes plain bytes).
            out.clear();
            convert_prefix(&buf, buf.len(), &mut out);
            writer.write_all(&out)?;
            return Ok(());
        }
        buf.extend_from_slice(&scratch[..n]);

        if buf.len() <= keep {
            continue;
        }

        // Process up to `safe_end` so any multibyte pattern starting before
        // `safe_end` has at least `max_pattern` bytes visible in `buf`.
        let safe_end = buf.len() - keep;
        out.clear();
        let consumed = convert_prefix(&buf, safe_end, &mut out);
        writer.write_all(&out)?;
        buf.drain(..consumed);
    }
}

/// Process `input[..end]`, but allow multibyte patterns starting before
/// `end` to consume bytes past `end` (the slice still gives us visibility
/// up to `input.len()`). Output is appended to `out`; returns the number of
/// input bytes consumed (`>= end`).
fn convert_prefix(input: &[u8], end: usize, out: &mut Vec<u8>) -> usize {
    let mut i = 0;

    while i < end {
        if let Some((pattern, replacement)) = match_multibyte(&input[i..]) {
            out.extend_from_slice(replacement);
            i += pattern.len();
            continue;
        }

        out.push(BYTE_MAP[input[i] as usize]);
        i += 1;
    }

    i
}

#[inline]
fn match_multibyte(slice: &[u8]) -> Option<(&'static [u8], &'static [u8])> {
    // Dispatch by first byte: only `\r` and `c` are prefixes of any
    // multibyte pattern, so most positions short-circuit immediately.
    match slice.first()? {
        b'\r' | b'c' => {}
        _ => return None,
    }

    for &(pattern, replacement) in MULTIBYTE {
        if slice.starts_with(pattern) {
            return Some((pattern, replacement));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_buffer_crlf_vira_lf() {
        assert_eq!(convert_buffer(b"a\r\nb"), b"a\nb");
    }

    #[test]
    fn convert_buffer_texto_chr_251_vira_chr_42() {
        assert_eq!(convert_buffer(b"chr(251)"), b"chr(42)");
    }

    #[test]
    fn convert_buffer_texto_chr_30_vira_chr_94() {
        assert_eq!(convert_buffer(b"chr(30)"), b"chr(94)");
    }

    #[test]
    fn convert_buffer_byte_251_vira_42() {
        assert_eq!(convert_buffer(&[0xFB]), b"*");
    }

    #[test]
    fn convert_buffer_byte_26_vira_espaco() {
        assert_eq!(convert_buffer(&[26]), b" ");
    }

    #[test]
    fn convert_buffer_byte_160_vira_a() {
        // CP850 `á` (0xA0) → ASCII `a`.
        assert_eq!(convert_buffer(&[0xA0]), b"a");
    }

    #[test]
    fn convert_buffer_bytes_acentuados_cp850() {
        // C, a, e, o, i, u, c
        let input = [0x80, 0xA0, 0x82, 0xA2, 0xA1, 0xA3, 0x87];
        assert_eq!(convert_buffer(&input), b"Caeoiuc");
    }

    #[test]
    fn convert_buffer_byte_ascii_nao_altera() {
        let text = b"Hello, World! 0123456789";
        assert_eq!(convert_buffer(text), text);
    }

    #[test]
    fn convert_buffer_vazio_retorna_vazio() {
        assert!(convert_buffer(b"").is_empty());
    }

    #[test]
    fn convert_buffer_mistura_multibyte_e_byte() {
        // byte 0xFB dentro de um parágrafo cercado por CRLF.
        let input = [b'a', b'\r', b'\n', 0xFB, b'\r', b'\n'];
        assert_eq!(convert_buffer(&input), b"a\n*\n");
    }

    fn run_stream(input: &[u8], chunk_size: usize) -> Vec<u8> {
        let mut reader = std::io::Cursor::new(input);
        let mut writer: Vec<u8> = Vec::new();
        convert_stream(&mut reader, &mut writer, chunk_size).unwrap();
        writer
    }

    #[test]
    fn convert_stream_equivale_a_convert_buffer_em_chunks() {
        let mut input: Vec<u8> = Vec::new();
        for _ in 0..2_000 {
            input.extend_from_slice(b"chr(251)\r\n");
        }
        for chunk in [1, 4, 7, 8, 9, 64, 1024] {
            assert_eq!(run_stream(&input, chunk), convert_buffer(&input));
        }
    }

    #[test]
    fn convert_stream_padroes_em_offsets_chunk_boundary() {
        // Para cada k em 1..=7, garante que o padrão "chr(251)" iniciado em
        // offset (chunk_size - k) é resolvido corretamente apesar do split.
        let chunk_size = 16;
        for k in 1..=7usize {
            let mut input = vec![b'.'; chunk_size - k];
            input.extend_from_slice(b"chr(251)tail");
            let expected = convert_buffer(&input);
            let actual = run_stream(&input, chunk_size);
            assert_eq!(actual, expected, "falhou para k={k}");
        }
    }

    #[test]
    fn convert_stream_crlf_em_boundary() {
        // \r no final de um chunk, \n no começo do próximo.
        let mut input = vec![b'.'; 15];
        input.push(b'\r');
        input.push(b'\n');
        input.extend_from_slice(b"end");
        let expected = convert_buffer(&input);
        let actual = run_stream(&input, 16);
        assert_eq!(actual, expected);
    }

    /// Garantia de compatibilidade: o novo single-pass deve produzir o mesmo
    /// output que o loop sequencial antigo (que aplicava cada conversão em
    /// um pass completo sobre o buffer).
    #[test]
    fn convert_buffer_equivale_ao_algoritmo_antigo() {
        let fixture: Vec<u8> = (0u8..=255u8)
            .chain(
                b"\r\nchr(251) chr(30) chr(24) chr(31) body\r\n"
                    .iter()
                    .copied(),
            )
            .collect();
        assert_eq!(convert_buffer(&fixture), convert_buffer_legacy(&fixture));
    }

    /// Reimplementação byte-a-byte do algoritmo antigo (multi-pass) para
    /// servir de oráculo nos testes. NÃO usada em produção.
    fn convert_buffer_legacy(input: &[u8]) -> Vec<u8> {
        const LEGACY: [(&[u8], &[u8]); 38] = [
            (&[13, 10], &[10]),
            (&[26], &[32]),
            (b"chr(251)", b"chr(42)"),
            (&[251], &[42]),
            (b"chr(24)", b"chr(94)"),
            (b"chr(30)", b"chr(94)"),
            (b"chr(31)", b"chr(86)"),
            (&[30], &[94]),
            (&[31], &[86]),
            (&[193], &[196]),
            (&[194], &[196]),
            (&[180], &[179]),
            (&[195], &[179]),
            (&[183], b"A"),
            (&[181], b"A"),
            (&[142], b"A"),
            (&[199], b"A"),
            (&[144], b"E"),
            (&[153], b"O"),
            (&[128], b"C"),
            (&[132], b"a"),
            (&[133], b"a"),
            (&[131], b"a"),
            (&[160], b"a"),
            (&[225], b"a"),
            (&[198], b"a"),
            (&[130], b"e"),
            (&[136], b"e"),
            (&[137], b"e"),
            (&[161], b"i"),
            (&[147], b"o"),
            (&[162], b"o"),
            (&[148], b"o"),
            (&[163], b"u"),
            (&[129], b"u"),
            (&[135], b"c"),
            (&[166], b"."),
            (&[167], b"."),
        ];

        let mut buf = input.to_vec();
        for &(from, to) in &LEGACY {
            if from.is_empty() {
                continue;
            }
            let mut i = 0;
            while i < buf.len() {
                if buf[i..].starts_with(from) {
                    buf.splice(i..i + from.len(), to.iter().copied());
                    if to.is_empty() {
                        continue;
                    }
                    i += to.len();
                } else {
                    i += 1;
                }
            }
        }
        buf
    }
}
