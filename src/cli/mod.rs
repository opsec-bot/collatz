//! The `collatz` command-line tool.

/// Like `println!`, but exits quietly when stdout is closed (for example when
/// piped into `head`) instead of panicking.
macro_rules! out {
    () => {
        $crate::cli::emit(format_args!(""))
    };
    ($($arg:tt)*) => {
        $crate::cli::emit(format_args!($($arg)*))
    };
}

pub mod args;
pub mod chart;
pub mod commands;
pub mod format;

/// How a command failed.
#[derive(Debug)]
pub enum CliError {
    /// Bad arguments; the usage text is worth showing.
    Usage(String),
    /// The command ran and found a problem.
    Failure(String),
}

impl From<String> for CliError {
    fn from(message: String) -> Self {
        CliError::Usage(message)
    }
}

pub type CliResult = Result<(), CliError>;

/// Writes one line to stdout. See [`out!`].
pub fn emit(line: std::fmt::Arguments) {
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();
    if let Err(err) = writeln!(stdout, "{line}") {
        if err.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        panic!("failed to write to stdout: {err}");
    }
}
