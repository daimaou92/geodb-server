use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use maxminddb::MaxMindDBError;

#[derive(Default)]
struct AppState {
    init: bool,
    cou_reader: Option<maxminddb::Reader<Vec<u8>>>,
    cit_reader: Option<maxminddb::Reader<Vec<u8>>>,
    asn_reader: Option<maxminddb::Reader<Vec<u8>>>,
    cou_hashmap: HashMap<String, geodb::Country>,
}

type SharedState = Arc<RwLock<AppState>>;

async fn index_handler(axum::extract::State(state): axum::extract::State<SharedState>) -> String {
    "Hello World".to_string()
}

fn add_routes(state: SharedState) -> axum::Router<SharedState> {
    let app =
        axum::Router::with_state(Arc::clone(&state)).route("/", axum::routing::get(index_handler));
    app
}

#[tokio::main]
async fn main() {
    let shared_state = SharedState::default();
    let ss1 = Arc::clone(&shared_state);
    tokio::spawn(async move {
        let ss = ss1;
        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(10);
        tokio::spawn(async move {
            geodb::sync_dbs(Some(tx)).await;
        });

        while let Some(v) = rx.recv().await {
            if &v == "updated" || &v == "nochange" {
                let mut ss = ss.write().await;
                if &v == "updated" {
                    if let Ok(t) = geodb::countries_hashmap().await {
                        ss.cou_hashmap = t;
                    }
                    if let Ok(t) = geodb::reader_countries().await {
                        ss.cou_reader = Some(maxminddb::Reader::<Vec<u8>>::from(t));
                    }

                    if let Ok(t) = geodb::reader_cities().await {
                        ss.cit_reader = Some(maxminddb::Reader::<Vec<u8>>::from(t));
                    }

                    if let Ok(t) = geodb::reader_asn().await {
                        ss.asn_reader = Some(maxminddb::Reader::<Vec<u8>>::from(t));
                    }
                }
                ss.init = true;
            } else {
                eprintln!("geo db update errored");
            }
        }
    });

    println!("Waiting for init....");
    loop {
        let ss = shared_state.read().await;
        if ss.init {
            break;
        }
    }
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 40000));
    let app = add_routes(Arc::clone(&shared_state));
    println!("Starting server at: 40000");
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
