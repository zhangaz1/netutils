extern crate rustls;
extern crate webpki_roots;

use rustls::Session;

use std::env;
use std::io::{stderr, stdout, Read, Write};
use std::net::TcpStream;
use std::process;
use std::str;
use std::sync::Arc;

fn main() {
    if let Some(url) = env::args().nth(1) {
        let (scheme, reference) = url.split_at(url.find(':').unwrap_or(0));
        if scheme == "http" || scheme == "https" {
            let https = scheme == "https";

            let mut parts = reference.split('/').skip(2); //skip first two slashes
            let remote = parts.next().unwrap_or("");
            let mut remote_parts = remote.split(':');
            let host = remote_parts.next().unwrap_or("");
            let port = remote_parts.next().unwrap_or("").parse::<u16>().unwrap_or(if https { 443} else { 80 });
            let mut path = parts.next().unwrap_or("").to_string();
            for part in parts {
                path.push('/');
                path.push_str(part);
            }

            write!(stderr(), "* Connecting to {}:{}\n", host, port).unwrap();

            let mut stream = TcpStream::connect((host, port)).unwrap();

            write!(stderr(), "* Requesting /{}\n", path).unwrap();

            let request = format!("GET /{} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", path, host);
            let mut response = Vec::new();

            if https {
                let mut config = rustls::ClientConfig::new();
                config.root_store.add_trust_anchors(&webpki_roots::ROOTS[..]);
                let rc_config = Arc::new(config);

                let mut client = rustls::ClientSession::new(&rc_config, host);
                client.write(request.as_bytes()).unwrap();
                client.write_tls(&mut stream).unwrap();

                write!(stderr(), "* Waiting for response\n").unwrap();

                'reading:loop {
                    while client.wants_read() {
                        client.read_tls(&mut stream).unwrap();
                        client.process_new_packets().unwrap();
                        while client.wants_write() {
                            client.write_tls(&mut stream).unwrap();
                        }
                    }

                    let mut buf = [0; 65536];
                    loop {
                        let bytes = client.read(&mut buf).unwrap();
                        if bytes < buf.len() {
                            response.append(&mut buf[..bytes].to_vec());
                            break 'reading;
                        }
                        response.append(&mut buf.to_vec());
                    }
                }
            } else {
                stream.write(request.as_bytes()).unwrap();
                stream.flush().unwrap();

                write!(stderr(), "* Waiting for response\n").unwrap();

                loop {
                    let mut buf = [0; 65536];
                    let count = stream.read(&mut buf).unwrap();
                    if count == 0 {
                        break;
                    }
                    response.extend_from_slice(&buf[.. count]);
                }
            }

            write!(stderr(), "* Received {} bytes\n", response.len()).unwrap();

            let mut header_end = 0;
            while header_end < response.len() {
                if response[header_end..].starts_with(b"\r\n\r\n") {
                    break;
                }
                header_end += 1;
            }

            for line in unsafe { str::from_utf8_unchecked(&response[..header_end]) }.lines() {
                write!(stderr(), "> {}\n", line).unwrap();
            }

            stdout().write(&response[header_end + 4 ..]).unwrap();
        } else {
            write!(stderr(), "wget: unknown scheme '{}'\n", scheme).unwrap();
            process::exit(1);
        }
    } else {
        write!(stderr(), "wget: http://host:port/path\n").unwrap();
        process::exit(1);
    }
}
