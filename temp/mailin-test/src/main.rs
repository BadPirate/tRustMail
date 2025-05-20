use mailin::{Handler, Response};
use std::net::IpAddr;
use std::io::Error as IoError;

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

fn main() {
    // Just a test implementation
}
