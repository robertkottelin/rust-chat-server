use anyhow::{Result};
use sqlite::{Connection};
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::{broadcast, Mutex},
};

lazy_static::lazy_static! {
    static ref DB_MUTEX: Arc<Mutex<Connection>> = {
        let conn = Connection::open("users.db").expect("Failed to open users.db");
        Arc::new(Mutex::new(conn))
    };
}

pub async fn database(username: &str, password: &str) -> Result<()> {
    let query = format!(
        "INSERT INTO users (username, password) VALUES ('{}', '{}');",
        username, password
    );
    let conn = DB_MUTEX.lock().await;
    conn.execute(&query)?;
    Ok(())
}

pub async fn user_auth(username: &str, password: &str) -> Result<bool> {
    let query = format!(
        "SELECT COUNT(*) FROM users WHERE username = '{}' AND password = '{}';",
        username, password
    );

    let conn = DB_MUTEX.lock().await;
    let mut count = 0;
    conn.iterate(&query, |row| {
        let count_str = row[0].1.unwrap();
        count = count_str.parse::<i64>().unwrap() as usize;
        true
    })?;

    Ok(count > 0)
}

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

            let mut authenticated = false;
            while !authenticated {
                // Check if the user is authenticated
                if user_auth(&username, &password).await.unwrap_or(false) {
                    authenticated = true;
                } else {
                    // If the user is not authenticated, store their credentials and add them to the database
                    database(&username, &password)
                        .await
                        .expect("Failed to add user to the database");
                    writer
                        .write_all(b"\nUsername-password combination not found. Creating user.\n")
                        .await
                        .expect("Failed to write to socket");
                }
            }
            writer
                .write_all(format!("\nLog in successful. Welcome {}! \n", username).as_bytes())
                .await
                .expect("Failed to write to socket");

            loop {
                tokio::select! {
                    result = reader.read_line(&mut line) => {
                        if result.expect("Failed to read line") == 0 {
                            break;
                        }

                        let msg = format!("{}: {}", username, line);
                        // Send the message to all subscribers
                        tx.send((msg.clone(), addr)).expect("Failed to send message");
                        line.clear();
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
