use sqlite;
use std::sync::Mutex;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::broadcast,
};

lazy_static::lazy_static! {
    static ref DB_MUTEX: Mutex<()> = Mutex::new(());
}

pub fn database(username: &str, password: &str) {
    let _lock = DB_MUTEX.lock().unwrap(); // acquire the lock

    let connection = sqlite::open("users.db").unwrap();
    let query = format!(
        "INSERT INTO users (username, password) VALUES ('{}', '{}');",
        username, password
    );
    connection.execute(&query).unwrap();
}

pub fn user_auth(username: &str, password: &str) -> bool {
    let _lock = DB_MUTEX.lock().unwrap(); // acquire the lock

    let connection = sqlite::open("users.db").unwrap();
    let query = format!(
        "SELECT COUNT(*) FROM users WHERE username = '{}' AND password = '{}';",
        username, password
    );

    let mut result = 0;
    connection
        .iterate(&query, |row| {
            let count_str = row[0].1.unwrap();
            result = count_str.parse::<i64>().unwrap() as usize;
            true
        })
        .unwrap();

    result > 0
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
    let (tx, _rx) = broadcast::channel(100);
    // database();

    loop {
        let (mut socket, addr) = listener.accept().await.unwrap();
        let tx = tx.clone();
        let mut rx = tx.subscribe();

        tokio::spawn(async move {
            let (reader, mut writer) = socket.split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();
            writer
                .write_all(b"Initializing server... \n")
                .await
                .unwrap();

            // Prompt for username and password
            writer.write_all(b"Username: \n").await.unwrap();
            let mut username = String::new();
            reader.read_line(&mut username).await.unwrap();
            let username = username.trim().to_string();

            writer.write_all(b"\nPassword: \n").await.unwrap();
            let mut password = String::new();
            reader.read_line(&mut password).await.unwrap();
            let password = password.trim().to_string();

            let mut authenticated = false;
            while !authenticated {
                // Check if the user is authenticated
                if user_auth(&username, &password) {
                    authenticated = true;
                } else {
                    // If the user is not authenticated, store their credentials and add them to the database
                    database(&username, &password);
                    writer
                        .write_all(b"\nUsername-password combination not found. Creating user.\n")
                        .await
                        .unwrap();
                }
            }

            writer
                .write_all(format!("\nLog in successful. Welcome {}! \n", username).as_bytes())
                .await
                .unwrap();

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