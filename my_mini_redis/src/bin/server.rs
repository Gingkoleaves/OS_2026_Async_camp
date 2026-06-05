use mini_redis::{Connection, Frame};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};

type ShardedDb = Arc<Vec<Mutex<HashMap<String, Vec<u8>>>>>;

const NUM_SHARDS: usize = 16;

fn new_sharded_db() -> ShardedDb {
    let mut db = Vec::with_capacity(NUM_SHARDS);
    for _ in 0..NUM_SHARDS {
        db.push(Mutex::new(HashMap::new()));
    }
    Arc::new(db)
}

#[tokio::main]
async fn main() {
    // Bind the listener to the address
    // 监听指定地址，等待 TCP 连接进来
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    println!("Listening");

    // 使用 hashmap 来存储 redis 的数据
    let db: ShardedDb = new_sharded_db();

    loop {
        // 第二个被忽略的项中包含有新连接的 `IP` 和端口信息
        let (socket, ip) = listener.accept().await.unwrap();
        println!("Client connected from {}", ip);

        let db_clone = db.clone();
        tokio::spawn(async move {
            process(socket, db_clone).await;
        });
    }
}

async fn process(socket: TcpStream, db: ShardedDb) {
    use mini_redis::Command::{self, Get, Set};

    // `mini-redis` 提供的便利函数，使用返回的 `connection` 可以用于从 socket 中读取数据并解析为数据帧
    let mut connection = Connection::new(socket);

    // 使用 `read_frame` 方法从连接获取一个数据帧：一条redis命令 + 相应的数据
    while let Some(frame) = connection.read_frame().await.unwrap() {
        let response = match Command::from_frame(frame).unwrap() {
            Set(cmd) => {
                // 值被存储为 `Vec<u8>` 的形式
                let mut hasher = DefaultHasher::new();
                cmd.key().hash(&mut hasher);
                let shard_index = (hasher.finish() as usize) % NUM_SHARDS;
                db[shard_index]
                    .lock()
                    .unwrap()
                    .insert(cmd.key().to_string(), cmd.value().to_vec());
                Frame::Simple("OK".to_string())
            }
            Get(cmd) => {
                let mut hasher = DefaultHasher::new();
                cmd.key().hash(&mut hasher);
                let shard_index = (hasher.finish() as usize) % NUM_SHARDS;
                if let Some(value) = db[shard_index].lock().unwrap().get(cmd.key()) {
                    // `Frame::Bulk` 期待数据的类型是 `Bytes`
                    Frame::Bulk(value.clone().into())
                } else {
                    Frame::Null
                }
            }
            cmd => panic!("unimplemented {:?}", cmd),
        };

        // 将请求响应返回给客户端
        connection.write_frame(&response).await.unwrap();
    }
}

// main 函数在之前已实现
