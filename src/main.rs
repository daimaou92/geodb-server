use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Default)]
struct AppState {
    init: bool,
    cou_reader: Option<maxminddb::Reader<Vec<u8>>>,
    cit_reader: Option<maxminddb::Reader<Vec<u8>>>,
    asn_reader: Option<maxminddb::Reader<Vec<u8>>>,
    cou_hashmap: HashMap<String, geodb::Country>,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
struct Country {
    iso2: String,
    name: String,
    dial_codes: Vec<String>,
}

impl AppState {
    fn country_by_ip(&self, ip: std::net::IpAddr) -> Result<Country, String> {
        let reader = if let Some(r) = &self.cou_reader {
            r
        } else {
            return Err("NO_INIT".to_string());
        };

        let country: maxminddb::geoip2::Country = if let Ok(c) = reader.lookup(ip) {
            c
        } else {
            return Err("LOOKUP_ERR".to_string());
        };
        let country = if let Some(t) = country.country {
            t
        } else {
            return Err("COUNTRY_OP_ERR".to_string());
        };
        let mut res = Country::default();
        if let Some(i) = country.iso_code {
            res.iso2 = String::from(i);
        }
        if let Some(n) = country.names {
            if let Some(n) = n.get("en") {
                res.name = String::from(*n);
            }
        }

        if let Some(v) = self.cou_hashmap.get(&res.iso2) {
            res.dial_codes = v.dial_codes.clone();
        }
        Ok(res)
    }
}

type SharedState = Arc<RwLock<AppState>>;

async fn get_country_by_ip(
    axum::extract::Path(p): axum::extract::Path<String>,
    axum::extract::State(state): axum::extract::State<SharedState>,
) -> impl axum::response::IntoResponse {
    println!("IP: {:?}", p.as_str());
    let ip = if let Ok(p) = std::net::IpAddr::from_str(p.as_str()) {
        p
    } else {
        return (axum::http::StatusCode::BAD_REQUEST, axum::Json::default());
    };
    let mut c: Country = Country::default();
    match state.read().await.country_by_ip(ip) {
        Ok(v) => {
            c = v.clone();
        }
        Err(e) => {
            eprintln!("{:?}", e);
            return (axum::http::StatusCode::BAD_REQUEST, axum::Json::default());
        }
    }
    (axum::http::StatusCode::OK, axum::Json(c))
}

async fn index_handler(axum::extract::State(state): axum::extract::State<SharedState>) -> String {
    "Hello World".to_string()
}

fn add_routes(state: SharedState) -> axum::Router<SharedState> {
    let app = axum::Router::with_state(Arc::clone(&state))
        .route("/", axum::routing::get(index_handler))
        .route("/:key", axum::routing::get(get_country_by_ip));
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
                if (ss.init && &v == "updated") || !ss.init {
                    match geodb::countries_hashmap().await {
                        Ok(t) => {
                            ss.cou_hashmap = t;
                        }
                        Err(e) => {
                            eprintln!("Hashmap Err: {:?}", e);
                        }
                    };

                    match geodb::reader_countries().await {
                        Ok(t) => {
                            ss.cou_reader = Some(maxminddb::Reader::<Vec<u8>>::from(t));
                        }
                        Err(e) => {
                            eprintln!("Reader Countries Err: {:?}", e);
                        }
                    };

                    match geodb::reader_cities().await {
                        Ok(t) => {
                            ss.cit_reader = Some(maxminddb::Reader::<Vec<u8>>::from(t));
                        }
                        Err(e) => {
                            eprintln!("Reader Cities Err: {:?}", e);
                        }
                    };

                    match geodb::reader_asn().await {
                        Ok(t) => {
                            ss.asn_reader = Some(maxminddb::Reader::<Vec<u8>>::from(t));
                        }
                        Err(e) => {
                            eprintln!("Reader ASN Err: {:?}", e);
                        }
                    };
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
