use lazy_static::lazy_static;
use actix_web::{http, server, App, Path, Responder, Json, HttpResponse};
use serde::{Serialize, Deserialize};
use std::sync::{Mutex, Arc};
use leveldb::database::Database;
use leveldb::options::{Options, WriteOptions, ReadOptions};
use leveldb::kv::KV;
use uuid::Uuid; use serde_json;
use db_key::Key;
use rand::{self, Rng};
use rand::distributions::Alphanumeric;

struct DBKey(String);

impl Key for DBKey {
    fn from_u8(key: &[u8]) -> Self {
        DBKey(String::from_utf8_lossy(key).to_string())
    }

    fn as_slice<T, F: Fn(&[u8]) -> T>(&self, f: F) -> T {
        f(self.0.as_bytes())
    }
}

lazy_static! {
    static ref MASTER_KEY: Option<String> = std::env::var("MASTER_KEY").ok();
    static ref db: Arc<Mutex<Database<DBKey>>> = {
        let mut opts = Options::new();
        opts.create_if_missing = true;
        let inner = Database::open(std::path::Path::new("./db"), opts).unwrap();
        Arc::new(Mutex::new(inner))
    };
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Payload {
    key: Option<String>,
    target: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Resp {
    key: String,
    slug: String,
}

fn redirect(path: Path<(String, )>) -> impl Responder {
    let guard = db.lock().unwrap();

    match guard.get(ReadOptions::new(), DBKey(path.0.clone())).unwrap() {
        Some(cont) => {
            let payload: Payload = serde_json::from_slice(&cont).unwrap();
            HttpResponse::Found().header(http::header::LOCATION, payload.target).finish()
        },
        None => HttpResponse::NotFound().finish(),
    }
}

fn create(mut payload: Json<Payload>) -> impl Responder {
    if MASTER_KEY.is_some() && *MASTER_KEY != payload.key {
        return HttpResponse::Forbidden().finish();
    }

    let dbkey = DBKey(Uuid::new_v4().to_hyphenated().to_string());

    let mut rng = rand::thread_rng();
    let key = std::iter::repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .take(32)
        .collect();

    let resp = Resp{
        slug: dbkey.0.clone(),
        key,
    };

    payload.key = Some(resp.key.clone());

    let guard = db.lock().unwrap();
    guard.put(WriteOptions::new(), dbkey, &serde_json::to_vec(&*payload).unwrap());

    HttpResponse::Created().json(resp)
}

/*
fn edit(path: Path<(String, )>) -> impl Responder {
}
*/

fn main() {
    let server = server::new(|| App::new()
                .route("/edit", http::Method::POST, create)
                .resource("/{id}", |r| r.with(redirect)));
    let bind = server.bind("127.0.0.1:7103").unwrap();
    bind.run();
}
