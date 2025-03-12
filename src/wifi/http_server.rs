use core::fmt::{Debug, Display};
use edge_http::io::server::{Connection, DefaultServer, Handler};
use edge_http::io::Error;
use edge_http::Method;
use edge_nal::TcpBind;
use edge_nal_embassy::{Tcp, TcpBuffers};
use embedded_io_async::{Read, Write};
use esp_println::println;

pub async fn run_http_server(stack: &embassy_net::Stack<'_>) -> Result<(), ()> {
    let addr = "1.1.1.1:8080";
    println!("Running HTTP server on {addr}");

    let buffers = TcpBuffers::<4, 2048, 2048>::new();
    let tcp = Tcp::new(*stack, &buffers);
    let acceptor = match tcp.bind(addr.parse().unwrap()).await {
        Ok(a) => a,
        Err(e) => {
            println!("Failed to bind to {addr}: {:?}", e);
            return Err(());
        }
    };

    println!("HTTP server bound to {addr}, now accepting connections");

    let result = DefaultServer::run(&mut DefaultServer::new(), None, acceptor, HttpHandler).await;

    match result {
        Ok(_) => {
            println!("HTTP server has completed normally");
            Ok(())
        }
        Err(e) => {
            println!("HTTP server error: {:?}", e);
            Err(())
        }
    }
}

struct HttpHandler;

impl Handler for HttpHandler {
    type Error<E>
        = Error<E>
    where
        E: Debug;

    async fn handle<T, const N: usize>(
        &self,
        _task_id: impl Display + Copy,
        conn: &mut Connection<'_, T, N>,
    ) -> Result<(), Self::Error<T::Error>>
    where
        T: Read + Write,
    {
        let headers = conn.headers()?;
        println!(
            "HTTP request received, Method: {:?}, Path: {:?}",
            headers.method, headers.path
        );

        match (headers.method, headers.path) {
            (Method::Get, "/login") => {
                conn.initiate_response(200, Some("OK"), &[("Content-Type", "text/html")])
                    .await?;
                let html_content = include_str!("login.html");
                conn.write_all(html_content.as_bytes()).await?;
            }
            (Method::Post, "/login") => {
                // Handle login form submission
                println!("Processing POST request to /login");

                // Check Content-Length header to know how much data to read
                let content_length = headers
                    .headers
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case("Content-Length"))
                    .and_then(|(_, value)| value.parse::<usize>().ok())
                    .unwrap_or(0);

                println!("Content-Length: {}", content_length);

                if content_length == 0 {
                    println!("No content to read");
                    conn.initiate_response(400, Some("Bad Request"), &[])
                        .await?;
                    conn.write_all(b"No form data provided").await?;
                    return Ok(());
                }

                // Allocate buffer based on content length (with a reasonable maximum)
                let max_size = core::cmp::min(content_length, 1024);
                let mut buffer = [0u8; 1024]; // Fixed size buffer
                let mut total_read = 0;

                // Read the request body
                println!("Reading request body...");
                while total_read < max_size {
                    match conn.read(&mut buffer[total_read..]).await {
                        Ok(0) => break, // End of data
                        Ok(n) => {
                            total_read += n;
                            println!("Read {} bytes, total: {}", n, total_read);
                        }
                        Err(e) => {
                            println!("Error reading request body: {:?}", e);
                            conn.initiate_response(500, Some("Internal Server Error"), &[])
                                .await?;
                            conn.write_all(b"Error reading form data").await?;
                            return Ok(());
                        }
                    }
                }

                println!("Total bytes read: {}", total_read);

                // Convert to string and parse
                if let Ok(form_str) = core::str::from_utf8(&buffer[..total_read]) {
                    println!("Received form data: {}", form_str);

                    // Parse form data (application/x-www-form-urlencoded format)
                    let mut username = "";
                    let mut password = "";

                    for pair in form_str.split('&') {
                        println!("Processing pair: {}", pair);
                        let mut parts = pair.split('=');
                        if let Some(key) = parts.next() {
                            if let Some(value) = parts.next() {
                                println!("Found key: {}, value: {}", key, value);
                                if key == "username" {
                                    username = value;
                                } else if key == "password" {
                                    password = value;
                                }
                            }
                        }
                    }

                    println!("Parsed username: {}", username);
                    println!("Parsed password: {}", password);

                    // Send success response
                    conn.initiate_response(200, Some("OK"), &[("Content-Type", "text/html")])
                        .await?;
                    conn.write_all(b"<html><body><h1>Login Successful</h1><p>Username: ")
                        .await?;
                    conn.write_all(username.as_bytes()).await?;
                    conn.write_all(b"</p><p>Password: ").await?;
                    conn.write_all(password.as_bytes()).await?;
                    conn.write_all(b"</p></body></html>").await?;
                } else {
                    // Cannot parse as UTF-8
                    println!("Failed to parse form data as UTF-8");
                    conn.initiate_response(400, Some("Bad Request"), &[])
                        .await?;
                    conn.write_all(b"Invalid form data encoding").await?;
                }
            }
            (Method::Get, "/") => {
                conn.initiate_response(200, Some("OK"), &[("Content-Type", "text/plain")])
                    .await?;
                conn.write_all(b"Welcome to the root page").await?;
            }
            _ => {
                conn.initiate_response(404, Some("Not Found"), &[]).await?;
                conn.write_all(b"Not Found").await?;
            }
        }

        conn.flush().await?;
        println!("HTTP response sent successfully");
        Ok(())
    }
}
