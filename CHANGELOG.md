# Changelog

All notable changes to `exportbranch` are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project does not
yet follow strict SemVer.

## [0.1.8] - 2026-04-25

### Added
- `-q`, `--quiet`: suppress all non-error output. Useful in scripts and CI;
  conflicts with `--show`.
- `--completions <SHELL>`: emit a shell completion script for `bash`, `zsh`,
  `fish`, `powershell` or `elvish`. Skips `-s`/`-d` validation so it can be
  invoked standalone.
- `--version` is now enriched with the short git SHA and commit date
  captured at build time (e.g. `exportbranch 0.1.8 (a1b2c3d, 2026-04-25)`),
  so support tickets can pinpoint the exact build. Falls back to
  `unknown` when built outside a git checkout.
- End-of-run summary line: `Done in Ns · X files: Y converted, Z copied,
  W skipped`. Backed by `ExportStats` aggregated through the parallel
  walk via `try_reduce`.
- `benches/walk.rs`: end-to-end criterion bench for the directory walk
  (cold + warm). Lets us catch regressions in the syscall/HashMap path,
  which dominates many-small-files workloads.

### Changed
- **Output**: per-file lines collapsed from 3 to 1 (`convert SRC → DST`,
  `copy    SRC → DST`). Status moved entirely to **stderr** so stdout is
  free for piping. Banner trimmed to `source = …`, `destination = …`,
  `Exporting…`. The `\r\n` literals are gone (no more `^M` on Linux
  terminals).
- `--show` no longer prints filters/only-copy/flags when they equal the
  built-in defaults — only customised entries surface.
- `FileUpdate` carries `only_copy: bool` so the summary can split
  `converted` from `copied`.
- `export()` returns `ExportOutcome { updates, stats }` instead of
  `Vec<FileUpdate>`. `ExportBranch::perform_exporting` returns the
  aggregated `ExportStats` for the caller to fold.

### Performance
- Walk: replaced per-entry `is_dir()`/`is_file()` (two `stat` syscalls)
  with `DirEntry::file_type()` (no extra syscall on most filesystems),
  and dropped the `exists()` check before `create_dir_all`. Both
  measurably reduce syscall traffic on dense trees.
- `FileChecker`: keys are now `PathBuf` instead of `String`, eliminating
  the `to_string_lossy().into_owned()` allocation per file. Persisted
  format is unchanged — existing metadata files round-trip.
- `convert_file`: `BufReader`/`BufWriter` capacity bumped to 64 KB,
  reducing write syscalls on large `.a`/`.so` payloads.
- `convert_stream`: scratch buffers reused across calls via
  `thread_local!` (`buf`, `scratch`, `out`), avoiding ~256 KB of `Vec`
  allocation per file in the parallel walk.
- `walk_warm` bench (5000 UpToDate files, WSL2): ~6.6 ms.

### CI / build
- New `.cargo/config.toml` pins `rustflags = ["-C", "target-cpu=x86-64"]`
  so release binaries stay compatible with Intel Core i3/i5 3rd-gen
  (Ivy Bridge) and similar CPUs without AVX2/BMI/FMA. AVX2 codepaths
  inside `regex`/`aho-corasick`/`memchr` continue to light up at runtime
  via `is_x86_feature_detected!` on newer hardware.
- `build.rs`: captures `GIT_SHA` and `GIT_DATE` for `--version`. No new
  dependencies; falls back gracefully outside a git checkout.

### Dependencies
- Added `clap_complete = "4"` (direct) for shell completions.

## [0.1.7] - 2026-04-24

### Added
- `--debug [PATH]`: opt-in debug log. Captures argv, parsed configuration,
  each source's canonicalized path and resolved destination, per-source
  success/failure, and total elapsed time. Each line is prefixed with the
  milliseconds elapsed since start. If `--debug` is passed without a
  value, writes to `exportbranch-<unix_secs>.log` in the current
  directory. Intended for diagnosing field bug reports where reproducing
  the environment is impractical.

### Fixed
- **Windows**: `-d` with a path past the drive is now preserved. v0.1.6
  dropped anything after the drive (Windows path-join semantics), so
  `exportbranch -s T:\new -d L:\trunk2\` landed files at `L:\new` instead
  of `L:\trunk2\new`. The destination is now used as a root and the
  source is mirrored under it (minus its drive): `L:\trunk2\new`,
  `R:\Trunk2\trunk\frente`, etc. The single-drive case `-d R:\` stays
  identical to v0.1.6 (`R:\trunk\frente\…`), so wrapper scripts that
  rely on `-d <drive>:\` do not need to change.
- **Windows**: items of `-s` without a drive `Prefix` now inherit the
  drive of the first item. `exportbranch -s T:\new;\src;\include` is
  equivalent to `-s T:\new;T:\src;T:\include`. Previously the drive-less
  items failed individually with `MissingDrivePrefix`, silently dropping
  everything but the first source.

### Changed
- Windows-only: `destination_path` drops the `RootDir` component from
  the stripped source before joining onto the destination, so
  `Path::join` treats the remainder as relative and the destination's
  path-after-drive is kept intact.

## [0.1.6] - 2026-04-24

### Changed
- **Windows (breaking)**: destination path now matches the historical
  `MateusZanchoNeto/exportbranch` behaviour: the source drive `Prefix`
  is stripped and joined onto the destination drive, and anything after
  the drive in `-d` is dropped (Windows path-join semantics replace the
  destination's path-after-prefix when the right side has a root). So
  `-s L:\trunk\frente -d R:\` lands files at `R:\trunk\frente\…`, and
  `-s L:\trunk\include -d R:\` lands them at `R:\trunk\include\…` —
  wrapper scripts can invoke the CLI repeatedly with the same `-d` drive
  and several sources without collisions.
- Reverts v0.1.5's "destination used as-is on every platform" on Windows
  only. Linux keeps the flat behaviour.

### Added
- `ExportError::MissingDrivePrefix`: raised on Windows when the path
  passed to `-s` has no drive `Prefix` component, since in that case the
  destination drive cannot be derived.
- Unit tests in `lib::tests` pinning the Windows join semantics and the
  `MissingDrivePrefix` error case (`#[cfg(windows)]`).

### Removed
- `tests/end_to_end.rs` is now Linux-only (`#![cfg(not(windows))]`).
  The Windows join semantics collapse a same-drive `dst` onto the source
  path, making sibling-tempdir integration tests meaningless on Windows;
  the unit tests above cover the destination-path logic there.

## [0.1.5] - 2026-04-24

### Changed
- **Windows (breaking)**: destination path is used as-is on every platform;
  the canonical source is no longer mirrored underneath it. Previously,
  exporting `C:\ProdutosSG\Branches\Trunk` to `E:\Trunk` produced
  `E:\Trunk\ProdutosSG\Branches\Trunk\...`; it now produces `E:\Trunk\...`,
  matching Linux semantics and the intent of the `-d` flag. Reverts the
  v0.1.3 mirror behaviour.
- End-to-end tests simplified: `dest_root_for` helper removed, all
  assertions reference `dst` directly. A dedicated regression test pins
  the flat-destination contract so the behaviour is not flipped again.

### Added
- README section explaining how to reduce Windows Defender real-time
  scanning overhead on large branches: folder exclusions via
  `Add-MpPreference -ExclusionPath` for the source/destination roots, and
  a narrower process exclusion via `Add-MpPreference -ExclusionProcess`
  for `exportbranch.exe`. Real-time scanning is the main source of
  wall-clock overhead on large exports.

## [0.1.4] - 2026-04-22

### Fixed
- Windows clippy: `destination_path` map closure replaced with the
  `Component::as_os_str` method reference, satisfying
  `clippy::redundant_closure_for_method_calls` under pedantic. The lint
  only fires on Windows because the closure lives behind `cfg(windows)`.

## [0.1.3] - 2026-04-22

### Fixed
- **Windows**: source path is again mirrored under destination — a source
  like `L:\trunk\include` lands at `<destination>\trunk\include`. v0.1.2
  collapsed this to flat behaviour to work around the drive-relative join
  bug; this release restores the mirror by stripping both the drive
  `Prefix` *and* the `RootDir` component before joining, so the result is
  a true relative path that `PathBuf::push` appends instead of replacing.
- End-to-end tests are now platform-aware about where output lands
  (`dest_root_for(src, dst)`), so Linux flat and Windows mirror semantics
  both stay green.

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
