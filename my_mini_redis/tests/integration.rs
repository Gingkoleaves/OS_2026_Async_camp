//! Integration tests for my_mini_redis — Frame-based, shared server.

use mini_redis::{Connection, Frame};
use std::sync::OnceLock;
use tokio::net::TcpStream;

const PORT: u16 = 17379;

/// Start the server on a dedicated OS thread with its own tokio runtime,
/// so it survives across all #[tokio::test] invocations.
fn ensure_server() {
    static S: OnceLock<()> = OnceLock::new();
    S.get_or_init(|| {
        std::thread::spawn(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let l = tokio::net::TcpListener::bind(format!("127.0.0.1:{PORT}")).await.unwrap();
                let db = my_mini_redis::new_db();
                loop {
                    let (s, _) = l.accept().await.unwrap();
                    let db = db.clone();
                    tokio::spawn(async move {
                        let mut c = Connection::new(s);
                        loop {
                            match c.read_frame().await {
                                Ok(Some(f)) => {
                                    let r = my_mini_redis::dispatch(&db, f);
                                    if c.write_frame(&r).await.is_err() { break; }
                                }
                                _ => break,
                            }
                        }
                    });
                }
            });
        });
    });
}

async fn send(args: &[&str]) -> Frame {
    // retry until server is ready (max 5s)
    let stream = {
        let start = std::time::Instant::now();
        loop {
            match TcpStream::connect(format!("127.0.0.1:{PORT}")).await {
                Ok(s) => break s,
                Err(_) if start.elapsed() < std::time::Duration::from_secs(5) => {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                Err(e) => panic!("server did not start within 5s: {e}"),
            }
        }
    };
    let mut c = Connection::new(stream);
    let frame = Frame::Array(args.iter().map(|s| Frame::Bulk(s.to_string().into())).collect());
    // 10s timeout for write + read
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        c.write_frame(&frame).await?;
        let resp = c.read_frame().await?.unwrap();
        Ok::<_, mini_redis::Error>(resp)
    }).await.unwrap().unwrap()
}

fn rpt(name: &str, ok: bool) { println!("[{}] {}", if ok {"PASS"} else {"FAIL"}, name); if !ok { panic!("FAILED: {name}"); } }
fn as_int(f: &Frame) -> u64 { match f { Frame::Integer(n) => *n, _ => panic!("expected Integer, got {f:?}") } }
fn as_str(f: &Frame) -> String { match f { Frame::Bulk(b) => String::from_utf8_lossy(b).into(), Frame::Simple(s) => s.clone(), _ => panic!("expected Bulk/Simple, got {f:?}") } }
fn as_opt(f: &Frame) -> Option<String> { match f { Frame::Bulk(b) => Some(String::from_utf8_lossy(b).into()), Frame::Null => None, _ => panic!("expected Bulk/Null, got {f:?}") } }
fn as_arr(f: &Frame) -> &Vec<Frame> { match f { Frame::Array(a) => a, _ => panic!("expected Array, got {f:?}") } }

// Unique key prefixes per test to avoid shared-state conflicts
// ==================== T1: Basic ====================

#[tokio::test] async fn t1_ping() {
    ensure_server(); rpt("T1-01 PING PONG", matches!(send(&["PING"]).await, Frame::Simple(ref s) if s=="PONG"));
}
#[tokio::test] async fn t1_set_get() {
    ensure_server();
    rpt("T1-02 SET OK", matches!(send(&["SET","_t1k","v"]).await, Frame::Simple(ref s) if s=="OK"));
    rpt("T1-03 GET v", as_str(&send(&["GET","_t1k"]).await)=="v");
    rpt("T1-04 GET nil", matches!(send(&["GET","_t1x"]).await, Frame::Null));
}
#[tokio::test] async fn t1_del_exists() {
    ensure_server();
    send(&["SET","_t1d","x"]).await;
    rpt("T1-05 EXISTS 1", as_int(&send(&["EXISTS","_t1d"]).await)==1);
    rpt("T1-06 DEL 1", as_int(&send(&["DEL","_t1d"]).await)==1);
    rpt("T1-07 EXISTS 0", as_int(&send(&["EXISTS","_t1d"]).await)==0);
    rpt("T1-08 DEL 0", as_int(&send(&["DEL","_t1nope"]).await)==0);
}
#[tokio::test] async fn t1_keys_flushall_dbsize() {
    ensure_server();
    send(&["SET","_t1a","1"]).await; send(&["SET","_t1ab","2"]).await; send(&["SET","_t1c","3"]).await;
    let keys_frame = send(&["KEYS","*"]).await;
    rpt("T1-09 KEYS * finds keys", as_arr(&keys_frame).len()>=3);
    rpt("T1-10 KEYS _t1a* 2", as_arr(&send(&["KEYS","_t1a*"]).await).len()==2);
    // DBSIZE just checks >0 since other tests may have keys
    rpt("T1-11 DBSIZE >0", as_int(&send(&["DBSIZE"]).await)>0);
}

// ==================== T2: String ====================

#[tokio::test] async fn t2_incr_decr() {
    ensure_server();
    rpt("T2-01 INCR new→1", as_int(&send(&["INCR","_t2cnt"]).await)==1);
    rpt("T2-02 INCR→2", as_int(&send(&["INCR","_t2cnt"]).await)==2);
    rpt("T2-03 DECR→1", as_int(&send(&["DECR","_t2cnt"]).await)==1);
    send(&["SET","_t2str","hi"]).await;
    rpt("T2-04 INCR err", matches!(send(&["INCR","_t2str"]).await, Frame::Error(_)));
    rpt("T2-05 STRLEN 2", as_int(&send(&["STRLEN","_t2str"]).await)==2);
    rpt("T2-06 APPEND 8", as_int(&send(&["APPEND","_t2str"," there"]).await)==8);
}
#[tokio::test] async fn t2_expire_ttl() {
    ensure_server();
    send(&["SET","_t2e","x"]).await; send(&["EXPIRE","_t2e","100"]).await;
    let t = as_int(&send(&["TTL","_t2e"]).await);
    rpt("T2-07 TTL ok", t>0&&t<=100);
    rpt("T2-08 PERSIST 1", as_int(&send(&["PERSIST","_t2e"]).await)==1);
    rpt("T2-09 TTL -1", as_int(&send(&["TTL","_t2e"]).await)==(-1i64)as u64);
    rpt("T2-10 TTL -2", as_int(&send(&["TTL","_t2no"]).await)==(-2i64)as u64);
}

// ==================== T2: List / Hash ====================

#[tokio::test] async fn t2_list() {
    ensure_server();
    // LPUSH _t2l c b a → Redis returns [c, b, a] (LIFO order)
    rpt("T3-01 LPUSH 3", as_int(&send(&["LPUSH","_t2l","c","b","a"]).await)==3);
    let v: Vec<_> = as_arr(&send(&["LRANGE","_t2l","0","-1"]).await).iter().map(as_str).collect();
    rpt("T3-02 LRANGE [c,b,a]", v==vec!["c","b","a"]);
    // After LPUSH c,b,a → [c,b,a]; RPUSH d → [c,b,a,d]; LPOP → "c"; [b,a,d]=len 3
    rpt("T3-03 RPUSH 4", as_int(&send(&["RPUSH","_t2l","d"]).await)==4);
    rpt("T3-04 LPOP c", as_str(&send(&["LPOP","_t2l"]).await)=="c");
    rpt("T3-05 LLEN 3", as_int(&send(&["LLEN","_t2l"]).await)==3);
}
#[tokio::test] async fn t2_hash() {
    ensure_server();
    rpt("T4-01 HSET 1", as_int(&send(&["HSET","_t2h","n","r"]).await)==1);
    rpt("T4-02 HGET r", as_str(&send(&["HGET","_t2h","n"]).await)=="r");
    rpt("T4-03 HGETALL 2", as_arr(&send(&["HGETALL","_t2h"]).await).len()==2);
    rpt("T4-04 HLEN 1", as_int(&send(&["HLEN","_t2h"]).await)==1);
    rpt("T4-05 HDEL 1", as_int(&send(&["HDEL","_t2h","n"]).await)==1);
}

// ==================== T3: Set / T4: Multi / Edge ====================

#[tokio::test] async fn t3_set() {
    ensure_server();
    rpt("T5-01 SADD 3", as_int(&send(&["SADD","_t3s","a","b","c"]).await)==3);
    rpt("T5-02 SADD dup 0", as_int(&send(&["SADD","_t3s","a"]).await)==0);
    rpt("T5-03 SMEMBERS 3", as_arr(&send(&["SMEMBERS","_t3s"]).await).len()==3);
    rpt("T5-04 SISMEMBER 1", as_int(&send(&["SISMEMBER","_t3s","a"]).await)==1);
    rpt("T5-05 SISMEMBER 0", as_int(&send(&["SISMEMBER","_t3s","z"]).await)==0);
    rpt("T5-06 SCARD 3", as_int(&send(&["SCARD","_t3s"]).await)==3);
}
#[tokio::test] async fn t4_multi_key() {
    ensure_server();
    rpt("T6-01 MSET OK", matches!(send(&["MSET","_t4k1","v1","_t4k2","v2"]).await, Frame::Simple(_)));
    let v: Vec<_> = as_arr(&send(&["MGET","_t4k1","_t4k2","_t4k3"]).await).iter().map(as_opt).collect();
    rpt("T6-02 MGET", v==vec![Some("v1".into()),Some("v2".into()),None]);
}
#[tokio::test] async fn edge_wrongtype() {
    ensure_server();
    send(&["SET","_e1s","x"]).await;
    rpt("E-01 WRONGTYPE LPUSH", matches!(send(&["LPUSH","_e1s","x"]).await, Frame::Error(ref e) if e.contains("WRONGTYPE")));
}
#[tokio::test] async fn edge_unknown() {
    ensure_server();
    rpt("E-02 unknown cmd", matches!(send(&["FOOBAR"]).await, Frame::Error(_)));
}
#[tokio::test] async fn edge_expiry_lazy() {
    ensure_server();
    send(&["SET","_ex","v"]).await; send(&["EXPIRE","_ex","1"]).await;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    rpt("E-03 expired nil", matches!(send(&["GET","_ex"]).await, Frame::Null));
}
