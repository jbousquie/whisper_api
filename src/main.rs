mod handlers;

use actix_web::{App, HttpServer};
use handlers::transcribe;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Starting Whisper API server on http://0.0.0.0:8080");
    
    HttpServer::new(|| {
        App::new()
            .service(transcribe)
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
