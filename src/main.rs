use bytes::{Bytes, BytesMut};
use prost::{self, Message};
use std::collections::HashMap;
use std::io::{self, BufRead};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
pub mod pb {
    pub mod geo {
        include!(concat!(env!("OUT_DIR"), "/pb.geo.rs"));
    }
}

#[derive(Default)]
struct AppState {
    init: bool,
    cou_reader: Option<maxminddb::Reader<Vec<u8>>>,
    cit_reader: Option<maxminddb::Reader<Vec<u8>>>,
    asn_reader: Option<maxminddb::Reader<Vec<u8>>>,
    cou_hashmap: HashMap<String, geodb::Country>,
    auth_keys: HashMap<String, ()>,
}

impl AppState {
    fn country_by_ip(&self, ip: std::net::IpAddr) -> Result<pb::geo::Country, String> {
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

        let iso2_code = if let Some(code) = country.iso_code {
            code
        } else {
            return Err("COUNTRY_NO_ISO2".to_string());
        };

        let chm = if let Some(v) = self.cou_hashmap.get(iso2_code) {
            v
        } else {
            return Err("COUNTRY_NOT_IN_HASHMAP".to_string());
        };

        let res = pb::geo::Country {
            dial_codes: chm.dial_codes.clone(),
            iso3: chm.iso3.clone(),
            iso_num: chm.iso_num,
            iso2: chm.iso2.clone(),
            currency_code: chm.currency_code.clone(),
            currency_name: chm.currency_name.clone(),
            name: chm.name.clone(),
            region: chm.region.clone(),
            capital: chm.capital.clone(),
            continent_code: chm.continent_code.clone(),
            tld: chm.tld.clone(),
            language_codes: chm.language_codes.clone(),
            geoname_id: chm.geoname_id,
            display_name: chm.display_name.clone(),
        };
        Ok(res)
    }

    fn country_by_iso(&self, iso_code: String) -> Result<pb::geo::Country, String> {
        if iso_code.len() != 2 {
            return Err("QUERY_INVALID".to_string());
        }

        let chm = if let Some(v) = self.cou_hashmap.get(&iso_code) {
            v
        } else {
            return Err("COUNTRY_NOT_IN_HASHMAP".to_string());
        };

        let res = pb::geo::Country {
            dial_codes: chm.dial_codes.clone(),
            iso3: chm.iso3.clone(),
            iso_num: chm.iso_num,
            iso2: chm.iso2.clone(),
            currency_code: chm.currency_code.clone(),
            currency_name: chm.currency_name.clone(),
            name: chm.name.clone(),
            region: chm.region.clone(),
            capital: chm.capital.clone(),
            continent_code: chm.continent_code.clone(),
            tld: chm.tld.clone(),
            language_codes: chm.language_codes.clone(),
            geoname_id: chm.geoname_id,
            display_name: chm.display_name.clone(),
        };
        Ok(res)
    }

    fn city_by_ip(&self, ip: std::net::IpAddr) -> Result<pb::geo::City, String> {
        let reader = if let Some(r) = &self.cit_reader {
            r
        } else {
            return Err("NO_INIT".to_string());
        };

        let city: maxminddb::geoip2::City = if let Ok(c) = reader.lookup(ip) {
            c
        } else {
            return Err("LOOKUP_ERR".to_string());
        };
        let city_inner = if let Some(t) = city.city {
            t
        } else {
            return Err("CITY_OP_ERR".to_string());
        };

        let mut res = pb::geo::City::default();
        if let Some(v) = city_inner.geoname_id {
            res.geoname_id = Some(v as i64);
        }

        if let Some(v) = city_inner.names {
            if let Some(v) = v.get("en") {
                res.name = String::from(*v);
            }
        }

        if let Some(c) = city.country {
            if let Some(v) = c.iso_code {
                res.country_iso2 = String::from(v);
            }
        }

        if let Some(loc) = city.location {
            res.latitude = loc.latitude;
            res.longitude = loc.longitude;
            if let Some(v) = loc.metro_code {
                res.metro_code = Some(v as u32);
            }

            if let Some(v) = loc.time_zone {
                res.time_zone = Some(String::from(v));
            }

            if let Some(v) = loc.accuracy_radius {
                res.radius = Some(v as u32);
            }
        }

        if let Some(pc) = city.postal {
            if let Some(v) = pc.code {
                res.postal_code = Some(String::from(v));
            }
        }

        if let Some(t) = city.traits {
            if let Some(b) = t.is_anonymous_proxy {
                res.is_anonymous_proxy = b;
            }

            if let Some(b) = t.is_satellite_provider {
                res.is_satellite_provider = b;
            }
        }

        Ok(res)
    }

    fn asn_by_ip(&self, ip: std::net::IpAddr) -> Result<pb::geo::Asn, String> {
        let reader = if let Some(r) = &self.asn_reader {
            r
        } else {
            return Err("NO_INIT".to_string());
        };

        let asn: maxminddb::geoip2::Asn = if let Ok(c) = reader.lookup(ip) {
            c
        } else {
            return Err("LOOKUP_ERR".to_string());
        };

        let mut res = pb::geo::Asn::default();
        if let Some(v) = asn.autonomous_system_number {
            res.autonomous_system_number = v;
        }

        if let Some(v) = asn.autonomous_system_organization {
            res.autonomous_system_org = String::from(v);
        }

        Ok(res)
    }

    fn authorized(&self, key: String) -> bool {
        self.auth_keys.get(&key).is_some()
    }
}

type SharedState = Arc<RwLock<AppState>>;

async fn authorized(state: SharedState, headers: axum::http::HeaderMap) -> Result<bool, String> {
    let h = if let Some(v) = headers.get(axum::http::header::AUTHORIZATION) {
        v
    } else {
        return Err("no authorization header".to_string());
    };
    let auth_key = if let Ok(v) = h.to_str() {
        String::from(v)
    } else {
        return Err("to_str on header failed".to_string());
    };

    let state = state.read().await;
    Ok(state.authorized(auth_key))
}

async fn get_country_by_ip(
    axum::extract::Path(p): axum::extract::Path<String>,
    axum::extract::State(state): axum::extract::State<SharedState>,
    headers: axum::http::HeaderMap,
) -> impl axum::response::IntoResponse {
    let ip = if let Ok(p) = std::net::IpAddr::from_str(p.as_str()) {
        p
    } else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Bytes::from_static(b"invalid ip"),
        );
    };

    match authorized(Arc::clone(&state), headers).await {
        Ok(v) => {
            if !v {
                eprintln!("not authorized!!");
                return (
                    axum::http::StatusCode::UNAUTHORIZED,
                    Bytes::from_static(b"unauthorized"),
                );
            }
        }
        Err(e) => {
            eprintln!("{:?}", e);
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Bytes::from_static(b"unauthorized"),
            );
        }
    }

    let c = match state.read().await.country_by_ip(ip) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{:?}", e);
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Bytes::from_static(b"fetching country failed"),
            );
        }
    };
    let mut b = BytesMut::new();
    match (c).encode(&mut b) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{:?}", e);
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Bytes::from_static(b"serialization failed"),
            );
        }
    };
    (axum::http::StatusCode::OK, b.into())
}

async fn get_country_by_iso(
    axum::extract::Path(p): axum::extract::Path<String>,
    axum::extract::State(state): axum::extract::State<SharedState>,
    headers: axum::http::HeaderMap,
) -> impl axum::response::IntoResponse {
    if p.len() != 2 {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Bytes::from_static(b"invalid iso2 code"),
        );
    }
    match authorized(Arc::clone(&state), headers).await {
        Ok(v) => {
            if !v {
                eprintln!("not authorized!!");
                return (
                    axum::http::StatusCode::UNAUTHORIZED,
                    Bytes::from_static(b"unauthorized"),
                );
            }
        }
        Err(e) => {
            eprintln!("{:?}", e);
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Bytes::from_static(b"unauthorized"),
            );
        }
    }

    let c = match state.read().await.country_by_iso(p.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{:?}", e);
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Bytes::from_static(b"fetching country failed"),
            );
        }
    };
    let mut b = BytesMut::new();
    match (c).encode(&mut b) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{:?}", e);
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Bytes::from_static(b"serialization failed"),
            );
        }
    };
    (axum::http::StatusCode::OK, b.into())
}

async fn get_city_by_ip(
    axum::extract::Path(p): axum::extract::Path<String>,
    axum::extract::State(state): axum::extract::State<SharedState>,
    headers: axum::http::HeaderMap,
) -> impl axum::response::IntoResponse {
    let ip = if let Ok(p) = std::net::IpAddr::from_str(p.as_str()) {
        p
    } else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Bytes::from_static(b"invalid ip"),
        );
    };

    match authorized(Arc::clone(&state), headers).await {
        Ok(v) => {
            if !v {
                eprintln!("not authorized!!");
                return (
                    axum::http::StatusCode::UNAUTHORIZED,
                    Bytes::from_static(b"unauthorized"),
                );
            }
        }
        Err(e) => {
            eprintln!("{:?}", e);
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Bytes::from_static(b"unauthorized"),
            );
        }
    }

    let c = match state.read().await.city_by_ip(ip) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{:?}", e);
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Bytes::from_static(b"fetching City failed"),
            );
        }
    };
    let mut b = BytesMut::new();
    match (c).encode(&mut b) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{:?}", e);
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Bytes::from_static(b"serialization failed"),
            );
        }
    };
    (axum::http::StatusCode::OK, b.into())
}

async fn get_asn_by_ip(
    axum::extract::Path(p): axum::extract::Path<String>,
    axum::extract::State(state): axum::extract::State<SharedState>,
    headers: axum::http::HeaderMap,
) -> impl axum::response::IntoResponse {
    let ip = if let Ok(p) = std::net::IpAddr::from_str(p.as_str()) {
        p
    } else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Bytes::from_static(b"invalid ip"),
        );
    };

    match authorized(Arc::clone(&state), headers).await {
        Ok(v) => {
            if !v {
                eprintln!("not authorized!!");
                return (
                    axum::http::StatusCode::UNAUTHORIZED,
                    Bytes::from_static(b"unauthorized"),
                );
            }
        }
        Err(e) => {
            eprintln!("{:?}", e);
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Bytes::from_static(b"unauthorized"),
            );
        }
    }

    let c = match state.read().await.asn_by_ip(ip) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{:?}", e);
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Bytes::from_static(b"fetching ASN failed"),
            );
        }
    };
    let mut b = BytesMut::new();
    match (c).encode(&mut b) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{:?}", e);
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Bytes::from_static(b"serialization failed"),
            );
        }
    };
    (axum::http::StatusCode::OK, b.into())
}

fn add_routes(state: SharedState) -> axum::Router<SharedState> {
    axum::Router::with_state(Arc::clone(&state))
        .route("/country/ip/:ip", axum::routing::get(get_country_by_ip))
        .route("/country/iso/:iso", axum::routing::get(get_country_by_iso))
        .route("/city/:ip", axum::routing::get(get_city_by_ip))
        .route("/asn/:ip", axum::routing::get(get_asn_by_ip))
}

#[tokio::main]
async fn main() {
    /* println!(env!("OUT_DIR")); */
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
                if !ss.init || &v == "updated" {
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
                            ss.cou_reader = Some(t);
                        }
                        Err(e) => {
                            eprintln!("Reader Countries Err: {:?}", e);
                        }
                    };

                    match geodb::reader_cities().await {
                        Ok(t) => {
                            ss.cit_reader = Some(t);
                        }
                        Err(e) => {
                            eprintln!("Reader Cities Err: {:?}", e);
                        }
                    };

                    match geodb::reader_asn().await {
                        Ok(t) => {
                            ss.asn_reader = Some(t);
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
    if let Ok(auth_file) = std::env::var("GEODB_AUTH_FILE") {
        let f = if let Ok(file) = std::fs::File::open(&auth_file) {
            file
        } else {
            eprintln!("opening file failed");
            return;
        };
        for line in io::BufReader::new(f).lines().flatten() {
            let mut writer = shared_state.write().await;
            writer.auth_keys.insert(line, ());
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
