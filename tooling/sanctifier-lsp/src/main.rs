use std::process::ExitCode;

const USAGE: &str = "\
sanctifier-lsp — Language Server Protocol server for Sanctifier

USAGE:
    sanctifier-lsp --stdio

OPTIONS:
    --stdio      Communicate over stdin/stdout (the only supported transport)
    -h, --help   Print this message
    -V, --version Print version
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        // `--stdio` is required rather than assumed: editors always pass it,
        // and a bare invocation is almost always a human who will otherwise
        // sit in front of a silent process waiting on stdin.
        Some("--stdio") => match sanctifier_lsp::run_stdio() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("sanctifier-lsp: {e}");
                ExitCode::FAILURE
            }
        },
        Some("-h") | Some("--help") => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("-V") | Some("--version") => {
            println!("sanctifier-lsp {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprint!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}
