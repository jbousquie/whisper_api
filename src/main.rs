use actix_web::{App, HttpServer};
mod handlers; // Importing handler module

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let listener = "127.0.0.1:8181";
    HttpServer::new(|| {
        App::new().service(handlers::transcribe) // Registering the transcribe endpoint
    })
    .bind(&listener)?
    .run()
    .await
}
