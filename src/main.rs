use anyhow::{Result};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::{broadcast, Mutex},
};
use std::collections::HashMap;
use std::sync::Arc;

mod database;
mod auth;

async fn list_chat_rooms(chat_rooms: &Mutex<HashMap<String, broadcast::Sender<(String, std::net::SocketAddr)>>>) -> String {
    let chat_rooms = chat_rooms.lock().await;
    let names: Vec<String> = chat_rooms.keys().cloned().collect();
    names.join(", ")
}

#[tokio::main]
async fn main() -> Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8081").await?;
    let chat_rooms: HashMap<String, broadcast::Sender<(String, std::net::SocketAddr)>> = HashMap::new();
    let chat_rooms = Arc::new(Mutex::new(chat_rooms));

    loop {
        let (mut socket, addr) = listener.accept().await?;
        let chat_rooms = chat_rooms.clone();

        tokio::spawn(async move {
            let (reader, mut writer) = socket.split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();
            writer
                .write_all(b"Initializing server... \n")
                .await
                .expect("Failed to write to socket");            

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

            let available_rooms = list_chat_rooms(&chat_rooms).await;
            writer
                .write_all(format!("Enter chat room name, or create one by typing its name. Available rooms: {}\n", available_rooms).as_bytes())
                .await
                .expect("Failed to write to socket");
            let mut chat_room_name = String::new();
            reader.read_line(&mut chat_room_name).await.expect("Failed to read chat room name");
            let chat_room_name = chat_room_name.trim().to_string();
                

            let (tx, mut rx) = {
                let mut chat_rooms = chat_rooms.lock().await;
                if let Some(sender) = chat_rooms.get(&chat_room_name) {
                    (sender.clone(), sender.subscribe())
                } else {
                    let (tx, rx) = broadcast::channel(100);
                    chat_rooms.insert(chat_room_name.clone(), tx.clone());
                    (tx, rx)
                }
            };

            loop {
                tokio::select! {
                    result = reader.read_line(&mut line) => {
                        match result {
                            Ok(n) if n == 0 => break,
                            Ok(_) => {
                                let msg = format!("{}: {}", username, line);
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
