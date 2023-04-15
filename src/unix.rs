use crate::{Command, Infuser, PlatformInfuser, ResponseAction};
use async_trait::async_trait;
use regex::Regex;
use std::path::Path;
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
};

/// clear a terminal screen
const CLEAR_SCREEN: &str = "\x1b\x63";

/// Wrapper over a tty handle and path
struct Tty {
    inner: File,
    path: String,
}

#[async_trait]
impl Infuser for PlatformInfuser {
    async fn run_input(
        pipe: &str,
        initial_tty: Option<String>,
        inital_filter: Option<String>,
    ) -> anyhow::Result<()> {
        let mut re = inital_filter
            .as_ref()
            .and_then(|filter| Regex::new(filter).ok());
        let mut filter_string = inital_filter;
        let mut input_lines = BufReader::new(tokio::io::stdin()).lines();

        let tx_path = Path::new("/tmp").join(pipe);
        let _ = std::fs::remove_file(&tx_path);

        let sock = UnixListener::bind(&tx_path)?;
        let mut tty: Option<Tty> = None;

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
                                if client.inner.write_all(x.as_bytes()).await.is_err() {
                                    // clear the tty if we had a write error
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
                                    Command::GetCurrentTty => {
                                        let tty_name = tty
                                            .as_ref()
                                            .map(|x| x.path.as_bytes())
                                            .unwrap_or_else(|| b"<no current tty>");
                                        let _ = client.write_all(tty_name).await;
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

    async fn run_listen(sock: &str) -> anyhow::Result<()> {
        let tty = nix::unistd::ttyname(0)?
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("cannot convert tty to string"))?
            .to_string();
        let cmd = Command::Listen(tty);
        Self::run_utility_command(sock, cmd, ResponseAction::Oneshot).await
    }

    async fn run_utility_command(
        sock: &str,
        command: Command,
        response: ResponseAction,
    ) -> anyhow::Result<()> {
        // Write to the sock
        let tx_path = Path::new("/tmp").join(sock);
        let mut sock = UnixStream::connect(&tx_path).await?;
        let json = serde_json::to_vec(&command)?;
        let _ = sock.write(&json).await;

        // Perform action if necessary
        match response {
            ResponseAction::WaitAndPrint => {
                let mut response = String::new();
                sock.read_to_string(&mut response).await?;
                println!("{}", response);
            }
            ResponseAction::Oneshot => {}
        }
        Ok(())
    }
}

async fn open_and_clear_tty(tty: &str) -> anyhow::Result<Tty> {
    let mut f = tokio::fs::OpenOptions::new()
        .write(true)
        .append(true)
        .open(tty)
        .await?;
    f.write_all(CLEAR_SCREEN.as_bytes()).await?;

    Ok(Tty {
        inner: f,
        path: tty.to_string(),
    })
}
