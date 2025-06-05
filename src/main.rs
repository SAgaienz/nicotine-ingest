use actix_http::Error;
use actix_web::{web, App, HttpResponse, HttpServer, Responder, HttpRequest, middleware};
use serde::{Deserialize, Serialize};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use chrono::{Utc, Duration};
use bcrypt::{hash, verify, DEFAULT_COST};
use std::error::Error as stdErr;
use std::sync::Mutex;
use std::collections::HashMap;
use log::{error, info};
use std::sync::LazyLock;
use std::env;
use futures::stream;
use influxdb2::models::DataPoint;
use influxdb2::Client;


// Data structures
#[derive(Serialize, Deserialize, Clone)]
struct User {
    id: u32,
    username: String,
    password_hash: String,
}

#[derive(Serialize, Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize, Deserialize)]
struct WriteResponse {
    success: bool,
    msg: String,
}


#[derive(Serialize, Deserialize)]
struct EventData {
    measurement: String,
    fields: DataField,
    form: String
}

#[derive(Serialize, Deserialize)]
struct DataField {
    mg: f64,
    count: i64,
}

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: i64,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
}

// Application state
struct AppState {
    users: Mutex<HashMap<String, User>>,
}

// Web Auth
const TOKEN_DURATION: i64 = 24 * 60 * 60; // 24 hours in seconds
const JWT_SECRET: LazyLock<String> = LazyLock::new(|| {
    std::env::var("JWT_SECRET").unwrap()
});


// Middleware to validate JWT
async fn validate_token(req: &HttpRequest) -> Result<User, HttpResponse> {
    let auth_header = match req.headers().get("Authorization") {
        Some(header) => header.to_str().unwrap_or(""),
        None => {
            error!("Missing Authorization header");
            return Err(HttpResponse::Unauthorized().json("Missing Authorization header"));
        },
    };

    if !auth_header.starts_with("Bearer ") {
        return Err(HttpResponse::Unauthorized().json("Invalid Authorization header"));
    }

    let token = auth_header.trim_start_matches("Bearer ");
    let validation = Validation::default();
    let token_data = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET.as_ref()),
        &validation
    ) {
        Ok(data) => data,
        Err(_) => return Err(HttpResponse::Unauthorized().json("Invalid token")),
    };

    let state = req.app_data::<web::Data<AppState>>().unwrap();
    let users = state.users.lock().unwrap();
    match users.get(&token_data.claims.sub) {
        Some(user) => Ok(user.clone()),
        None => Err(HttpResponse::Unauthorized().json("User not found")),
    }
}

async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("API is running")
}

fn register(state: &web::Data<AppState>) -> Result<(), Error> {
    let mut users = state.users.lock().unwrap();

    let username_key = "NICO_USERNAME";
    let password_key = "NICO_PASSWORD";

    let username = match env::var(username_key) {
        Ok(t) => t,
        Err(e) =>  {
            panic!("{}", format!("'NICO_USERNAME' is not defined: {}", e));
        }
    };

    let password = match env::var(password_key) {
        Ok(t) => t,
        Err(e) =>  {
            panic!("{}", format!("'NICO_PASSWORD' is not defined: {}", e));
        }
    };
    

    let password_hash = hash(&password, DEFAULT_COST).unwrap();
    
    let new_user = User {
        id: 1,
        username: username,
        password_hash: password_hash,
    };

    users.insert(new_user.username.clone(), new_user);
    Ok(())
}

async fn login(state: web::Data<AppState>, login: web::Json<LoginRequest>) -> impl Responder {
    let users = state.users.lock().unwrap();

    let user = match users.get(&login.username) {
        Some(user) => user,
        None => return HttpResponse::Unauthorized().json("Invalid credentials"),
    };

    let valid = match verify(&login.password, &user.password_hash) {
        Ok(valid) => valid,
        Err(_) => return HttpResponse::InternalServerError().json("Verification error"),
    };

    if !valid {
        return HttpResponse::Unauthorized().json("Invalid credentials");
    }

    let claims = Claims {
        sub: user.username.clone(),
        exp: (Utc::now() + Duration::seconds(TOKEN_DURATION)).timestamp(),
    };

    let token = match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET.as_ref()),
    ) {
        Ok(token) => token,
        Err(_) => return HttpResponse::InternalServerError().json("Failed to generate token"),
    };

    HttpResponse::Ok().json(LoginResponse { token })

}

async fn check_influx_config() -> Result<Client, Box<dyn stdErr>> {
    let host = match std::env::var("INFLUXDB_HOST") {
        Ok(v) => v,
        Err(e) => return Err(Box::new(e))
    };

    let org = match std::env::var("INFLUXDB_ORG") {
        Ok(v) => v,
        Err(e) => return Err(Box::new(e))
    };

    let token = match std::env::var("INFLUXDB_TOKEN") {
        Ok(v) => v,
        Err(e) => return Err(Box::new(e))
    };

    let client = Client::new(host, org, token);

    let _client_health = match client.health().await {
        Ok(v) => v,
        Err(e) => return Err(Box::new(e))
    };

    Ok(client)
}

async fn write_data(req: HttpRequest,  data: web::Json<EventData>) -> impl Responder {
    match validate_token(&req).await {
        Ok(_) => true,
        Err(response) => return response,
    };

    let client = match check_influx_config().await {
        Ok(t) => t,
        Err(e) => return HttpResponse::InternalServerError()
        .json(serde_json::json!({ "error": format!("{}", e) }))
    };

    let bucket = match std::env::var("INFLUXDB_BUCKET") {
        Ok(v) => v,
        Err(e) => return HttpResponse::InternalServerError()
            .json(serde_json::json!({"error": format!("{}", e)}))
    };
    
    let mg = data.fields.mg;
    let count = data.fields.count as f64;

    let builder = DataPoint::builder( &data.measurement)
        .field("mg", mg)
        .field("count", count)
        .tag("form", &data.form)
        .build();

    let data_point = match builder {
        Ok(dp) => vec![dp],
        Err(e) => return HttpResponse::BadRequest()
                .json(serde_json::json!({ "error": format!("Invalid data point: {}", e) }))
    };
        
    match client.write(&bucket, stream::iter(data_point)).await {
        Ok(_) => return HttpResponse::Ok().json(serde_json::json!({ "status": "success" })),
        Err(e) => return HttpResponse::InternalServerError()
            .json(serde_json::json!({ "error": format!("Failed to write to InfluxDB: {}", e) })),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    info!("Started server");

    match check_influx_config().await {
        Ok(t) => t,
        Err(e) => panic!("failed to start API server: {}", e)
    };
    
    let state = web::Data::new(AppState {
        users: Mutex::new(HashMap::new()),
    });

    let _ =  register(&state).unwrap();

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(middleware::Logger::default())
            // .route("/register", web::post().to(register))
            .route("/health", web::get().to(health_check))
            .route("/login", web::post().to(login))
            .route("/write_data", web::post().to(write_data))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}