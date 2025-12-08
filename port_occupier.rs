use std::net::TcpListener;
use std::io::{Read, Write};

// Simple HTTP server that occupies a port
fn main() {
    let listener = TcpListener::bind("127.0.0.1:9999").expect("Could not bind to port 9999");
    println!("Occupying port 9999...");
    
    // Accept one connection to keep the port busy
    if let Ok((mut stream, addr)) = listener.accept() {
        println!("Connection from {:?}", addr);
        let mut buffer = [0; 512];
        stream.read(&mut buffer).unwrap();
        println!("Received request");
        
        let response = "HTTP/1.1 404 NOT FOUND\r\n\r\n";
        stream.write(response.as_bytes()).unwrap();
        stream.flush().unwrap();
    }
    
    // Keep the listener alive to maintain the port occupation
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1000));
    }
}