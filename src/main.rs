use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener},
    sync::broadcast,
};
//use std::{collections::HashMap, net::SocketAddr};


#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
    let (tx, _rx) = broadcast::channel(100);


    loop {
        let (mut socket, addr) = listener.accept().await.unwrap();
        let tx = tx.clone();
        let mut rx = tx.subscribe();


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

                        let msg = format!("{}", line);
                        // Send the message to all subscribers
                        tx.send((msg.clone(), addr)).unwrap();
                        line.clear();
                    }
                    result = rx.recv() => {
                        let (msg, _other_addr) = result.unwrap();
                        writer.write_all(msg.as_bytes()).await.unwrap();
                        writer.flush().await.unwrap();

                        // Write the message to the client if it's from a different address
                        // if addr != other_addr {
                        //     writer.write_all(msg.as_bytes()).await.unwrap();
                        //     writer.flush().await.unwrap();
                        // }
                    }
                }
            }
        });
    }
}
