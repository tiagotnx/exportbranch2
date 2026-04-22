use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    match exportbranch::run(env::args().collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(u8::try_from(err.exit_code()).unwrap_or(1))
        }
    }
}
