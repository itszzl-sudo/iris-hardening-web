use std::sync::Arc;
use warp::{Filter, Rejection, Reply};

use crate::database::{Database, ConfigStatus};
use crate::nginx_gen::NginxGenerator;

#[derive(Debug, serde::Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn error_response(msg: String) -> Box<dyn Reply> {
    Box::new(warp::reply::with_status(
        warp::reply::json(&ApiResponse::<()> {
            success: false,
            data: None,
            error: Some(msg),
        }),
        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
    ))
}

fn not_found_response(msg: String) -> Box<dyn Reply> {
    Box::new(warp::reply::with_status(
        warp::reply::json(&ApiResponse::<()> {
            success: false,
            data: None,
            error: Some(msg),
        }),
        warp::http::StatusCode::NOT_FOUND,
    ))
}

fn success_response<T: serde::Serialize>(data: T) -> Box<dyn Reply> {
    Box::new(warp::reply::json(&ApiResponse {
        success: true,
        data: Some(data),
        error: None,
    }))
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateDomainRequest {
    pub domain: String,
    pub nginx_port: Option<u16>,
    pub gateway_port: Option<u16>,
    pub gateway_host: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct UpdateConfigRequest {
    pub domain: String,
    pub nginx_config: Option<String>,
    pub status: Option<String>,
}

pub fn routes(
    db: Arc<Database>,
    gateway_url: String,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let db1 = db.clone();
    let db2 = db.clone();
    let db3 = db.clone();
    let db4 = db.clone();
    let db5 = db.clone();
    let db6 = db.clone();
    let db7 = db.clone();
    let gw_url = gateway_url;

    // List domains - GET /api/domains
    let list = warp::path("api")
        .and(warp::path("domains"))
        .and(warp::path::end())
        .and(warp::get())
        .map(move || {
            match db1.get_all_domains() {
                Ok(domains) => success_response(domains),
                Err(e) => error_response(e.to_string()),
            }
        });

    // Create domain - POST /api/domains
    let create = warp::path("api")
        .and(warp::path("domains"))
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json())
        .map(move |body: CreateDomainRequest| {
            let nginx_port = body.nginx_port.unwrap_or(80);
            let gateway_port = body.gateway_port.unwrap_or(9001);
            let gateway_host = body.gateway_host.as_deref();
            match db2.create_domain(&body.domain, nginx_port, gateway_port, gateway_host) {
                Ok(config) => success_response(config),
                Err(e) => error_response(e.to_string()),
            }
        });

    // Get domain - GET /api/domains/:domain
    let get = warp::path("api")
        .and(warp::path("domains"))
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .and(warp::get())
        .map(move |domain: String| {
            match db3.get_domain(&domain) {
                Ok(Some(config)) => success_response(config),
                Ok(None) => not_found_response("Domain not found".to_string()),
                Err(e) => error_response(e.to_string()),
            }
        });

    // Delete domain - DELETE /api/domains/:domain
    let delete = warp::path("api")
        .and(warp::path("domains"))
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .and(warp::delete())
        .map(move |domain: String| {
            match db4.delete_domain(&domain) {
                Ok(true) => success_response("Deleted".to_string()),
                Ok(false) => not_found_response("Domain not found".to_string()),
                Err(e) => error_response(e.to_string()),
            }
        });

    // Generate nginx config - POST /api/domains/:domain/generate
    let generate = warp::path("api")
        .and(warp::path("domains"))
        .and(warp::path::param::<String>())
        .and(warp::path("generate"))
        .and(warp::path::end())
        .and(warp::post())
        .map(move |domain: String| {
            let config = match db5.get_domain(&domain) {
                Ok(Some(c)) => c,
                Ok(None) => return not_found_response("Domain not found".to_string()),
                Err(e) => return error_response(e.to_string()),
            };

            let gateway_host = config.gateway_host.unwrap_or_else(|| "127.0.0.1".to_string());
            let ctx = crate::nginx_gen::NginxContext {
                domain: config.domain.clone(),
                nginx_port: config.nginx_port,
                gateway_host,
                gateway_port: config.gateway_port,
                wasm_validity_hours: 24,
            };

            let nginx_config = NginxGenerator::generate_config(&ctx);
            if let Err(e) = db5.update_nginx_config(&domain, &nginx_config) {
                return error_response(e.to_string());
            }

            success_response(nginx_config)
        });

    // Get nginx config - GET /api/domains/:domain/config
    let get_config = warp::path("api")
        .and(warp::path("domains"))
        .and(warp::path::param::<String>())
        .and(warp::path("config"))
        .and(warp::path::end())
        .and(warp::get())
        .map(move |domain: String| {
            match db6.get_domain(&domain) {
                Ok(Some(config)) => {
                    if let Some(cfg) = config.nginx_config {
                        success_response(cfg)
                    } else {
                        not_found_response("No nginx config generated yet".to_string())
                    }
                }
                Ok(None) => not_found_response("Domain not found".to_string()),
                Err(e) => error_response(e.to_string()),
            }
        });

    // Update status - PATCH /api/domains/:domain/status
    let update_status = warp::path("api")
        .and(warp::path("domains"))
        .and(warp::path::param::<String>())
        .and(warp::path("status"))
        .and(warp::path::end())
        .and(warp::patch())
        .and(warp::body::json())
        .map(move |domain: String, body: UpdateConfigRequest| {
            let status = match body.status.as_deref() {
                Some("synced") => ConfigStatus::Synced,
                Some("failed") => ConfigStatus::Failed,
                _ => ConfigStatus::Pending,
            };
            match db7.update_status(&domain, status) {
                Ok(_) => success_response("Status updated".to_string()),
                Err(e) => error_response(e.to_string()),
            }
        });

    // NJS script - GET /api/njs
    let njs = warp::path("api")
        .and(warp::path("njs"))
        .and(warp::path::end())
        .and(warp::get())
        .map(move || warp::reply::html(NginxGenerator::generate_njs_script(&gw_url)));

    // Index - GET /
    let index = warp::path::end()
        .and(warp::get())
        .map(|| warp::reply::html(include_str!("../web/index.html")));

    list.or(create)
        .or(get)
        .or(delete)
        .or(generate)
        .or(get_config)
        .or(update_status)
        .or(njs)
        .or(index)
}