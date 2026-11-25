use std::{
    collections::HashSet,
    fs,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::AppError;

use super::private_ipc::{PrivateIpcServer, PrivateIpcService};
use super::supervisor::{BridgeRuntime, BridgeRuntimeInterrupt, OperationIdentity};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeEvent {
    pub event_type: String,
    pub identity: OperationIdentity,
    pub payload: Value,
}

pub trait BridgeEventSink: Send + Sync {
    fn event(&self, event: BridgeEvent);
}

pub struct NativeBridgeConfig {
    pub node_path: PathBuf,
    pub bridge_path: PathBuf,
    pub dsh_home: PathBuf,
    pub cwd: PathBuf,
    pub profile: String,
    pub patches: Vec<PathBuf>,
    pub provider: String,
    pub model: String,
    pub profile_patch: Option<String>,
    pub event_sink: Arc<dyn BridgeEventSink>,
    pub ipc_service: Arc<dyn PrivateIpcService>,
}

struct ProcessControl {
    child: Mutex<Option<Child>>,
}

impl ProcessControl {
    fn new() -> Self {
        Self {
            child: Mutex::new(None),
        }
    }

    fn terminate_tree(&self) -> Result<(), AppError> {
        let mut child = self.child.lock().expect("assistant child mutex poisoned");
        let Some(mut process) = child.take() else {
            return Ok(());
        };
        if process.try_wait().map_err(process_error)?.is_none() {
            terminate_process_tree(&mut process)?;
        }
        let _ = process.wait();
        Ok(())
    }
}

impl BridgeRuntimeInterrupt for ProcessControl {
    fn interrupt(&self) -> Result<(), AppError> {
        self.terminate_tree()
    }
}

pub struct NativeBridgeRuntime {
    config: NativeBridgeConfig,
    control: Arc<ProcessControl>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    next_request_id: u64,
    ipc_server: Option<PrivateIpcServer>,
    sessions: HashSet<String>,
}

impl NativeBridgeRuntime {
    pub fn new(config: NativeBridgeConfig) -> Self {
        Self {
            config,
            control: Arc::new(ProcessControl::new()),
            stdin: None,
            stdout: None,
            next_request_id: 0,
            ipc_server: None,
            sessions: HashSet::new(),
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, AppError> {
        self.next_request_id += 1;
        let id = format!("netcat-{}", self.next_request_id);
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let stdin = self.stdin.as_mut().ok_or_else(runtime_not_started)?;
        serde_json::to_writer(&mut *stdin, &request).map_err(protocol_error)?;
        stdin.write_all(b"\n").map_err(process_error)?;
        stdin.flush().map_err(process_error)?;

        loop {
            let stdout = self.stdout.as_mut().ok_or_else(runtime_not_started)?;
            let mut line = String::new();
            if stdout.read_line(&mut line).map_err(process_error)? == 0 {
                return Err(AppError::new(
                    "ASSISTANT_RUNTIME_EXITED",
                    "智奈 Runtime 已意外退出",
                ));
            }
            let message: BridgeMessage = serde_json::from_str(&line).map_err(protocol_error)?;
            if message.method.as_deref() == Some("event") {
                self.emit_event(message.params)?;
                continue;
            }
            if message.id.as_deref() != Some(id.as_str()) {
                return Err(AppError::new(
                    "ASSISTANT_PROTOCOL_ERROR",
                    "智奈 Bridge 返回了无法关联的响应",
                ));
            }
            if let Some(error) = message.error {
                return Err(AppError::new("ASSISTANT_BRIDGE_ERROR", error.message));
            }
            return message.result.ok_or_else(|| {
                AppError::new("ASSISTANT_PROTOCOL_ERROR", "智奈 Bridge 响应缺少结果")
            });
        }
    }

    fn emit_event(&self, params: Option<Value>) -> Result<(), AppError> {
        let event: WireBridgeEvent =
            serde_json::from_value(params.unwrap_or(Value::Null)).map_err(protocol_error)?;
        self.config.event_sink.event(BridgeEvent {
            event_type: event.event_type,
            identity: OperationIdentity::new(
                event.request_id,
                event.conversation_id,
                event.session_id,
                event.operation_id,
            ),
            payload: event.payload,
        });
        Ok(())
    }

    fn clear_streams(&mut self) {
        self.stdin = None;
        self.stdout = None;
    }
}

impl BridgeRuntime for NativeBridgeRuntime {
    fn interrupt_handle(&self) -> Arc<dyn BridgeRuntimeInterrupt> {
        self.control.clone()
    }

    fn start(&mut self) -> Result<(), AppError> {
        if self
            .control
            .child
            .lock()
            .expect("assistant child mutex poisoned")
            .is_some()
        {
            return Ok(());
        }
        fs::create_dir_all(&self.config.dsh_home).map_err(process_error)?;
        fs::create_dir_all(&self.config.cwd).map_err(process_error)?;
        let ipc_token = uuid::Uuid::new_v4().to_string();
        let ipc_endpoint = private_ipc_endpoint(&ipc_token, &self.config.dsh_home);
        self.ipc_server = Some(PrivateIpcServer::start(
            ipc_endpoint.clone(),
            ipc_token.clone(),
            Arc::clone(&self.config.ipc_service),
        )?);
        let mut command = Command::new(&self.config.node_path);
        command
            .arg(&self.config.bridge_path)
            .current_dir(&self.config.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .env("NETCAT_TOOL_ENDPOINT", &ipc_endpoint)
            .env("NETCAT_TOOL_TOKEN", &ipc_token);
        configure_process_group(&mut command);
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                if let Some(mut server) = self.ipc_server.take() {
                    let _ = server.stop();
                }
                return Err(process_error(error));
            }
        };
        self.stdin = child.stdin.take();
        self.stdout = child.stdout.take().map(BufReader::new);
        *self
            .control
            .child
            .lock()
            .expect("assistant child mutex poisoned") = Some(child);

        let mut patches = self.config.patches.clone();
        if let Some(contents) = &self.config.profile_patch {
            let path = self.config.dsh_home.join("netcat-openai.patch.yml");
            fs::write(&path, contents).map_err(process_error)?;
            patches.push(path);
        }
        let params = json!({
            "provider": self.config.provider,
            "model": self.config.model,
            "dshHome": self.config.dsh_home,
            "cwd": self.config.cwd,
            "profile": self.config.profile,
            "patches": patches,
        });
        if let Err(error) = self.request("runtime/initialize", params) {
            self.clear_streams();
            let _ = self.control.terminate_tree();
            if let Some(mut server) = self.ipc_server.take() {
                let _ = server.stop();
            }
            return Err(error);
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AppError> {
        if self.stdin.is_some() {
            let _ = self.request("runtime/shutdown", json!({}));
        }
        self.clear_streams();
        self.sessions.clear();
        self.control.terminate_tree()?;
        if let Some(mut server) = self.ipc_server.take() {
            server.stop()?;
        }
        Ok(())
    }

    fn run(&mut self, operation: &OperationIdentity, input: &str) -> Result<String, AppError> {
        if !self.sessions.contains(&operation.session_id) {
            let identity = json!({
                "requestId": operation.request_id,
                "conversationId": operation.conversation_id,
                "sessionId": operation.session_id,
                "operationId": operation.operation_id,
            });
            if self.request("session/resume", identity.clone()).is_err() {
                self.request("session/create", identity)?;
            }
            self.sessions.insert(operation.session_id.clone());
        }
        let result = self.request(
            "session/run",
            json!({
                "requestId": operation.request_id,
                "conversationId": operation.conversation_id,
                "sessionId": operation.session_id,
                "operationId": operation.operation_id,
                "input": input,
            }),
        )?;
        result
            .get("finalResponse")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                AppError::new("ASSISTANT_PROTOCOL_ERROR", "智奈 Bridge 响应缺少最终回复")
            })
    }

    fn configure(
        &mut self,
        provider: String,
        model: String,
        profile_patch: Option<String>,
    ) -> Result<(), AppError> {
        self.config.provider = provider;
        self.config.model = model;
        self.config.profile_patch = profile_patch;
        self.sessions.clear();
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BridgeMessage {
    id: Option<String>,
    method: Option<String>,
    params: Option<Value>,
    result: Option<Value>,
    error: Option<BridgeError>,
}

#[derive(Deserialize)]
struct BridgeError {
    message: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireBridgeEvent {
    #[serde(rename = "type")]
    event_type: String,
    request_id: String,
    conversation_id: String,
    session_id: String,
    operation_id: String,
    #[serde(default)]
    payload: Value,
}

fn runtime_not_started() -> AppError {
    AppError::new("ASSISTANT_RUNTIME_STOPPED", "智奈 Runtime 尚未启动")
}

fn process_error(error: impl std::fmt::Display) -> AppError {
    AppError::new("ASSISTANT_RUNTIME_ERROR", "无法运行智奈 Runtime").with_detail(error.to_string())
}

fn protocol_error(error: impl std::fmt::Display) -> AppError {
    AppError::new("ASSISTANT_PROTOCOL_ERROR", "智奈 Bridge 协议无效").with_detail(error.to_string())
}

#[cfg(unix)]
fn private_ipc_endpoint(token: &str, _dsh_home: &std::path::Path) -> PathBuf {
    PathBuf::from(format!("/tmp/netcat-assistant-{token}.sock"))
}

#[cfg(windows)]
fn private_ipc_endpoint(token: &str, _dsh_home: &std::path::Path) -> PathBuf {
    PathBuf::from(format!(r"\\.\pipe\netcat-assistant-{token}"))
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
fn configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn terminate_process_tree(process: &mut Child) -> Result<(), AppError> {
    let result = unsafe { libc::kill(-(process.id() as i32), libc::SIGKILL) };
    if result == 0 {
        Ok(())
    } else {
        process.kill().map_err(process_error)
    }
}

#[cfg(windows)]
fn terminate_process_tree(process: &mut Child) -> Result<(), AppError> {
    let result = Command::new("taskkill")
        .args(["/PID", &process.id().to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match result {
        Ok(status) if status.success() => Ok(()),
        _ => process.kill().map_err(process_error),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::{Arc, Mutex},
    };

    use tempfile::tempdir;

    use crate::assistant::{
        native_runtime::{BridgeEvent, BridgeEventSink, NativeBridgeConfig, NativeBridgeRuntime},
        private_ipc::PrivateIpcService,
        supervisor::{BridgeRuntime, OperationIdentity},
    };
    use crate::error::AppError;
    use serde_json::Value;

    #[cfg(unix)]
    #[test]
    fn private_ipc_endpoint_stays_bindable_for_long_application_data_paths() {
        use std::os::unix::net::SocketAddr;

        let long_dsh_home = PathBuf::from("/").join("a".repeat(160));
        let first = super::private_ipc_endpoint("first-token", &long_dsh_home);
        let second = super::private_ipc_endpoint("second-token", &long_dsh_home);

        assert!(SocketAddr::from_pathname(&first).is_ok());
        assert!(SocketAddr::from_pathname(&second).is_ok());
        assert_ne!(first, second);
    }

    #[derive(Default)]
    struct RecordingSink(Mutex<Vec<BridgeEvent>>);

    impl BridgeEventSink for RecordingSink {
        fn event(&self, event: BridgeEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    struct NoopIpcService;

    impl PrivateIpcService for NoopIpcService {
        fn call(&self, _method: &str, _params: &Value) -> Result<Value, AppError> {
            Ok(Value::Null)
        }
    }

    #[test]
    fn starts_bridge_runs_a_correlated_turn_and_receives_events() {
        let directory = tempdir().unwrap();
        let script = directory.path().join("bridge.mjs");
        fs::write(
            &script,
            r#"
import { createInterface } from 'node:readline'
const lines = createInterface({ input: process.stdin, crlfDelay: Infinity })
for await (const line of lines) {
  const request = JSON.parse(line)
  if (request.method === 'runtime/initialize') {
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: request.id, result: { status: 'ready' } }) + '\n')
  } else if (request.method === 'session/resume') {
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: request.id, error: { message: 'new session' } }) + '\n')
  } else if (request.method === 'session/create') {
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: request.id, result: { sessionId: request.params.sessionId } }) + '\n')
  } else if (request.method === 'session/run') {
    const identity = request.params
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', method: 'event', params: {
      type: 'session/activity',
      requestId: identity.requestId,
      conversationId: identity.conversationId,
      sessionId: identity.sessionId,
      operationId: identity.operationId,
      payload: { label: '正在分析' }
    } }) + '\n')
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: request.id, result: {
      sessionId: identity.sessionId,
      finalResponse: `已分析: ${identity.input}`,
      finishReason: 'stop'
    } }) + '\n')
  } else if (request.method === 'runtime/shutdown') {
    process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: request.id, result: { status: 'stopped' } }) + '\n')
    break
  }
}
"#,
        )
        .unwrap();
        let sink = Arc::new(RecordingSink::default());
        let mut runtime = NativeBridgeRuntime::new(NativeBridgeConfig {
            node_path: PathBuf::from("node"),
            bridge_path: script,
            dsh_home: directory.path().join("home"),
            cwd: directory.path().join("work"),
            profile: "netcat-agent".into(),
            patches: vec![directory.path().join("cordis.patch.yml")],
            provider: "deepseek-official".into(),
            model: "deepseek-v4-flash".into(),
            profile_patch: None,
            event_sink: sink.clone(),
            ipc_service: Arc::new(NoopIpcService),
        });
        let identity =
            OperationIdentity::new("request-1", "conversation-1", "session-1", "operation-1");

        runtime.start().unwrap();
        let response = runtime.run(&identity, "检查登录接口").unwrap();
        runtime.stop().unwrap();

        assert_eq!(response, "已分析: 检查登录接口");
        let events = sink.0.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "session/activity");
        assert_eq!(events[0].identity, identity);
        assert_eq!(events[0].payload["label"], "正在分析");
    }
}
