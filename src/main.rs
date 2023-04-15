use async_trait::async_trait;
use clap::Parser;
use serde::{Deserialize, Serialize};

#[cfg(unix)]
mod unix;

#[cfg(windows)]
mod windows;

static LONG_ABOUT: &str = "
Meant to be a replacement for the following:

tee:
long_running_command | tee >(rg \"Important Line\" > /dev/pts/X)

infuser:
long_running_command | infuser run -f \"Important Line\" /dev/pts/X

The filter being used can be updated during the execution from a different terminal:
infuser update \"New.*Thing\"

On Windows, this only works in cmd.exe ( for now ) since Powershell pipes attempt to pipe
all of the previous command's output before passing it on to the consumer process.

More work is required to make this work like Powershell's `Tee-Object`
";

/// Filters your tee
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = LONG_ABOUT)]
struct Args {
    /// Name of communication pipe
    /// On Unix, this is a Unix Domain Socket in /tmp
    /// On Windows, this is the name of a named pipe
    #[clap(long, value_parser, default_value = "infuser.pipe")]
    sock_name: String,

    /// process operation mode
    #[clap(subcommand)]
    mode: OperationMode,
}

#[derive(clap::Parser, Debug, PartialEq)]
enum OperationMode {
    /// Clear running filter
    Clear,
    /// Get currently running filter
    GetFilter,
    /// Get currently registered tty or console
    GetTty,
    /// Register current console for output; replaces previous tty or console, if any.
    /// This is required on Windows since there aren't ttys
    Listen,
    /// Run and get input
    Run {
        /// TTY to send filtered lines to, makes no difference on Windows
        tty: Option<String>,
        /// Initial filter
        #[clap(short, long)]
        filter: Option<String>,
    },
    /// Update running infuser filter
    Update {
        /// updated filter
        new_filter: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
enum Command {
    NewFilter(Option<String>),
    GetCurrentFilter,
    GetCurrentTty,
    Listen(String),
}

#[derive(Debug, Copy, Clone)]
enum ResponseAction {
    WaitAndPrint,
    Oneshot,
}

struct PlatformInfuser;

#[async_trait]
trait Infuser {
    /// Run in "input-mode", essentially tee+grep
    async fn run_input(
        sock: &str,
        initial_tty: Option<String>,
        inital_filter: Option<String>,
    ) -> anyhow::Result<()>;

    /// Run in listen-mode, gets filtered output from server
    async fn run_listen(sock: &str) -> anyhow::Result<()>;

    /// Run a utility command
    async fn run_utility_command(
        sock: &str,
        command: Command,
        response: ResponseAction,
    ) -> anyhow::Result<()>;

    /// Clear the current filter
    async fn clear_filter(sock: &str) -> anyhow::Result<()> {
        let cmd = Command::NewFilter(None);
        Self::run_utility_command(sock, cmd, ResponseAction::Oneshot).await
    }

    /// Update the current filter, if any
    async fn update_filter(pipe: &str, filter: String) -> anyhow::Result<()> {
        let cmd = Command::NewFilter(Some(filter));
        Self::run_utility_command(pipe, cmd, ResponseAction::Oneshot).await
    }

    /// Print the current filter, if any
    async fn print_filter(pipe: &str) -> anyhow::Result<()> {
        let cmd = Command::GetCurrentFilter;
        Self::run_utility_command(pipe, cmd, ResponseAction::WaitAndPrint).await
    }

    /// Get the current TTY / Console listening, if any
    async fn get_tty(pipe: &str) -> anyhow::Result<()> {
        let cmd = Command::GetCurrentTty;
        Self::run_utility_command(pipe, cmd, ResponseAction::WaitAndPrint).await
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.mode {
        OperationMode::Run { tty, filter } => {
            PlatformInfuser::run_input(&args.sock_name, tty, filter).await
        }
        OperationMode::Update { new_filter } => {
            PlatformInfuser::update_filter(args.sock_name.as_ref(), new_filter).await
        }
        OperationMode::Clear => PlatformInfuser::clear_filter(args.sock_name.as_ref()).await,
        OperationMode::GetFilter => PlatformInfuser::print_filter(&args.sock_name).await,
        OperationMode::Listen => PlatformInfuser::run_listen(args.sock_name.as_ref()).await,
        OperationMode::GetTty => PlatformInfuser::get_tty(&args.sock_name).await,
    }
}
