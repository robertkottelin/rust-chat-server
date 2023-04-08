use anyhow::{Result};
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::{broadcast, Mutex},
};

mod database;
mod auth;

#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8080").await?;
    let (tx, _rx) = broadcast::channel(100);

    loop {
        let (mut socket, addr) = listener.accept().await?;
        let tx = tx.clone();
        let mut rx = tx.subscribe();

        tokio::spawn(async move {
            let (reader, mut writer) = socket.split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();
            writer
                .write_all(b"Initializing server... \n")
                .await
                .expect("Failed to write to socket");

            // Prompt for username and password
            writer.write_all(b"Username: \n").await.expect("Failed to write to socket");
            let mut username = String::new();
            reader.read_line(&mut username).await.expect("Failed to read username");
            let username = username.trim().to_string();

            writer.write_all(b"\nPassword: \n").await.expect("Failed to write to socket");
            let mut password = String::new();
            reader.read_line(&mut password).await.expect("Failed to read password");
            let password = password.trim().to_string();

            auth::authenticate_user(&username, &password)
            .await
            .expect("Failed to authenticate user");
        
            writer
                .write_all(format!("\nLog in successful. Welcome {}! \n", username).as_bytes())
                .await
                .expect("Failed to write to socket");

            loop {
                tokio::select! {
                    result = reader.read_line(&mut line) => {
                        match result {
                            Ok(n) if n == 0 => break,
                            Ok(_) => {
                                let msg = format!("{}: {}", username, line);
                                // Send the message to all subscribers
                                tx.send((msg.clone(), addr)).expect("Failed to send message");
                                line.clear();
                            }
                            Err(e) => {
                                eprintln!("Failed to read line, user closed the connection: {}", e);
                                break;
                            }
                        }
                    }
                    result = rx.recv() => {
                        match result {
                            Ok((msg, other_addr)) => {
                                // Write the message to the client if it's from a different address
                                if addr != other_addr {
                                    writer.write_all(msg.as_bytes()).await.expect("Failed to write to socket");
                                    writer.flush().await.expect("Failed to flush writer");
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
        });
    }
}
