use crate::{Command, Infuser, PlatformInfuser, ResponseAction};
use anyhow::Context;
use async_trait::async_trait;
use regex::Regex;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions},
};

fn format_pipe_name(name: &str) -> String {
    format!(r"\\.\pipe\{name}")
}

fn recreate_server(name: &str, first: bool) -> anyhow::Result<NamedPipeServer> {
    ServerOptions::new()
        .first_pipe_instance(first)
        .create(name)
        .context(format!("Failed to create named pipe {}", name))
}

#[async_trait]
impl Infuser for PlatformInfuser {
    async fn run_input(
        pipe_name: &str,
        _: Option<String>,
        inital_filter: Option<String>,
    ) -> anyhow::Result<()> {
        let mut re = inital_filter
            .as_ref()
            .and_then(|filter| Regex::new(filter).ok());
        let mut filter_string = inital_filter;
        let mut input_lines = BufReader::new(tokio::io::stdin()).lines();

        let pipe_name = format_pipe_name(pipe_name);

        let mut server = recreate_server(&pipe_name, true)?;
        let mut active_server: Option<NamedPipeServer> = None;

        loop {
            tokio::select! {
                Ok(Some(mut x)) = input_lines.next_line() => {
                    println!("{x}");
                    if let Some(serv) = active_server.as_mut() {
                        if let Some(re) = &re {
                            if re.is_match(&x) {
                                x.push('\n');
                                if serv.write_all(x.as_bytes()).await.is_err() {
                                    active_server = None;
                                }
                            }
                        }
                    }
                }
                Ok(_) = server.connect() => {

                    // don't update active_server yet since this may be a one-off command
                    let mut inner = server;
                    // recreate the server to allow another connection
                    server = recreate_server(&pipe_name, false)?;

                    let mut buf = [0; 1024];
                    if let Ok(x) = inner.read(&mut buf).await {
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
                                        let _ = inner.write_all(filter).await;
                                    }
                                    Command::GetCurrentTty => {
                                        let _ = inner.write_all(b"<not supported on Windows>").await;
                                    }
                                    Command::Listen(_) => {
                                        // if this was a listen process, update the active_server
                                        active_server = Some(inner);
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
        let pipe_name = format_pipe_name(sock);
        let mut client = ClientOptions::new()
            .open(pipe_name)
            .context("Failed to connect to server pipe")?;

        // fire off a listen command to register with the server
        let syn = Command::Listen("".to_string());
        let json = serde_json::to_vec(&syn)?;
        client.write_all(&json).await?;

        // read from the named pipe and write to stdout
        let mut buf = [0; 2048];
        loop {
            match client.read(&mut buf).await {
                Ok(x) if x > 0 => {
                    let _ = tokio::io::stdout().write_all(&buf[..x]).await;
                }
                _ => {
                    break;
                }
            }
        }
        Ok(())
    }

    async fn run_utility_command(
        sock: &str,
        command: Command,
        response: ResponseAction,
    ) -> anyhow::Result<()> {
        let pipe_name = format_pipe_name(sock);
        let mut client = ClientOptions::new()
            .open(pipe_name)
            .context("Failed to connect to server pipe")?;
        let json = serde_json::to_vec(&command)?;
        let _ = client.write(&json).await;

        // Perform action if necessary
        match response {
            ResponseAction::WaitAndPrint => {
                let mut response = String::new();
                client.read_to_string(&mut response).await?;
                println!("{}", response);
            }
            ResponseAction::Oneshot => {}
        }
        Ok(())
    }
}
