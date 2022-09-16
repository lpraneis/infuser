use clap::Parser;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
};

static LONG_ABOUT: &str = "
Meant to be a replacement for the following:

tee:
long_running_command | tee >(rg \"Important Line\" > /dev/pts/X)

infuser:
long_running_command | infuser run /dev/pts/X \"Important Line\" 

The filter being used can be updated during the execution from a different terminal:
infuser update \"New.*Thing\"
";

/// Filters your tee
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = LONG_ABOUT)]
struct Args {
    /// Name of unix domain socket to be created in /tmp for IPC
    #[clap(long, value_parser, default_value = "infuser.sock")]
    sock_name: String,

    /// process operation mode
    #[clap(subcommand)]
    mode: OperationMode,
}

#[derive(clap::Parser, Debug, PartialEq)]
enum OperationMode {
    /// run and get input
    Run {
        /// TTY to send filtered lines to
        tty: String,
        /// initial filter
        inital_filter: Option<String>,
    },
    /// update running infuser
    Update {
        /// updated filter
        new_filter: String,
    },
    /// clear running filter
    Clear,
}

#[derive(Debug, Serialize, Deserialize)]
enum Command {
    NewFilter(Option<String>),
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.mode {
        OperationMode::Run { tty, inital_filter } => {
            run_input(&args.sock_name, &tty, inital_filter).await
        }
        OperationMode::Update { new_filter } => {
            run_utility(&args.sock_name, Some(new_filter)).await
        }
        OperationMode::Clear => run_utility(&args.sock_name, None).await,
    }
}

async fn run_utility(sock: &str, filter: Option<String>) -> anyhow::Result<()> {
    let tx_path = Path::new("/tmp").join(sock);
    let mut sock = UnixStream::connect(&tx_path).await?;
    let command = Command::NewFilter(filter);
    let json = serde_json::to_vec(&command)?;
    let _ = sock.write(&json).await;
    Ok(())
}
async fn run_input(sock: &str, tty: &str, inital_filter: Option<String>) -> anyhow::Result<()> {
    let mut re = inital_filter.and_then(|filter| Regex::new(&filter).ok());
    let mut input_lines = BufReader::new(tokio::io::stdin()).lines();

    let tx_path = Path::new("/tmp").join(sock);
    let _ = std::fs::remove_file(&tx_path);

    let sock = UnixListener::bind(&tx_path)?;
    let mut tty = tokio::fs::OpenOptions::new()
        .write(true)
        .append(true)
        .open(tty)
        .await?;

    loop {
        tokio::select! {
            Ok(Some(mut x)) = input_lines.next_line() => {
                println!("{x}");
                if let Some(re) = &re {
                    if re.is_match(&x) {
                        x.push('\n');
                        let _ = tty.write(x.as_bytes()).await;
                    }
                }
            }
            Ok((mut client,_)) = sock.accept() => {
                let mut buf = [0; 1024];
                if let Ok(x) = client.read(&mut buf).await {
                    if x > 0 {
                        let s : serde_json::Result<Command> = serde_json::from_slice(&buf[..x]);
                        match s {
                            Ok(cmd) => match cmd {
                                Command::NewFilter(Some(f)) => {
                                    if let Ok(new_regex) = Regex::new(&f) {
                                        #[cfg(debug_assertions)]
                                        println!("Updating regex to {f:?}");
                                        re = Some(new_regex);
                                    }
                                }
                                Command::NewFilter(None) => {
                                        #[cfg(debug_assertions)]
                                        println!("Clearing regex");
                                        re = None;
                                }
                            },
                            Err(e) => {
                                println!("Invalid command {e:?}");
                            }
                        }
                    }
                }
            }
            else => {
                break;
            }
        }
    }
    Ok(())
}
