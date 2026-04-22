# Changelog

All notable changes to `exportbranch` are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project does not
yet follow strict SemVer.

## [0.1.2] - 2026-04-21

### Fixed
- **Windows**: destination path no longer accidentally rebases output to the
  source's drive-relative path. The previous Windows-specific branch built
  `dst.join(stripped_source)` where `stripped_source` started with `\`,
  which under Windows path semantics is "drive-relative" and silently
  discarded `dst`. Files were written next to the source instead of under
  the requested destination, and `FileChecker::save` then failed because
  `dst` itself was never created. The destination is now used as-is on
  every platform — same behaviour Linux already had.
- `ExportError::PathPrefix` removed (no longer constructed).

## [0.1.1] - 2026-04-21

### Fixed
- `to_nanos` rewritten with `map_or` to satisfy `clippy::map_unwrap_or`
  under pedantic (CI was red on the freshly tagged 0.1.0).
- `save_e_reload_preserva_entrada` no longer fails on Windows. The fixture
  used `Duration::from_nanos(42)`, which rounds to 0 on Windows where
  `SystemTime` resolution is 100 ns; the test now uses whole seconds.
- Dockerfile's deps-only stage stubs `benches/convert.rs` so Cargo's
  manifest parse succeeds before the real sources are copied in.

## [0.1.0] - 2026-04-21

### Changed (round 3 — argument parsing)
- Argument parsing migrated to `clap` (derive). `--help` and `--version` are
  now auto-generated; long options accept the `--flag=value` form; required
  flags get a structured error pointing at what's missing. The deprecated
  `--md5` alias still works (and still warns) for one more release.
- `src/help.rs` removed — its hand-rolled help text is superseded by clap's
  output.

### Added (round 2 — robustness & performance polish)
- `convert_file` now writes to `<dest>.exportbranch.tmp` and `rename`s on
  success — interrupted conversions no longer leave truncated destination
  files.
- `cargo bench` workload via `criterion` (`benches/convert.rs`) covering
  `convert_buffer` and `convert_stream` at multiple sizes/chunk widths.
- Crate-level + per-module doc comments with `#![warn(missing_docs)]`.
- CI matrix gains a `cargo build --release --all-targets` job and a
  `docker build` job.

### Changed (round 2)
- `export_file` propagates `ExportError::Io` instead of swallowing copy /
  conversion failures with `eprintln!`. The exit code now reflects the failure.
- `is_disregarded` no longer canonicalizes per visited entry — the source
  root is canonicalized once in `lib::source_path` so a hash lookup is
  enough.
- `convert_stream` reuses the output `Vec` between chunks (zero allocations
  per inner loop iteration).
- `format_lower` rewritten with `Path::strip_prefix`, dropping the
  string-replace dance and one full path allocation.
- `print_file` locks stdout once per message — no more interleaved lines
  under `rayon`.
- `FileUpdate` collapsed to a struct (the `Remove` variant became dead
  after the error-propagation change).
- Dockerfile rebased on `rust:1-slim` (was `ubuntu:16.04`) — smaller image,
  no manual `rustup` install.

### Added
- `--exists` flag: forces re-export when the destination file is missing.
- `filtrosarquivos.exb` per-source override for `-f` file filters.
- Parallel directory walk (`rayon`) for `O(workers)` speedup on large trees.
- Streaming CP850 conversion (`BufReader`/`BufWriter`, 64 KB chunks) — no
  longer reads each file fully into memory.
- `RegexSet`-based filtering, compiled once per export instead of per
  subdirectory.
- `FileChecker` now stores modification times as nanoseconds since
  `UNIX_EPOCH`. Falls back gracefully on the legacy `Debug`-formatted entries
  for one release (those entries are silently dropped on read and re-recorded
  on next export).
- `ExportError` enum + `WithPath` trait for typed, path-attributed errors
  across the library. Library code no longer calls `unwrap`/`expect`/`exit`.
- Test suite (`cargo test`): 51 unit + integration tests, including
  characterization tests, chunk-boundary regression tests, and parallelism
  end-to-end tests.
- CI matrix on `ubuntu-latest` + `windows-latest` (`fmt --check`,
  `clippy -D warnings`, `test`).

### Changed
- **Breaking**: file-filter globs are now anchored. `*.h` matches `foo.h` but
  no longer `foo.html` (previously the unanchored regex matched any substring).
- **Breaking**: `disregarded_directories` (`bin/`, `lib/`,
  `programas_externos/conversoes/`) are now actually skipped during traversal.
  Previously the configuration was parsed but never applied.
- `--md5` is deprecated and now an alias of `--exists`. Using `--md5` prints
  a deprecation warning to stderr. The flag will be removed in a future
  release.
- `convert_buffer` rewritten as a single pass over the input using a
  precomputed `[u8; 256]` lookup table plus a multibyte dispatch. Output is
  byte-for-byte identical to the previous multi-pass implementation
  (verified by a legacy oracle test).
- `Configuration::md5()` renamed to `Configuration::exists()`.

### Fixed
- `convert_buffer` no longer infinite-loops on substitutions whose replacement
  is an empty string.
- `check_configuration_file` no longer applies the `only_copy` config file to
  `file_filters`.

### Removed
- `Box<PathBuf>` field types and several defensive `.clone()` calls.
- Multiple `process::exit` calls from library code (now confined to `main`).
