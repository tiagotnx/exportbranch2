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
