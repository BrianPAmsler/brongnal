use anyhow::Result;
use client::{listen, message, register, MemoryClient};
use nom::bytes::complete::tag;
use nom::character::complete::{alphanumeric1, multispace1};
use nom::sequence::preceded;
use nom::IResult;
use server::proto::service::brongnal_client::BrongnalClient;
use std::io::stdin;
use std::io::BufRead;
use std::io::BufReader;
use std::sync::Arc;
use std::{env, thread};
use tokio::sync::{mpsc, Mutex};

#[derive(Debug)]
struct Command {
    to: String,
    msg: String,
}

fn parse_command(input: &str) -> IResult<&str, Command> {
    let (input, _) = preceded(tag("message"), multispace1)(input)?;
    let (input, name) = alphanumeric1(input)?;
    let (message, _spaces) = multispace1(input)?;
    Ok((
        &"",
        Command {
            to: name.to_owned(),
            msg: message.to_owned(),
        },
    ))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = env::args().collect::<Vec<String>>();
    let name = args.get(1).unwrap().to_owned();
    let addr: String = args
        .get(2)
        .map(|addr| addr.to_owned())
        .unwrap_or("https://signal.brongan.com:443".to_owned());

    eprintln!("Registering {name} at {addr}");

    let mut stub = BrongnalClient::connect(addr).await?;
    let client = Arc::new(Mutex::new(MemoryClient::new()));

    register(&mut stub, client.clone(), name.clone()).await?;

    println!("message NAME MESSAGE");

    let (tx, mut rx) = mpsc::channel(100);
    let (cli_tx, mut cli_rx) = mpsc::unbounded_channel::<Command>();

    thread::spawn(move || {
        let mut lines = BufReader::new(stdin()).lines();
        while let Some(line) = lines.next() {
            let line = line.unwrap();
            match parse_command(&line).map_err(|e| e.to_owned()) {
                Ok((_, command)) => {
                    if let Err(_) = cli_tx.send(command) {
                        return;
                    }
                }
                Err(e) => eprintln!("Invalid Command: {e}"),
            }
        }
    });

    {
        let stub = stub.clone();
        let client = client.clone();
        tokio::spawn(async move { listen(stub, client, name, tx).await });
    }

    loop {
        tokio::select! {
        command = cli_rx.recv() => {
            match command {
                Some(command) => {
                    message(&mut stub, client.clone(), &command.to, &command.msg)
                        .await
                        .unwrap();
                    },
                None => {
                    eprintln!("Closing...");
                    return Ok(());
                }
            }

        },
        msg = rx.recv() => {
            match msg {
                Some(msg) => {
                    println!("{}", String::from_utf8(msg).unwrap());
                },
                None =>  {
                    eprintln!("Server terminated connection."); return Ok(())
                },
            }
        }
        }
    }
}