use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::env;

#[derive(Serialize, Deserialize)]
struct Status {
    status: String,
    timestamp: String,
    message: String,
}

#[derive(Serialize, Deserialize)]
struct SearchRequest {
    q: String,
}

#[derive(Serialize, Deserialize)]
struct SearchResult {
    hits: Vec<Hit>,
}

#[derive(Serialize, Deserialize)]
struct Hit {
    id: String,
    title: String,
    date: String,
    description: String,
    reading_time: String,
    slug: String,
}

#[derive(Clone)]
struct MeiliConfig {
    url: String,
    api_key: String,
}

#[get("/")]
async fn status() -> impl Responder {
    let response = Status {
        status: "OK".to_string(),
        timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        message: "API is running".to_string(),
    };

    HttpResponse::Ok().json(response)
}

#[post("/search")]
async fn search_handler(
    body: web::Json<SearchRequest>,
    client: web::Data<Arc<Client>>,
    config: web::Data<MeiliConfig>,
) -> impl Responder {
    if body.q.trim().is_empty() {
        return HttpResponse::BadRequest().body("Query cannot be empty");
    }

    let url = format!("{}/indexes/posts/search", config.url);
    let payload = json!({
        "q": body.q,
        "matchingStrategy": "all",
        "attributesToRetrieve": ["id", "title", "description", "slug", "date", "reading_time"],
        "attributesToHighlight": ["*"],
        "sort": ["date:desc"],
    });

    let response = match client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("Request error: {}", e);
            return HttpResponse::InternalServerError().body("Search server error");
        }
    };

    match response.json::<SearchResult>().await {
        Ok(result) => HttpResponse::Ok().json(result),
        Err(e) => {
            eprintln!("Failed to parse response: {}", e);
            HttpResponse::InternalServerError().body("Invalid response format")
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();

    let config = MeiliConfig {
        url: env_var("MEILI_URL").unwrap_or_else(|e| {
            eprintln!("{}", e);
            std::process::exit(1);
        }),
        api_key: env_var("MEILI_MASTER_KEY").unwrap_or_else(|e| {
            eprintln!("{}", e);
            std::process::exit(1);
        }),
    };

    let client = Arc::new(Client::new());

    println!("✅ Server running on http://0.0.0.0:8080");
    println!("🔗 Connected to Meilisearch at {}", config.url);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(client.clone()))
            .app_data(web::Data::new(config.clone()))
            .service(status)
            .service(search_handler)
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}

fn env_var(name: &str) -> Result<String, String> {
    env::var(name).map_err(|e| format!("Environment variable {} not set: {}", name, e))
}
