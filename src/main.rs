mod handlers;

use actix_web::{App, HttpServer};
use handlers::transcribe;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Starting Whisper API server on http://0.0.0.0:8181");
    let timeout = std::time::Duration::from_secs(480);
    let keep_alive = std::time::Duration::from_secs(480);

    HttpServer::new(|| App::new().service(transcribe))
        .bind("0.0.0.0:8080")?
        .client_disconnect_timeout(timeout)
        .keep_alive(keep_alive)
        .run()
        .await
}
