use std::sync::{Arc, Mutex};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::broadcast,
};
use std::{collections::HashMap, net::SocketAddr};

async fn read_username(socket: &mut TcpStream) -> Result<String, std::io::Error> {
    let mut reader = BufReader::new(socket);
    let mut username = String::new();
    reader.read_line(&mut username).await?;
    Ok(username.trim().to_owned())
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
    let (tx, _rx) = broadcast::channel(100);

    let users = Arc::new(Mutex::new(HashMap::<String, SocketAddr>::new()));

    loop {
        let (mut socket, addr) = listener.accept().await.unwrap();
        let tx = tx.clone();
        let mut rx = tx.subscribe();

        // Send a prompt to the client for their username
        socket.write_all(b"Enter your username: ").await.unwrap();
        socket.flush().await.unwrap(); // Flush the output

        let username = match read_username(&mut socket).await {
            Ok(username) => username,
            Err(e) => {
                eprintln!("Error reading username: {}", e);
                continue;
            }
        };

        let mut users_guard = users.lock().unwrap();
        if users_guard.contains_key(&username) {
            // You can choose how to handle duplicate usernames (e.g., disconnect or generate a unique username)
            eprintln!("Username '{}' is already in use.", username);
            continue;
        }

        let user_addr = addr;
        users_guard.insert(username.clone(), user_addr);
        drop(users_guard);

        let users = Arc::clone(&users);

        tokio::spawn(async move {
            let (reader, mut writer) = socket.split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            loop {
                tokio::select! {
                    result = reader.read_line(&mut line) => {
                        if result.unwrap() == 0 {
                            break;
                        }

                        let msg = format!("{}: {}", username, line);
                        // Send the message to all subscribers
                        tx.send((msg.clone(), addr)).unwrap();
                        line.clear();
                    }
                    result = rx.recv() => {
                        let (msg, other_addr) = result.unwrap();

                        // Write the message to the client if it's from a different address
                        if addr != other_addr {
                            writer.write_all(msg.as_bytes()).await.unwrap();
                            writer.flush().await.unwrap();
                        }
                    }
                }
            }

            // Remove the user after they have disconnected
            let mut users_guard = users.lock().unwrap();
            users_guard.remove(&username);
        });
    }
}
