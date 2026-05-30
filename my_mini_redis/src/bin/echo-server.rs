use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6142").await.unwrap();

    loop {
        let (mut socket, _) = listener.accept().await.unwrap();

        tokio::spawn(async move {
            let mut buf = vec![0; 128];

            loop {
                let n = socket.read(&mut buf).await.unwrap();

                if n == 0 {
                    break;
                }

                socket.write_all(&buf[..n]).await.unwrap();
            }
        });
    }
}