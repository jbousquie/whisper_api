use std::net::TcpListener;
use std::process;

// Script pour tester la gestion d'erreur lorsqu'un port est déjà utilisé
fn main() {
    let port = 8181; // Port par défaut de l'API Whisper
    let address = format!("127.0.0.1:{}", port);
    
    // Essayons de réserver le port pour simuler un conflit
    match TcpListener::bind(&address) {
        Ok(_) => {
            println!("Port {} est disponible - occupons-le pour le test", port);
            println!("Démarrer le service Whisper API maintenant avec ce port...");
            println!("(Le port est réservé par ce programme de test)");
            
            // Attendre un peu pour laisser le temps de tenter le démarrage du service
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
        Err(e) => {
            println!("Erreur lors de la réservation du port {}: {}", port, e);
            process::exit(1);
        }
    }
    
    println!("Test terminé - le port {} est maintenant libéré", port);
}