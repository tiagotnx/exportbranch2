// These end-to-end tests place the temporary `src/` and `dst/` side by side
// under a single `TempDir`. On Windows, `destination_path` (see `lib.rs`)
// mirrors the source path (minus its drive) under the destination root, so
// with `src` and `dst` both under `C:\Users\…\TempDir\` the effective
// destination becomes `dst\Users\…\TempDir\src\…` — the shallow
// `dst.join("file.prg")` assertions below do not hold there. The
// `lib::tests` module unit-tests the Windows branch directly.
#![cfg(not(windows))]

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn write_file(path: &Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn run_export(src: &Path, dst: &Path, extra: &[&str]) {
    let mut args: Vec<String> = vec![
        "exportbranch".into(),
        "-s".into(),
        src.to_string_lossy().into_owned(),
        "-d".into(),
        dst.to_string_lossy().into_owned(),
    ];
    args.extend(extra.iter().map(|s| (*s).to_string()));
    exportbranch::run(args).expect("run should succeed");
}

#[test]
fn exporta_arquivo_prg_default() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    let dst = tmp.path().join("dst");
    fs::create_dir_all(&src).unwrap();

    write_file(&src.join("hello.prg"), b"PRG content\n");

    run_export(&src, &dst, &[]);

    assert!(
        dst.join("hello.prg").exists(),
        "*.prg deveria ter sido exportado"
    );
}

#[test]
fn nao_exporta_arquivo_fora_dos_filtros() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    let dst = tmp.path().join("dst");
    fs::create_dir_all(&src).unwrap();

    write_file(&src.join("ignored.txt"), b"texto\n");

    run_export(&src, &dst, &[]);

    assert!(
        !dst.join("ignored.txt").exists(),
        "*.txt não está nos filtros default"
    );
}

#[test]
fn exporta_recursivamente() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    let dst = tmp.path().join("dst");
    fs::create_dir_all(&src).unwrap();

    write_file(&src.join("sub").join("nested.prg"), b"nested\n");

    run_export(&src, &dst, &[]);

    assert!(dst.join("sub").join("nested.prg").exists());
}

#[test]
fn disregarded_directories_nao_sao_exportadas() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    let dst = tmp.path().join("dst");
    fs::create_dir_all(&src).unwrap();

    write_file(&src.join("bin").join("foo.prg"), b"bin prg\n");
    write_file(&src.join("lib").join("bar.prg"), b"lib prg\n");
    write_file(
        &src.join("programas_externos")
            .join("conversoes")
            .join("baz.prg"),
        b"conv prg\n",
    );
    write_file(&src.join("app").join("ok.prg"), b"ok\n");

    run_export(&src, &dst, &[]);

    assert!(
        !dst.join("bin").join("foo.prg").exists(),
        "bin/ é disregarded"
    );
    assert!(
        !dst.join("lib").join("bar.prg").exists(),
        "lib/ é disregarded"
    );
    assert!(
        !dst.join("programas_externos")
            .join("conversoes")
            .join("baz.prg")
            .exists(),
        "programas_externos/conversoes é disregarded"
    );
    assert!(
        dst.join("app").join("ok.prg").exists(),
        "diretório normal deve ser exportado"
    );
}

#[test]
fn export_paralelo_processa_todos_arquivos() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    let dst = tmp.path().join("dst");
    fs::create_dir_all(&src).unwrap();

    // Várias árvores e arquivos para forçar paralelismo entre subdirs.
    let mut expected: HashSet<PathBuf> = HashSet::new();
    for i in 0..8 {
        for j in 0..6 {
            let rel = PathBuf::from(format!("dir{i}/sub{j}/file.prg"));
            write_file(&src.join(&rel), format!("content {i}-{j}\n").as_bytes());
            expected.insert(rel);
        }
    }

    run_export(&src, &dst, &[]);

    let mut actual: HashSet<PathBuf> = HashSet::new();
    for entry in walkdir(&dst) {
        let rel = entry.strip_prefix(&dst).unwrap().to_path_buf();
        if rel.extension().and_then(|s| s.to_str()) == Some("prg") {
            actual.insert(rel);
        }
    }

    assert_eq!(actual, expected);
}

fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                out.extend(walkdir(&p));
            } else {
                out.push(p);
            }
        }
    }
    out
}

#[test]
fn copia_arquivos_only_copy_sem_converter() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    let dst = tmp.path().join("dst");
    fs::create_dir_all(&src).unwrap();

    let content = b"\xe9header"; // byte CP850 que seria convertido se fosse converted
    write_file(&src.join("foo.h"), content);

    run_export(&src, &dst, &[]);

    let copied = fs::read(dst.join("foo.h")).unwrap();
    assert_eq!(
        copied, content,
        "*.h está em DEFAULT_ONLY_COPY_FILES → deve ser copiado byte-a-byte"
    );
}
