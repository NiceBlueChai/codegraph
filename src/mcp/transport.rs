use std::io::{self, BufRead, Write, stdin, stdout};
use std::net::{TcpStream, TcpListener};
use serde_json::Value;
use log::{debug, error, info};
use crate::mcp::protocol::{JsonRpcRequest, JsonRpcResponse};

/// Transport trait for MCP protocol communication
pub trait Transport {
    fn read_request(&self) -> Result<Option<JsonRpcRequest>, io::Error>;
    fn write_response(&self, response: &JsonRpcResponse) -> Result<(), io::Error>;
    fn write_notification(&self, method: &str, params: Value) -> Result<(), io::Error>;
}

/// Stdio transport for MCP protocol
pub struct StdioTransport;

impl StdioTransport {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for StdioTransport {
    /// Read a JSON-RPC request from stdin
    fn read_request(&self) -> Result<Option<JsonRpcRequest>, io::Error> {
        let stdin = stdin();
        let reader = stdin.lock();
        let mut lines = reader.lines();

        if let Some(line_result) = lines.next() {
            match line_result {
                Ok(line) => {
                    debug!("Received: {}", line);
                    match serde_json::from_str::<JsonRpcRequest>(&line) {
                        Ok(request) => Ok(Some(request)),
                        Err(e) => {
                            error!("Failed to parse request: {}", e);
                            Err(io::Error::new(io::ErrorKind::InvalidData, e))
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to read from stdin: {}", e);
                    Err(e)
                }
            }
        } else {
            // EOF
            Ok(None)
        }
    }

    /// Write a JSON-RPC response to stdout
    fn write_response(&self, response: &JsonRpcResponse) -> Result<(), io::Error> {
        let json = serde_json::to_string(response)?;
        debug!("Sending: {}", json);

        let stdout = stdout();
        let mut handle = stdout.lock();
        writeln!(handle, "{}", json)?;
        handle.flush()?;

        Ok(())
    }

    /// Write a JSON-RPC notification (no id)
    fn write_notification(&self, method: &str, params: Value) -> Result<(), io::Error> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        let json = serde_json::to_string(&notification)?;
        debug!("Sending notification: {}", json);

        let stdout = stdout();
        let mut handle = stdout.lock();
        writeln!(handle, "{}", json)?;
        handle.flush()?;

        Ok(())
    }
}

/// TCP socket transport for daemon mode
pub struct TcpTransport {
    stream: std::sync::Mutex<TcpStream>,
}

impl TcpTransport {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream: std::sync::Mutex::new(stream),
        }
    }

    /// Connect to a TCP server
    pub fn connect(addr: &str) -> Result<Self, io::Error> {
        let stream = TcpStream::connect(addr)?;
        Ok(Self::new(stream))
    }
}

impl Transport for TcpTransport {
    fn read_request(&self) -> Result<Option<JsonRpcRequest>, io::Error> {
        let stream = self.stream.lock().map_err(|e| {
            io::Error::new(io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;

        let mut reader = io::BufReader::new(&*stream);
        let mut line = String::new();

        match reader.read_line(&mut line) {
            Ok(0) => Ok(None), // EOF
            Ok(_) => {
                debug!("Received: {}", line.trim());
                match serde_json::from_str::<JsonRpcRequest>(line.trim()) {
                    Ok(request) => Ok(Some(request)),
                    Err(e) => {
                        error!("Failed to parse request: {}", e);
                        Err(io::Error::new(io::ErrorKind::InvalidData, e))
                    }
                }
            }
            Err(e) => Err(e),
        }
    }

    fn write_response(&self, response: &JsonRpcResponse) -> Result<(), io::Error> {
        let json = serde_json::to_string(response)?;
        debug!("Sending: {}", json);

        let mut stream = self.stream.lock().map_err(|e| {
            io::Error::new(io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;

        writeln!(stream, "{}", json)?;
        stream.flush()?;

        Ok(())
    }

    fn write_notification(&self, method: &str, params: Value) -> Result<(), io::Error> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        let json = serde_json::to_string(&notification)?;
        debug!("Sending notification: {}", json);

        let mut stream = self.stream.lock().map_err(|e| {
            io::Error::new(io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;

        writeln!(stream, "{}", json)?;
        stream.flush()?;

        Ok(())
    }
}

/// Daemon configuration
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub port: u16,
    pub db_path: String,
    pub project_root: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            port: 9540,
            db_path: ".codegraph/codegraph.db".to_string(),
            project_root: ".".to_string(),
        }
    }
}

/// Daemon server that listens for TCP connections
pub struct DaemonServer {
    config: DaemonConfig,
}

impl DaemonServer {
    pub fn new(config: DaemonConfig) -> Self {
        Self { config }
    }

    /// Start the daemon server
    pub fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let addr = format!("127.0.0.1:{}", self.config.port);
        let listener = TcpListener::bind(&addr)?;
        info!("Daemon listening on {}", addr);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    info!("New connection from {}", stream.peer_addr()?);
                    let transport = TcpTransport::new(stream);
                    // Handle connection in a new thread
                    std::thread::spawn(move || {
                        if let Err(e) = Self::handle_connection(&transport) {
                            error!("Connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Connection failed: {}", e);
                }
            }
        }

        Ok(())
    }

    fn handle_connection(transport: &TcpTransport) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            match transport.read_request() {
                Ok(Some(request)) => {
                    // Process request
                    let response = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id.clone(),
                        result: Some(serde_json::json!({"status": "ok"})),
                        error: None,
                    };
                    transport.write_response(&response)?;
                }
                Ok(None) => {
                    debug!("Client disconnected");
                    break;
                }
                Err(e) => {
                    error!("Read error: {}", e);
                    break;
                }
            }
        }
        Ok(())
    }
}

/// Proxy that connects stdin/stdout to a daemon
pub struct DaemonProxy {
    daemon_addr: String,
}

impl DaemonProxy {
    pub fn new(daemon_addr: &str) -> Self {
        Self {
            daemon_addr: daemon_addr.to_string(),
        }
    }

    /// Run the proxy
    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Connecting to daemon at {}", self.daemon_addr);
        let transport = TcpTransport::connect(&self.daemon_addr)?;

        // Forward stdin -> daemon (in a separate thread)
        let daemon_addr = self.daemon_addr.clone();

        // Read from stdin and forward to daemon
        std::thread::spawn(move || {
            let stdin = stdin();
            let reader = stdin.lock();
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        if line.is_empty() {
                            continue;
                        }
                        if let Ok(transport) = TcpTransport::connect(&daemon_addr) {
                            // Parse as request and forward
                            if let Ok(request) = serde_json::from_str::<JsonRpcRequest>(&line) {
                                let _ = transport.write_response(&JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: request.id,
                                    result: Some(serde_json::Value::String(line)),
                                    error: None,
                                });
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Forward daemon -> stdout
        let stdout = stdout();
        let mut stdout_handle = stdout.lock();
        loop {
            match transport.read_request() {
                Ok(Some(request)) => {
                    // Forward the raw JSON to stdout
                    if let Ok(json) = serde_json::to_string(&request) {
                        writeln!(stdout_handle, "{}", json)?;
                        stdout_handle.flush()?;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    error!("Daemon connection error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }
}
