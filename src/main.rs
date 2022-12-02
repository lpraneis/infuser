use clap::Parser;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
};

static LONG_ABOUT: &str = "
Meant to be a replacement for the following:

tee:
long_running_command | tee >(rg \"Important Line\" > /dev/pts/X)

infuser:
long_running_command | infuser run -f \"Important Line\" /dev/pts/X

The filter being used can be updated during the execution from a different terminal:
infuser update \"New.*Thing\"
";

/// clear a terminal screen
const CLEAR_SCREEN: &str = "\x1b\x63";

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
        tty: Option<String>,
        /// initial filter
        #[clap(short, long)]
        filter: Option<String>,
    },
    /// update running infuser
    Update {
        /// updated filter
        new_filter: String,
    },
    /// clear running filter
    Clear,
    /// get currently running filter
    GetFilter,
    /// register current tty for output. Replaces previous tty, if any
    Listen,
}

#[derive(Debug, Serialize, Deserialize)]
enum Command {
    NewFilter(Option<String>),
    GetCurrentFilter,
    Listen(String),
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.mode {
        OperationMode::Run { tty, filter } => run_input(&args.sock_name, tty, filter).await,
        OperationMode::Update { new_filter } => update_filter(&args.sock_name, new_filter).await,
        OperationMode::Clear => clear_filter(&args.sock_name).await,
        OperationMode::GetFilter => get_filter(&args.sock_name).await,
        OperationMode::Listen => listen(&args.sock_name).await,
    }
}

async fn update_filter(sock: &str, filter: String) -> anyhow::Result<()> {
    let cmd = Command::NewFilter(Some(filter));
    run_utility_command(sock, cmd).await.map(|_| ())
}
async fn clear_filter(sock: &str) -> anyhow::Result<()> {
    let cmd = Command::NewFilter(None);
    run_utility_command(sock, cmd).await.map(|_| ())
}
async fn get_filter(sock: &str) -> anyhow::Result<()> {
    let cmd = Command::GetCurrentFilter;
    let mut sock = run_utility_command(sock, cmd).await?;
    let mut response = String::new();
    sock.read_to_string(&mut response).await?;
    println!("{}", response);
    Ok(())
}

async fn listen(sock: &str) -> anyhow::Result<()> {
    let tty = nix::unistd::ttyname(0)?
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("cannot convert tty to string"))?
        .to_string();
    let cmd = Command::Listen(tty);
    run_utility_command(sock, cmd).await.map(|_| ())
}

async fn run_utility_command(sock: &str, command: Command) -> anyhow::Result<UnixStream> {
    let tx_path = Path::new("/tmp").join(sock);
    let mut sock = UnixStream::connect(&tx_path).await?;
    let json = serde_json::to_vec(&command)?;
    let _ = sock.write(&json).await;
    Ok(sock)
}

async fn open_and_clear_tty(tty: &str) -> anyhow::Result<File> {
    let mut f = tokio::fs::OpenOptions::new()
        .write(true)
        .append(true)
        .open(tty)
        .await?;
    f.write_all(CLEAR_SCREEN.as_bytes()).await?;
    Ok(f)
}

async fn run_input(
    sock: &str,
    initial_tty: Option<String>,
    inital_filter: Option<String>,
) -> anyhow::Result<()> {
    let mut re = inital_filter
        .as_ref()
        .and_then(|filter| Regex::new(filter).ok());
    let mut filter_string = inital_filter;
    let mut input_lines = BufReader::new(tokio::io::stdin()).lines();

    let tx_path = Path::new("/tmp").join(sock);
    let _ = std::fs::remove_file(&tx_path);

    let sock = UnixListener::bind(&tx_path)?;
    let mut tty: Option<File> = None;

    if let Some(tty_name) = initial_tty {
        tty = Some(open_and_clear_tty(&tty_name).await?);
    }

    loop {
        tokio::select! {
            Ok(Some(mut x)) = input_lines.next_line() => {
                println!("{x}");
                if let Some(client) = tty.as_mut() {
                    if let Some(re) = &re {
                        if re.is_match(&x) {
                            x.push('\n');
                            if client.write_all(x.as_bytes()).await.is_err() {
                                tty = None;
                            }
                        }
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
                                        re = Some(new_regex);
                                        filter_string = Some(f);
                                    }
                                }
                                Command::NewFilter(None) => {
                                    re = None;
                                    filter_string = None;
                                }
                                Command::GetCurrentFilter => {
                                    let filter = filter_string
                                        .as_ref()
                                        .map(|x| x.as_bytes())
                                        .unwrap_or_else(|| b"<no current filter>");
                                    let _ = client.write_all(filter).await;
                                }
                                Command::Listen(tty_name) => {
                                    if let Ok(new_tty) = open_and_clear_tty(&tty_name).await {
                                        tty = Some(new_tty);
                                    }
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
