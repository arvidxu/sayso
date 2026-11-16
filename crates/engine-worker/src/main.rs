use std::io::stdin;
use std::process::ExitCode;

fn main() -> ExitCode {
    let served = engine_worker::reply_output()
        .map_err(|err| format!("replies cannot be sent: {err}"))
        .and_then(|output| {
            engine_worker::serve(&asr_local::built_in_backends(), stdin().lock(), output)
                .map_err(|err| err.to_string())
        });
    match served {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("recognition worker: {err}");
            ExitCode::FAILURE
        }
    }
}
