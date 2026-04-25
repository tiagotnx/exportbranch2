use std::process::Command;

fn capture(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?;
    let trimmed = s.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn main() {
    let sha = capture(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let date = capture(&["log", "-1", "--format=%cs"]).unwrap_or_else(|| "unknown".into());

    println!("cargo:rustc-env=GIT_SHA={sha}");
    println!("cargo:rustc-env=GIT_DATE={date}");

    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");
    println!("cargo:rerun-if-changed=build.rs");
}
