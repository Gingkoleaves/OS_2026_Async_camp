//! my_mini_redis — A Redis-compatible in-memory database server
//!
//! Supports typed storage (String, List, Hash, Set), TTL-based expiration,
//! and 27+ Redis commands across 4 tiers.

use mini_redis::Frame;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub enum DataType {
    String(Vec<u8>),
    List(VecDeque<Vec<u8>>),
    Hash(HashMap<String, Vec<u8>>),
    Set(HashSet<Vec<u8>>),
}

impl DataType {
    pub fn type_name(&self) -> &'static str {
        match self {
            DataType::String(_) => "string",
            DataType::List(_) => "list",
            DataType::Hash(_) => "hash",
            DataType::Set(_) => "set",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DbEntry {
    pub data: DataType,
    pub expires_at: Option<Instant>,
}

impl DbEntry {
    pub fn is_expired(&self) -> bool {
        self.expires_at.map_or(false, |t| t <= Instant::now())
    }
}

pub const NUM_SHARDS: usize = 16;
pub type Shard = Mutex<HashMap<String, DbEntry>>;
pub type ShardedDb = Arc<Vec<Shard>>;

pub fn new_db() -> ShardedDb {
    let mut db = Vec::with_capacity(NUM_SHARDS);
    for _ in 0..NUM_SHARDS { db.push(Mutex::new(HashMap::new())); }
    Arc::new(db)
}

fn shard_index(key: &str) -> usize {
    let mut h = DefaultHasher::new();
    key.hash(&mut h);
    (h.finish() as usize) % NUM_SHARDS
}

fn get_entry_mut<'a>(s: &'a mut HashMap<String, DbEntry>, k: &str) -> Option<&'a mut DbEntry> {
    if s.get(k).map_or(false, |e| e.is_expired()) { s.remove(k); return None; }
    s.get_mut(k)
}

fn get_entry<'a>(s: &'a HashMap<String, DbEntry>, k: &str) -> Option<&'a DbEntry> {
    s.get(k).filter(|e| !e.is_expired())
}

fn err(msg: &str) -> Frame { Frame::Error(msg.into()) }

// ==================== T1: Basic ====================

pub fn cmd_ping() -> Frame { Frame::Simple("PONG".into()) }

pub fn cmd_del(db: &ShardedDb, keys: &[String]) -> Frame {
    let mut n = 0u64;
    for k in keys { let i = shard_index(k); if db[i].lock().unwrap().remove(k).is_some() { n += 1; } }
    Frame::Integer(n)
}

pub fn cmd_exists(db: &ShardedDb, key: &str) -> Frame {
    Frame::Integer(if get_entry(&db[shard_index(key)].lock().unwrap(), key).is_some() { 1 } else { 0 })
}

pub fn cmd_keys(db: &ShardedDb, pattern: &str) -> Frame {
    let wc = pattern == "*";
    let prefix = pattern.trim_end_matches('*');
    let mut r: Vec<Frame> = Vec::new();
    for s in db.iter() {
        let mut m = s.lock().unwrap();
        let expired: Vec<String> = m.iter().filter(|(_,e)| e.is_expired()).map(|(k,_)| k.clone()).collect();
        for k in &expired { m.remove(k); }
        for k in m.keys() {
            let ok = if wc { true } else if pattern.ends_with('*') { k.starts_with(prefix) } else { k == pattern };
            if ok { r.push(Frame::Bulk(k.clone().into())); }
        }
    }
    Frame::Array(r)
}

pub fn cmd_flushall(db: &ShardedDb) -> Frame {
    for s in db.iter() { s.lock().unwrap().clear(); }
    Frame::Simple("OK".into())
}

pub fn cmd_dbsize(db: &ShardedDb) -> Frame {
    Frame::Integer(db.iter().map(|s| s.lock().unwrap().values().filter(|e| !e.is_expired()).count() as u64).sum())
}

// ==================== T2: String ====================

pub fn cmd_set(db: &ShardedDb, key: &str, value: Vec<u8>) -> Frame {
    let i = shard_index(key);
    db[i].lock().unwrap().insert(key.to_string(), DbEntry { data: DataType::String(value), expires_at: None });
    Frame::Simple("OK".into())
}

pub fn cmd_get(db: &ShardedDb, key: &str) -> Frame {
    let i = shard_index(key);
    let s = db[i].lock().unwrap();
    match get_entry(&s, key) {
        Some(e) => match &e.data { DataType::String(v) => Frame::Bulk(v.clone().into()), o => err(&format!("WRONGTYPE expected string got {}", o.type_name())) },
        None => Frame::Null,
    }
}

fn parse_i64(data: &[u8]) -> Result<i64, Frame> {
    std::str::from_utf8(data).map_err(|_| err("ERR not integer")).and_then(|s| s.trim().parse::<i64>().map_err(|_| err("ERR not integer")))
}

pub fn cmd_incr(db: &ShardedDb, key: &str) -> Frame {
    cmd_incr_decr(db, key, 1)
}
pub fn cmd_decr(db: &ShardedDb, key: &str) -> Frame {
    cmd_incr_decr(db, key, -1)
}
fn cmd_incr_decr(db: &ShardedDb, key: &str, delta: i64) -> Frame {
    let i = shard_index(key);
    let mut s = db[i].lock().unwrap();
    match get_entry_mut(&mut s, key) {
        Some(e) => match &mut e.data {
            DataType::String(v) => {
                let n = match parse_i64(v) { Ok(n) => n, Err(x) => return x };
                let r = n + delta; *v = r.to_string().into_bytes();
                Frame::Integer(r as u64)
            }
            o => err(&format!("WRONGTYPE expected string got {}", o.type_name())),
        },
        None => {
            let init = if delta == 1 { b"1".to_vec() } else { b"-1".to_vec() };
            s.insert(key.to_string(), DbEntry { data: DataType::String(init), expires_at: None });
            Frame::Integer(if delta == 1 { 1 } else { (-1i64) as u64 })
        }
    }
}

pub fn cmd_append(db: &ShardedDb, key: &str, value: &[u8]) -> Frame {
    let i = shard_index(key);
    let mut s = db[i].lock().unwrap();
    match get_entry_mut(&mut s, key) {
        Some(e) => match &mut e.data {
            DataType::String(v) => { v.extend_from_slice(value); Frame::Integer(v.len() as u64) }
            o => err(&format!("WRONGTYPE expected string got {}", o.type_name())),
        },
        None => { let v = value.to_vec(); let l = v.len() as u64; s.insert(key.to_string(), DbEntry{data:DataType::String(v),expires_at:None}); Frame::Integer(l) }
    }
}

pub fn cmd_strlen(db: &ShardedDb, key: &str) -> Frame {
    let i = shard_index(key);
    let s = db[i].lock().unwrap();
    match get_entry(&s, key) {
        Some(e) => match &e.data { DataType::String(v) => Frame::Integer(v.len() as u64), _ => Frame::Integer(0) },
        None => Frame::Integer(0),
    }
}

// ==================== T2: Expiry ====================

pub fn cmd_expire(db: &ShardedDb, key: &str, seconds: u64) -> Frame {
    let i = shard_index(key);
    let mut s = db[i].lock().unwrap();
    match get_entry_mut(&mut s, key) {
        Some(e) => { e.expires_at = Some(Instant::now() + Duration::from_secs(seconds)); Frame::Integer(1) }
        None => Frame::Integer(0),
    }
}

pub fn cmd_ttl(db: &ShardedDb, key: &str) -> Frame {
    let i = shard_index(key);
    let s = db[i].lock().unwrap();
    match get_entry(&s, key) {
        Some(e) => match e.expires_at {
            Some(t) => { let n = Instant::now(); if t <= n { Frame::Integer((-2i64) as u64) } else { Frame::Integer((t - n).as_secs()) } }
            None => Frame::Integer((-1i64) as u64),
        },
        None => Frame::Integer((-2i64) as u64),
    }
}

pub fn cmd_persist(db: &ShardedDb, key: &str) -> Frame {
    let i = shard_index(key);
    let mut s = db[i].lock().unwrap();
    match get_entry_mut(&mut s, key) {
        Some(e) => if e.expires_at.is_some() { e.expires_at = None; Frame::Integer(1) } else { Frame::Integer(0) },
        None => Frame::Integer(0),
    }
}

// ==================== T2: List ====================

pub fn cmd_lpush(db: &ShardedDb, key: &str, values: &[Vec<u8>]) -> Frame {
    list_op(db, key, |list| { for v in values.iter().rev() { list.push_front(v.clone()); } list.len() as u64 })
}
pub fn cmd_rpush(db: &ShardedDb, key: &str, values: &[Vec<u8>]) -> Frame {
    list_op(db, key, |list| { for v in values { list.push_back(v.clone()); } list.len() as u64 })
}
fn list_op<F: FnOnce(&mut VecDeque<Vec<u8>>) -> u64>(db: &ShardedDb, key: &str, f: F) -> Frame {
    let i = shard_index(key);
    let mut s = db[i].lock().unwrap();
    match get_entry_mut(&mut s, key) {
        Some(e) => match &mut e.data { DataType::List(l) => Frame::Integer(f(l)), o => err(&format!("WRONGTYPE expected list got {}", o.type_name())) },
        None => { let mut l = VecDeque::new(); let r = f(&mut l); s.insert(key.to_string(), DbEntry{data:DataType::List(l),expires_at:None}); Frame::Integer(r) }
    }
}

pub fn cmd_lpop(db: &ShardedDb, key: &str) -> Frame {
    let i = shard_index(key);
    let mut s = db[i].lock().unwrap();
    match get_entry_mut(&mut s, key) {
        Some(e) => match &mut e.data { DataType::List(l) => match l.pop_front() { Some(v) => Frame::Bulk(v.into()), None => Frame::Null }, o => err(&format!("WRONGTYPE expected list got {}", o.type_name())) },
        None => Frame::Null,
    }
}

pub fn cmd_rpop(db: &ShardedDb, key: &str) -> Frame {
    let i = shard_index(key);
    let mut s = db[i].lock().unwrap();
    match get_entry_mut(&mut s, key) {
        Some(e) => match &mut e.data { DataType::List(l) => match l.pop_back() { Some(v) => Frame::Bulk(v.into()), None => Frame::Null }, o => err(&format!("WRONGTYPE expected list got {}", o.type_name())) },
        None => Frame::Null,
    }
}

pub fn cmd_lrange(db: &ShardedDb, key: &str, start: i64, stop: i64) -> Frame {
    let i = shard_index(key);
    let s = db[i].lock().unwrap();
    match get_entry(&s, key) {
        Some(e) => match &e.data {
            DataType::List(l) => {
                let len = l.len() as i64; if len == 0 { return Frame::Array(vec![]); }
                let a = if start < 0 { (len+start).max(0) } else { start.min(len-1) };
                let b = if stop < 0 { (len+stop).max(0) } else { stop.min(len-1) };
                if a > b { return Frame::Array(vec![]); }
                Frame::Array(l.iter().skip(a as usize).take((b-a+1) as usize).map(|v| Frame::Bulk(v.clone().into())).collect())
            }
            o => err(&format!("WRONGTYPE expected list got {}", o.type_name())),
        },
        None => Frame::Array(vec![]),
    }
}

pub fn cmd_llen(db: &ShardedDb, key: &str) -> Frame {
    let i = shard_index(key);
    let s = db[i].lock().unwrap();
    match get_entry(&s, key) {
        Some(e) => match &e.data { DataType::List(l) => Frame::Integer(l.len() as u64), _ => Frame::Integer(0) },
        None => Frame::Integer(0),
    }
}

// ==================== T2: Hash ====================

pub fn cmd_hset(db: &ShardedDb, key: &str, field: &str, value: Vec<u8>) -> Frame {
    let i = shard_index(key);
    let mut s = db[i].lock().unwrap();
    let new = match get_entry_mut(&mut s, key) {
        Some(e) => match &mut e.data { DataType::Hash(h) => { let n = !h.contains_key(field); h.insert(field.to_string(), value); n }, _ => return err("WRONGTYPE expected hash") },
        None => { let mut h = HashMap::new(); h.insert(field.to_string(), value); s.insert(key.to_string(), DbEntry{data:DataType::Hash(h),expires_at:None}); true }
    };
    Frame::Integer(if new { 1 } else { 0 })
}

pub fn cmd_hget(db: &ShardedDb, key: &str, field: &str) -> Frame {
    let i = shard_index(key);
    let s = db[i].lock().unwrap();
    match get_entry(&s, key) {
        Some(e) => match &e.data { DataType::Hash(h) => match h.get(field) { Some(v) => Frame::Bulk(v.clone().into()), None => Frame::Null }, _ => Frame::Null },
        None => Frame::Null,
    }
}

pub fn cmd_hdel(db: &ShardedDb, key: &str, fields: &[String]) -> Frame {
    let i = shard_index(key);
    let mut s = db[i].lock().unwrap();
    let d = match get_entry_mut(&mut s, key) {
        Some(e) => match &mut e.data { DataType::Hash(h) => fields.iter().filter(|f| h.remove(*f).is_some()).count() as u64, _ => 0 },
        None => 0,
    };
    Frame::Integer(d)
}

pub fn cmd_hgetall(db: &ShardedDb, key: &str) -> Frame {
    let i = shard_index(key);
    let s = db[i].lock().unwrap();
    match get_entry(&s, key) {
        Some(e) => match &e.data { DataType::Hash(h) => Frame::Array(h.iter().flat_map(|(k,v)| vec![Frame::Bulk(k.clone().into()), Frame::Bulk(v.clone().into())]).collect()), _ => Frame::Array(vec![]) },
        None => Frame::Array(vec![]),
    }
}

pub fn cmd_hkeys(db: &ShardedDb, key: &str) -> Frame {
    let i = shard_index(key);
    let s = db[i].lock().unwrap();
    match get_entry(&s, key) {
        Some(e) => match &e.data { DataType::Hash(h) => Frame::Array(h.keys().map(|k| Frame::Bulk(k.clone().into())).collect()), _ => Frame::Array(vec![]) },
        None => Frame::Array(vec![]),
    }
}

pub fn cmd_hlen(db: &ShardedDb, key: &str) -> Frame {
    let i = shard_index(key);
    let s = db[i].lock().unwrap();
    match get_entry(&s, key) {
        Some(e) => match &e.data { DataType::Hash(h) => Frame::Integer(h.len() as u64), _ => Frame::Integer(0) },
        None => Frame::Integer(0),
    }
}

// ==================== T3: Set ====================

pub fn cmd_sadd(db: &ShardedDb, key: &str, members: &[Vec<u8>]) -> Frame {
    let i = shard_index(key);
    let mut s = db[i].lock().unwrap();
    match get_entry_mut(&mut s, key) {
        Some(e) => match &mut e.data { DataType::Set(st) => Frame::Integer(members.iter().filter(|m| st.insert(m.to_vec())).count() as u64), o => err(&format!("WRONGTYPE expected set got {}", o.type_name())) },
        None => { let st: HashSet<Vec<u8>> = members.iter().map(|m| m.to_vec()).collect(); let n = st.len() as u64; s.insert(key.to_string(), DbEntry{data:DataType::Set(st),expires_at:None}); Frame::Integer(n) }
    }
}

pub fn cmd_srem(db: &ShardedDb, key: &str, members: &[Vec<u8>]) -> Frame {
    let i = shard_index(key);
    let mut s = db[i].lock().unwrap();
    let d = match get_entry_mut(&mut s, key) {
        Some(e) => match &mut e.data { DataType::Set(st) => members.iter().filter(|m| st.remove(*m)).count() as u64, _ => 0 },
        None => 0,
    };
    Frame::Integer(d)
}

pub fn cmd_smembers(db: &ShardedDb, key: &str) -> Frame {
    let i = shard_index(key);
    let s = db[i].lock().unwrap();
    match get_entry(&s, key) {
        Some(e) => match &e.data { DataType::Set(st) => Frame::Array(st.iter().map(|v| Frame::Bulk(v.clone().into())).collect()), _ => Frame::Array(vec![]) },
        None => Frame::Array(vec![]),
    }
}

pub fn cmd_sismember(db: &ShardedDb, key: &str, member: &[u8]) -> Frame {
    let i = shard_index(key);
    let s = db[i].lock().unwrap();
    Frame::Integer(match get_entry(&s, key) { Some(e) => match &e.data { DataType::Set(st) => if st.contains(member) {1} else {0}, _ => 0 }, None => 0 })
}

pub fn cmd_scard(db: &ShardedDb, key: &str) -> Frame {
    let i = shard_index(key);
    let s = db[i].lock().unwrap();
    Frame::Integer(match get_entry(&s, key) { Some(e) => match &e.data { DataType::Set(st) => st.len() as u64, _ => 0 }, None => 0 })
}

// ==================== T4: Multi ====================

pub fn cmd_mget(db: &ShardedDb, keys: &[String]) -> Frame {
    Frame::Array(keys.iter().map(|k| {
        let i = shard_index(k); let s = db[i].lock().unwrap();
        match get_entry(&s, k) { Some(e) => match &e.data { DataType::String(v) => Frame::Bulk(v.clone().into()), _ => Frame::Null }, None => Frame::Null }
    }).collect())
}

pub fn cmd_mset(db: &ShardedDb, kv_pairs: &[(String, Vec<u8>)]) -> Frame {
    for (k, v) in kv_pairs { let i = shard_index(k); db[i].lock().unwrap().insert(k.clone(), DbEntry{data:DataType::String(v.clone()),expires_at:None}); }
    Frame::Simple("OK".into())
}

// ==================== Dispatch ====================

use mini_redis::Command;

/// Process a Frame as a Redis command. Tries Command::from_frame first;
/// falls back to generic text-based dispatch for non-standard commands.
pub fn dispatch(db: &ShardedDb, frame: mini_redis::Frame) -> Frame {
    // Try to parse as a standard mini-redis Command (GET, SET)
    if let Ok(cmd) = Command::from_frame(frame.clone()) {
        use Command::{Get, Set};
        match cmd {
            Set(c) => return cmd_set(db, c.key(), c.value().to_vec()),
            Get(c) => return cmd_get(db, c.key()),
            _ => {} // Fall through to generic dispatch
        }
    }
    // Generic dispatch from Frame arguments
    handle_generic(db, frame)
}

fn handle_generic(db: &ShardedDb, frame: mini_redis::Frame) -> Frame {
    let args = match frame {
        mini_redis::Frame::Array(arr) => arr,
        _ => return err("ERR expected array of bulk strings"),
    };
    if args.is_empty() { return err("ERR empty command"); }
    let name = match &args[0] { Frame::Bulk(b) => String::from_utf8_lossy(b).to_ascii_uppercase(), _ => return err("ERR invalid format"), };
    match name.as_str() {
        "PING" => cmd_ping(),
        "DEL" => cmd_del(db, &extract_keys(&args[1..])),
        "EXISTS" => match args.get(1) { Some(Frame::Bulk(k)) => cmd_exists(db, &String::from_utf8_lossy(k)), _ => Frame::Integer(0) },
        "KEYS" => cmd_keys(db, &args.get(1).and_then(as_str).unwrap_or_else(|| "*".to_string())),
        "FLUSHALL" => cmd_flushall(db),
        "DBSIZE" => cmd_dbsize(db),
        "INCR"|"DECR" => match args.get(1) { Some(Frame::Bulk(k)) => { let k = String::from_utf8_lossy(k).to_string(); if name=="INCR"{cmd_incr(db,&k)}else{cmd_decr(db,&k)} }, _ => err("ERR wrong args") },
        "APPEND" => match (args.get(1),args.get(2)) { (Some(Frame::Bulk(k)),Some(Frame::Bulk(v))) => cmd_append(db,&String::from_utf8_lossy(k),v), _ => err("ERR wrong args for APPEND") },
        "STRLEN" => match args.get(1) { Some(Frame::Bulk(k)) => cmd_strlen(db,&String::from_utf8_lossy(k)), _ => Frame::Integer(0) },
        "EXPIRE" => match (args.get(1),args.get(2)) { (Some(Frame::Bulk(k)),Some(s)) => match as_u64_arg(s) { Some(n) => cmd_expire(db,&String::from_utf8_lossy(k),n), None => err("ERR wrong args for EXPIRE") }, _ => err("ERR wrong args for EXPIRE") },
        "TTL" => match args.get(1) { Some(Frame::Bulk(k)) => cmd_ttl(db,&String::from_utf8_lossy(k)), _ => Frame::Integer((-2i64) as u64) },
        "PERSIST" => match args.get(1) { Some(Frame::Bulk(k)) => cmd_persist(db,&String::from_utf8_lossy(k)), _ => Frame::Integer(0) },
        "LPUSH"|"RPUSH" => {
            if args.len()<3 { return err("ERR wrong args"); }
            match &args[1] { Frame::Bulk(k) => { let k=String::from_utf8_lossy(k).to_string(); let vs: Vec<Vec<u8>> = args[2..].iter().filter_map(as_bytes).collect(); if name=="LPUSH"{cmd_lpush(db,&k,&vs)}else{cmd_rpush(db,&k,&vs)} }, _ => err("ERR invalid key") }
        }
        "LPOP"|"RPOP" => match &args.get(1) { Some(Frame::Bulk(k)) => { let k=String::from_utf8_lossy(k).to_string(); if name=="LPOP"{cmd_lpop(db,&k)}else{cmd_rpop(db,&k)} }, _ => Frame::Null },
        "LRANGE" => match (args.get(1),args.get(2),args.get(3)) { (Some(Frame::Bulk(k)),Some(s),Some(e)) => match (as_u64_arg(s),as_u64_arg(e)) { (Some(a),Some(b)) => cmd_lrange(db,&String::from_utf8_lossy(k),a as i64,b as i64), _ => err("ERR wrong args for LRANGE") }, _ => err("ERR wrong args for LRANGE") },
        "LLEN" => match args.get(1) { Some(Frame::Bulk(k)) => cmd_llen(db,&String::from_utf8_lossy(k)), _ => Frame::Integer(0) },
        "HSET" => match (args.get(1),args.get(2),args.get(3)) { (Some(Frame::Bulk(k)),Some(Frame::Bulk(f)),Some(Frame::Bulk(v))) => cmd_hset(db,&String::from_utf8_lossy(k),&String::from_utf8_lossy(f),v.to_vec()), _ => err("ERR wrong args for HSET") },
        "HGET" => match (args.get(1),args.get(2)) { (Some(Frame::Bulk(k)),Some(Frame::Bulk(f))) => cmd_hget(db,&String::from_utf8_lossy(k),&String::from_utf8_lossy(f)), _ => Frame::Null },
        "HDEL" => match &args.get(1) { Some(Frame::Bulk(k)) => { let k=String::from_utf8_lossy(k).to_string(); cmd_hdel(db,&k,&extract_keys(&args[2..])) }, _ => Frame::Integer(0) },
        "HGETALL" => match args.get(1) { Some(Frame::Bulk(k)) => cmd_hgetall(db,&String::from_utf8_lossy(k)), _ => Frame::Array(vec![]) },
        "HKEYS" => match args.get(1) { Some(Frame::Bulk(k)) => cmd_hkeys(db,&String::from_utf8_lossy(k)), _ => Frame::Array(vec![]) },
        "HLEN" => match args.get(1) { Some(Frame::Bulk(k)) => cmd_hlen(db,&String::from_utf8_lossy(k)), _ => Frame::Integer(0) },
        "SADD"|"SREM" => {
            if args.len()<3 { return err("ERR wrong args"); }
            match &args[1] { Frame::Bulk(k) => { let k=String::from_utf8_lossy(k).to_string(); let ms: Vec<Vec<u8>> = args[2..].iter().filter_map(as_bytes).collect(); if name=="SADD"{cmd_sadd(db,&k,&ms)}else{cmd_srem(db,&k,&ms)} }, _ => Frame::Integer(0) }
        }
        "SMEMBERS" => match args.get(1) { Some(Frame::Bulk(k)) => cmd_smembers(db,&String::from_utf8_lossy(k)), _ => Frame::Array(vec![]) },
        "SISMEMBER" => match (args.get(1),args.get(2)) { (Some(Frame::Bulk(k)),Some(Frame::Bulk(m))) => cmd_sismember(db,&String::from_utf8_lossy(k),m), _ => Frame::Integer(0) },
        "SCARD" => match args.get(1) { Some(Frame::Bulk(k)) => cmd_scard(db,&String::from_utf8_lossy(k)), _ => Frame::Integer(0) },
        "MGET" => cmd_mget(db, &extract_keys(&args[1..])),
        "MSET" => {
            let mut p = Vec::new(); let mut i = 1;
            while i+1 < args.len() { match (&args[i],&args[i+1]) { (Frame::Bulk(k),Frame::Bulk(v)) => p.push((String::from_utf8_lossy(k).to_string(),v.to_vec())), _ => return err("ERR wrong format for MSET") } i+=2; }
            if p.is_empty() { return err("ERR wrong args for MSET"); } cmd_mset(db, &p)
        }
        _ => Frame::Error(format!("ERR unknown command '{}'", name)),
    }
}

// helpers
fn as_str(f: &Frame) -> Option<String> { match f { Frame::Bulk(b) => Some(String::from_utf8_lossy(b).to_string()), _ => None } }
fn as_bytes(f: &Frame) -> Option<Vec<u8>> { match f { Frame::Bulk(b) => Some(b.to_vec()), _ => None } }
fn as_u64_arg(f: &Frame) -> Option<u64> {
    match f { Frame::Integer(n) => Some(*n), Frame::Bulk(b) => std::str::from_utf8(b).ok().and_then(|s| s.trim().parse::<i64>().ok().map(|n| n as u64)), _ => None }
}
fn extract_keys(frames: &[Frame]) -> Vec<String> { frames.iter().filter_map(as_str).collect() }
