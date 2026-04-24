# ExportBranch

`ExportBranch` is a command-line tool used in conjunction with `Compex` to compile programs written in the Harbour programming language. It filters files and characters within branches before `Compex` begins compiling the project.

## Usage

```
exportbranch -s <source> -d <destination> [options]
```

### Options

| Flag                 | Description                                                                |
| -------------------- | -------------------------------------------------------------------------- |
| `-s <source>`        | Source path (use `;` to separate multiple paths).                          |
| `-d <destination>`   | Destination path (use `;` to separate multiple paths).                     |
| `-c <only_copy>`     | Glob patterns copied byte-for-byte (no CP850 conversion).                  |
| `-f <file_filters>`  | Glob patterns selected for export.                                         |
| `--exists`           | Force re-export when the destination copy is missing.                      |
| `--md5`              | **Deprecated** alias of `--exists`; will be removed in a future release.   |
| `--reload`           | Re-export all files, ignoring the modification tracker.                    |
| `--lower`            | Lowercase directory and file names under the destination.                  |
| `--show`             | Print the parsed configuration before exporting.                           |

### Globs

Patterns are matched against the **file name only** and are anchored end-to-end.
`*.h` matches `foo.h` but **not** `foo.html`.

### Destination layout (Windows)

On Windows, the destination is **always resolved under the drive of `-d`**:
the source drive `Prefix` (e.g. `L:`, `C:`) is stripped, and the rest of
the source path is joined onto the destination. Anything you pass after
the drive in `-d` is discarded — Windows path semantics drop the
destination's path-after-prefix when the right-hand side of a join has a
root.

Examples (Windows):

| `-s` | `-d` | Files land under |
| ---- | ---- | ---------------- |
| `L:\trunk\frente`  | `R:\`               | `R:\trunk\frente\…`  |
| `L:\trunk\include` | `R:\anything\else`  | `R:\trunk\include\…` |
| `C:\ProdutosSG\Branches\Trunk` | `E:\` | `E:\ProdutosSG\Branches\Trunk\…` |

The practical consequence is that a wrapper script can invoke
`exportbranch` repeatedly with the **same** `-d <drive>:\` and several
different `-s` roots without the outputs colliding at the drive root —
each source keeps its own subtree under the destination drive.

On Linux/macOS `-d` is used as-is.

### Disregarded directories

The directories `bin/`, `lib/` and `programas_externos/conversoes/` (resolved
relative to each source root) are skipped during traversal — files inside them
are neither copied nor converted.

### Per-source configuration files

If present at the root of a source directory, the following files override the
defaults for that source:

| File                          | Overrides           |
| ----------------------------- | ------------------- |
| `extecoesarquivos.exb`        | `-f <file_filters>` |
| `extecoesapenascopiar.exb`    | `-c <only_copy>`    |
| `filtrosarquivos.exb`         | `-f <file_filters>` |

Each file lists patterns separated by `;`.

## Antivirus / Windows Defender

Real-time scanning inspects every read and write the tool performs, and on
large trees this dominates the wall-clock time (hundreds of thousands of
syscalls on a full branch). Two narrow exclusions bring the exporter back
to near-disk speed while leaving the rest of the machine protected.

All commands below must be run in PowerShell **as Administrator**.

### Option 1 — exclude the source and destination folders

Most effective. Pick the roots you actually hand to `-s` and `-d`; adapt
the paths below.

```powershell
Add-MpPreference -ExclusionPath "C:\ProdutosSG\Branches"
Add-MpPreference -ExclusionPath "E:\Trunk"
```

Verify:

```powershell
Get-MpPreference | Select-Object -ExpandProperty ExclusionPath
```

Undo:

```powershell
Remove-MpPreference -ExclusionPath "C:\ProdutosSG\Branches"
Remove-MpPreference -ExclusionPath "E:\Trunk"
```

### Option 2 — exclude the `exportbranch.exe` process

Narrower than option 1: the binary is skipped by real-time scanning, but
any other process touching the same files (editors, IDEs, compilers)
remains fully protected.

```powershell
Add-MpPreference -ExclusionProcess "exportbranch.exe"
```

Verify:

```powershell
Get-MpPreference | Select-Object -ExpandProperty ExclusionProcess
```

Undo:

```powershell
Remove-MpPreference -ExclusionProcess "exportbranch.exe"
```

Combining both options gives the best throughput. Scheduled scans,
cloud-delivered protection, and SmartScreen on downloads remain active in
every case — only the real-time, per-syscall inspection of the configured
paths/process is bypassed.

## Building

1. [Install Rust](https://www.rust-lang.org/tools/install)
2. Clone the repository:
   ```
   git clone https://github.com/MateusZanchoNeto/exportbranch.git
   ```
3. Build:
   ```
   cargo build --release
   ```
4. Run:
   ```
   ./target/release/exportbranch -s <source> -d <destination>
   ```

### Docker

```
docker build -t exportbranch .
```

## Development

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

See [CLAUDE.md](CLAUDE.md) for repository conventions and the TDD workflow.

## License

[MIT](LICENSE)
