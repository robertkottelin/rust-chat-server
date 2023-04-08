use anyhow::{Result};
use tokio::io::{AsyncBufReadExt, BufReader};

pub async fn get_input(prompt: &str) -> Result<String> {
    let mut reader = BufReader::new(tokio::io::stdin());
    let mut input = String::new();

    print!("{}", prompt);
    reader.read_line(&mut input).await?;
    Ok(input.trim().to_string())
}
