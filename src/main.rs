use std::collections::HashMap;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::broadcast,
};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
    let (tx, _rx) = broadcast::channel(100);

    loop {
        let (mut socket, addr) = listener.accept().await.unwrap();
        let tx = tx.clone();
        let mut rx = tx.subscribe();

        let mut users: HashMap<String, String> = HashMap::new();

        tokio::spawn(async move {
            let (reader, mut writer) = socket.split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            writer
                .write_all(b"Initializing server... \n")
                .await
                .unwrap();

            // Prompt for username and password
            writer.write_all(b"Username: ").await.unwrap();
            let mut username = String::new();
            reader.read_line(&mut username).await.unwrap();
            let username = username.trim().to_string();

            writer.write_all(b"Password: ").await.unwrap();
            let mut password = String::new();
            reader.read_line(&mut password).await.unwrap();
            let password = password.trim().to_string();

            // Store the username and password in the hashmap
            users.insert(username.clone(), password.clone());

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
        });
    }
}
