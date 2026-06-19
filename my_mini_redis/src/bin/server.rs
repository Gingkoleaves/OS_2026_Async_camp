use mini_redis::Connection;
use my_mini_redis::{dispatch, new_db, ShardedDb};
use tokio::net::{TcpListener, TcpStream};

/// Process a single client connection
async fn process(socket: TcpStream, db: ShardedDb) {
    let mut connection = Connection::new(socket);

    while let Some(frame) = connection.read_frame().await.unwrap() {
        let response = dispatch(&db, frame);
        if let Err(e) = connection.write_frame(&response).await {
            eprintln!("Failed to write response: {:?}", e);
            break;
        }
    }
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();
    println!("my_mini_redis server listening on 127.0.0.1:6379");

    let db = new_db();

    loop {
        let (socket, addr) = listener.accept().await.unwrap();
        println!("Client connected: {}", addr);
        let db = db.clone();
        tokio::spawn(async move {
            process(socket, db).await;
            println!("Client disconnected: {}", addr);
        });
    }
}
