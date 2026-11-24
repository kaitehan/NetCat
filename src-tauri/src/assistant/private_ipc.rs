use std::{path::PathBuf, sync::Arc};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::AppError;

const MAX_REQUEST_BYTES: u64 = 1024 * 1024;

pub trait PrivateIpcService: Send + Sync {
    fn call(&self, method: &str, params: &Value) -> Result<Value, AppError>;
}

#[derive(Deserialize)]
struct IpcRequest {
    token: String,
    method: String,
    #[serde(default)]
    params: Value,
}

fn response_for(line: &str, token: &str, service: &dyn PrivateIpcService) -> Value {
    let request = match serde_json::from_str::<IpcRequest>(line) {
        Ok(request) => request,
        Err(error) => {
            return json!({
                "error": {
                    "code": "ASSISTANT_IPC_INVALID_REQUEST",
                    "message": "私有 IPC 请求无效",
                    "detail": error.to_string(),
                }
            });
        }
    };
    if !constant_time_eq(request.token.as_bytes(), token.as_bytes()) {
        return json!({
            "error": {
                "code": "ASSISTANT_IPC_UNAUTHORIZED",
                "message": "私有 IPC 认证失败",
            }
        });
    }
    match service.call(&request.method, &request.params) {
        Ok(result) => json!({ "result": result }),
        Err(error) => json!({ "error": error }),
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}

#[cfg(unix)]
pub struct PrivateIpcServer {
    path: PathBuf,
    stopping: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

#[cfg(unix)]
impl PrivateIpcServer {
    pub fn start(
        path: PathBuf,
        token: String,
        service: Arc<dyn PrivateIpcService>,
    ) -> Result<Self, AppError> {
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::net::UnixListener;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ipc_error)?;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(ipc_error(error)),
        }
        let listener = UnixListener::bind(&path).map_err(ipc_error)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(ipc_error)?;
        listener.set_nonblocking(true).map_err(ipc_error)?;
        let stopping = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let thread_stopping = Arc::clone(&stopping);
        let thread = std::thread::spawn(move || {
            while !thread_stopping.load(std::sync::atomic::Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => handle_unix_connection(stream, &token, service.as_ref()),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            path,
            stopping,
            thread: Some(thread),
        })
    }

    pub fn stop(&mut self) -> Result<(), AppError> {
        use std::os::unix::net::UnixStream;

        self.stopping
            .store(true, std::sync::atomic::Ordering::Release);
        let _ = UnixStream::connect(&self.path);
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| AppError::new("ASSISTANT_IPC_ERROR", "智奈私有 IPC 线程异常退出"))?;
        }
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(ipc_error(error)),
        }
    }
}

#[cfg(unix)]
impl Drop for PrivateIpcServer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(windows)]
pub struct PrivateIpcServer {
    stopping: Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

#[cfg(windows)]
impl PrivateIpcServer {
    pub fn start(
        path: PathBuf,
        token: String,
        service: Arc<dyn PrivateIpcService>,
    ) -> Result<Self, AppError> {
        let stopping = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let thread_stopping = Arc::clone(&stopping);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            let result = match runtime {
                Ok(runtime) => runtime.block_on(run_windows_pipe(
                    path,
                    token,
                    service,
                    thread_stopping,
                    ready_tx,
                )),
                Err(error) => {
                    let _ = ready_tx.send(Err(error.to_string()));
                    return;
                }
            };
            let _ = result;
        });
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                stopping,
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(ipc_error(error))
            }
            Err(error) => {
                let _ = thread.join();
                Err(ipc_error(error))
            }
        }
    }

    pub fn stop(&mut self) -> Result<(), AppError> {
        self.stopping
            .store(true, std::sync::atomic::Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| AppError::new("ASSISTANT_IPC_ERROR", "智奈私有 IPC 线程异常退出"))?;
        }
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for PrivateIpcServer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(windows)]
async fn run_windows_pipe(
    path: PathBuf,
    token: String,
    service: Arc<dyn PrivateIpcService>,
    stopping: Arc<std::sync::atomic::AtomicBool>,
    ready: std::sync::mpsc::Sender<Result<(), String>>,
) -> Result<(), AppError> {
    use std::ffi::c_void;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::windows::named_pipe::ServerOptions;
    use windows::{
        core::w,
        Win32::{
            Foundation::{LocalFree, BOOL, HLOCAL},
            Security::{
                Authorization::{
                    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
                },
                PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
            },
        },
    };

    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            w!("D:P(A;;GA;;;OW)"),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
        .map_err(ipc_error)?;
    }
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: BOOL(0),
    };
    let create = || unsafe {
        ServerOptions::new().create_with_security_attributes_raw(
            &path,
            (&mut attributes as *mut SECURITY_ATTRIBUTES).cast::<c_void>(),
        )
    };
    let mut server = match create() {
        Ok(server) => {
            let _ = ready.send(Ok(()));
            server
        }
        Err(error) => {
            let _ = ready.send(Err(error.to_string()));
            unsafe {
                LocalFree(Some(HLOCAL(descriptor.0)));
            }
            return Err(ipc_error(error));
        }
    };
    while !stopping.load(std::sync::atomic::Ordering::Acquire) {
        let connected = tokio::select! {
            result = server.connect() => result.map(|_| true),
            _ = tokio::time::sleep(std::time::Duration::from_millis(25)) => Ok(false),
        }
        .map_err(ipc_error)?;
        if !connected {
            continue;
        }
        let mut input = Vec::new();
        (&mut server)
            .take(MAX_REQUEST_BYTES)
            .read_to_end(&mut input)
            .await
            .map_err(ipc_error)?;
        let response = match std::str::from_utf8(&input) {
            Ok(line) => response_for(line, &token, service.as_ref()),
            Err(error) => json!({
                "error": {
                    "code": "ASSISTANT_IPC_INVALID_REQUEST",
                    "message": "私有 IPC 请求不是 UTF-8",
                    "detail": error.to_string(),
                }
            }),
        };
        let mut output = serde_json::to_vec(&response).map_err(ipc_error)?;
        output.push(b'\n');
        server.write_all(&output).await.map_err(ipc_error)?;
        server.shutdown().await.map_err(ipc_error)?;
        server = create().map_err(ipc_error)?;
    }
    unsafe {
        LocalFree(Some(HLOCAL(descriptor.0)));
    }
    Ok(())
}

#[cfg(unix)]
fn handle_unix_connection(
    mut stream: std::os::unix::net::UnixStream,
    token: &str,
    service: &dyn PrivateIpcService,
) {
    use std::io::{BufRead, BufReader, Read, Write};

    if stream.set_nonblocking(false).is_err() {
        return;
    }
    let mut line = String::new();
    let read = BufReader::new(&stream)
        .take(MAX_REQUEST_BYTES)
        .read_line(&mut line);
    let response = match read {
        Ok(0) | Err(_) => return,
        Ok(_) => response_for(&line, token, service),
    };
    if serde_json::to_writer(&mut stream, &response).is_ok() {
        let _ = stream.write_all(b"\n");
        let _ = stream.flush();
    }
}

fn ipc_error(error: impl std::fmt::Display) -> AppError {
    AppError::new("ASSISTANT_IPC_ERROR", "无法启动智奈私有 IPC").with_detail(error.to_string())
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::{fs::PermissionsExt, net::UnixStream},
        sync::Arc,
    };

    use serde_json::{json, Value};
    use tempfile::tempdir;

    use crate::{
        assistant::private_ipc::{PrivateIpcServer, PrivateIpcService},
        error::AppError,
    };

    struct EchoService;

    impl PrivateIpcService for EchoService {
        fn call(&self, method: &str, params: &Value) -> Result<Value, AppError> {
            Ok(json!({ "method": method, "params": params }))
        }
    }

    fn request(path: &std::path::Path, value: Value) -> Value {
        let mut stream = UnixStream::connect(path).unwrap();
        writeln!(stream, "{value}").unwrap();
        stream.shutdown(std::net::Shutdown::Write).unwrap();
        let mut response = String::new();
        BufReader::new(stream).read_line(&mut response).unwrap();
        serde_json::from_str(&response).unwrap()
    }

    #[test]
    fn unix_socket_is_private_and_rejects_an_invalid_startup_token() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("netcat-assistant.sock");
        let mut server = PrivateIpcServer::start(
            path.clone(),
            "fresh-startup-token".into(),
            Arc::new(EchoService),
        )
        .unwrap();

        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let rejected = request(
            &path,
            json!({ "token": "wrong", "method": "ping", "params": {} }),
        );
        assert_eq!(rejected["error"]["code"], "ASSISTANT_IPC_UNAUTHORIZED");

        let accepted = request(
            &path,
            json!({
                "token": "fresh-startup-token",
                "method": "credentials/describe",
                "params": { "reference": "DEEPSEEK_API_KEY" }
            }),
        );
        assert_eq!(accepted["result"]["method"], "credentials/describe");
        server.stop().unwrap();
        assert!(!path.exists());
    }
}
