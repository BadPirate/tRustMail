use mailin::{Handler, Response};
use std::io::Error as IoError;
use std::net::{IpAddr, SocketAddr};

struct ExampleHandler;

impl Handler for ExampleHandler {
    fn helo(&mut self, ip: IpAddr, domain: &str) -> Response {
        println!("HELO from {} domain {}", ip, domain);
        Response::custom(250, "OK".to_string())
    }
    
    fn mail(&mut self, ip: IpAddr, from: &str, extension: &str) -> Response {
        println!("MAIL FROM:<{}> with {} by {}", from, extension, ip);
        Response::custom(250, "OK".to_string())
    }
    
    fn rcpt(&mut self, to: &str) -> Response {
        println!("RCPT TO:<{}>", to);
        Response::custom(250, "OK".to_string())
    }
    
    fn data(&mut self, buf: &[u8]) -> Result<(), IoError> {
        println!("DATA: {} bytes received", buf.len());
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Checking mailin API structure");
    
    // Example address
    let addr = SocketAddr::from(([127, 0, 0, 1], 25));
    let handler = ExampleHandler;
    
    // Check crate's usage examples
    std::fs::read_to_string("/home/runner/workspace/.local/share/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/mailin-0.6.5/examples/server.rs")
        .map(|s| println!("Example: {}", s))
        .unwrap_or_else(|e| println!("Error reading example: {}", e));
        
    Ok(())
}
