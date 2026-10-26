use std::{
    fs,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
};

use crate::{
    error::AppError,
    models::{
        FridaApplication, FridaDevice, FridaEnvironmentStatus, MatchKind, PinningLaunchMode,
        PinningSessionConfig, PinningSessionStatus, ProxyScript, ScriptKind,
    },
};

pub fn builtin_injection_scripts() -> Vec<ProxyScript> {
    vec![
        ProxyScript {
            id: "builtin-android-pinning".into(),
            name: "Android 常见 TLS 栈".into(),
            enabled: true,
            priority: 100,
            methods: Vec::new(),
            match_kind: MatchKind::Contains,
            url_pattern: String::new(),
            source: ANDROID_PINNING_SCRIPT.into(),
            kind: ScriptKind::Injection,
            platform: Some("android".into()),
            builtin: true,
            created_at: "2026-10-26T00:00:00Z".into(),
        },
        ProxyScript {
            id: "builtin-ios-pinning".into(),
            name: "iOS 常见 TLS 栈".into(),
            enabled: true,
            priority: 100,
            methods: Vec::new(),
            match_kind: MatchKind::Contains,
            url_pattern: String::new(),
            source: IOS_PINNING_SCRIPT.into(),
            kind: ScriptKind::Injection,
            platform: Some("ios".into()),
            builtin: true,
            created_at: "2026-10-26T00:00:00Z".into(),
        },
    ]
}

const ANDROID_PINNING_SCRIPT: &str = r#"Java.perform(function () {
  var TrustManager = Java.use('javax.net.ssl.X509TrustManager');
  var SSLContext = Java.use('javax.net.ssl.SSLContext');
  var Permissive = Java.registerClass({
    name: 'com.netcat.PermissiveTrustManager',
    implements: [TrustManager],
    methods: {
      checkClientTrusted: function () {},
      checkServerTrusted: function () {},
      getAcceptedIssuers: function () { return []; }
    }
  });
  var managers = [Permissive.$new()];
  SSLContext.init.overload('[Ljavax.net.ssl.KeyManager;', '[Ljavax.net.ssl.TrustManager;', 'java.security.SecureRandom')
    .implementation = function (keyManagers, trustManagers, random) {
      return this.init(keyManagers, managers, random);
    };
  try {
    var Pinner = Java.use('okhttp3.CertificatePinner');
    Pinner.check.overload('java.lang.String', 'java.util.List').implementation = function () {};
  } catch (_) {}
  try {
    var WebViewClient = Java.use('android.webkit.WebViewClient');
    WebViewClient.onReceivedSslError.implementation = function (_, handler) { handler.proceed(); };
  } catch (_) {}
  send({ type: 'netcat', message: 'Android TLS hooks installed' });
});"#;

const IOS_PINNING_SCRIPT: &str = r#"var trustEvaluate = Module.findExportByName('Security', 'SecTrustEvaluateWithError');
if (trustEvaluate) {
  Interceptor.replace(trustEvaluate, new NativeCallback(function (_, error) {
    if (!error.isNull()) error.writePointer(ptr(0));
    return 1;
  }, 'bool', ['pointer', 'pointer']));
}
var trustEvaluateLegacy = Module.findExportByName('Security', 'SecTrustEvaluate');
if (trustEvaluateLegacy) {
  Interceptor.replace(trustEvaluateLegacy, new NativeCallback(function (_, result) {
    result.writeU32(1);
    return 0;
  }, 'int', ['pointer', 'pointer']));
}
send({ type: 'netcat', message: 'iOS Security hooks installed; NSURLSession/AFNetworking/Alamofire/TrustKit use the system trust path when applicable' });"#;

pub struct FridaManager {
    session: Mutex<Option<Child>>,
    status: Mutex<PinningSessionStatus>,
    script_path: PathBuf,
}

impl FridaManager {
    pub fn new(data_directory: PathBuf) -> Self {
        Self {
            session: Mutex::new(None),
            status: Mutex::new(PinningSessionStatus::default()),
            script_path: data_directory.join("frida-session.js"),
        }
    }

    pub fn environment(&self) -> FridaEnvironmentStatus {
        match Command::new("frida").arg("--version").output() {
            Ok(output) if output.status.success() => FridaEnvironmentStatus {
                available: true,
                version: Some(String::from_utf8_lossy(&output.stdout).trim().to_string()),
                error: None,
            },
            Ok(output) => FridaEnvironmentStatus {
                available: false,
                version: None,
                error: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
            },
            Err(error) => FridaEnvironmentStatus {
                available: false,
                version: None,
                error: Some(error.to_string()),
            },
        }
    }

    pub fn devices(&self) -> Result<Vec<FridaDevice>, AppError> {
        let output = run_output("frida-ls-devices", &[])?;
        Ok(parse_devices(&output))
    }

    pub fn applications(&self, device_id: &str) -> Result<Vec<FridaApplication>, AppError> {
        let output = run_output("frida-ps", &["-D", device_id, "-ai"])?;
        Ok(parse_applications(&output))
    }

    pub fn start(
        &self,
        config: &PinningSessionConfig,
        script_source: &str,
        log_sink: Arc<dyn Fn(String) + Send + Sync>,
    ) -> Result<PinningSessionStatus, AppError> {
        self.stop()?;
        if let Some(parent) = self.script_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AppError::new("FRIDA_SCRIPT_ERROR", "无法创建 Frida 脚本目录")
                    .with_detail(error.to_string())
            })?;
        }
        fs::write(&self.script_path, script_source).map_err(|error| {
            AppError::new("FRIDA_SCRIPT_ERROR", "无法写入 Frida 会话脚本")
                .with_detail(error.to_string())
        })?;
        let mut command = Command::new("frida");
        if let Some(address) = config
            .gadget_address
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            command.args(["-H", address]);
        } else {
            command.args(["-D", config.device_id.as_str()]);
        }
        match config.mode {
            PinningLaunchMode::Spawn => command.args(["-f", config.target.as_str()]),
            PinningLaunchMode::Attach => command.args(["-n", config.target.as_str()]),
        };
        command.arg("-l").arg(&self.script_path).arg("--quiet");
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|error| {
            AppError::new("FRIDA_START_ERROR", "无法启动 Frida 会话")
                .with_detail(error.to_string())
                .with_action("请确认 frida-tools 已安装，且设备端 Frida 版本兼容")
        })?;
        if let Some(stdout) = child.stdout.take() {
            stream_logs(stdout, Arc::clone(&log_sink), "stdout");
        }
        if let Some(stderr) = child.stderr.take() {
            stream_logs(stderr, log_sink, "stderr");
        }
        let status = PinningSessionStatus {
            state: "running".into(),
            target: Some(config.target.clone()),
            process_id: Some(child.id()),
            error: None,
        };
        *self.session.lock().expect("frida session mutex poisoned") = Some(child);
        *self.status.lock().expect("frida status mutex poisoned") = status.clone();
        Ok(status)
    }

    pub fn status(&self) -> PinningSessionStatus {
        let mut session = self.session.lock().expect("frida session mutex poisoned");
        if let Some(child) = session.as_mut() {
            if let Ok(Some(exit)) = child.try_wait() {
                *session = None;
                let mut status = self.status.lock().expect("frida status mutex poisoned");
                status.state = "error".into();
                status.error = Some(format!("Frida 会话已退出: {exit}"));
            }
        }
        self.status
            .lock()
            .expect("frida status mutex poisoned")
            .clone()
    }

    pub fn stop(&self) -> Result<PinningSessionStatus, AppError> {
        if let Some(mut child) = self
            .session
            .lock()
            .expect("frida session mutex poisoned")
            .take()
        {
            child.kill().map_err(|error| {
                AppError::new("FRIDA_STOP_ERROR", "无法停止 Frida 会话")
                    .with_detail(error.to_string())
            })?;
            let _ = child.wait();
        }
        let _ = fs::remove_file(&self.script_path);
        let status = PinningSessionStatus::default();
        *self.status.lock().expect("frida status mutex poisoned") = status.clone();
        Ok(status)
    }
}

fn stream_logs(
    stream: impl std::io::Read + Send + 'static,
    sink: Arc<dyn Fn(String) + Send + Sync>,
    channel: &'static str,
) {
    thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            sink(format!("[{channel}] {line}"));
        }
    });
}

fn run_output(program: &str, args: &[&str]) -> Result<String, AppError> {
    let output = Command::new(program).args(args).output().map_err(|error| {
        AppError::new("FRIDA_UNAVAILABLE", "无法运行 Frida 工具")
            .with_detail(error.to_string())
            .with_action("请安装 frida-tools 并确保命令位于 PATH")
    })?;
    if !output.status.success() {
        return Err(AppError::new("FRIDA_COMMAND_FAILED", "Frida 命令执行失败")
            .with_detail(String::from_utf8_lossy(&output.stderr).trim().to_string()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn parse_devices(output: &str) -> Vec<FridaDevice> {
    output
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut columns = line.split_whitespace();
            let id = columns.next()?;
            let kind = columns.next()?;
            let name = columns.collect::<Vec<_>>().join(" ");
            (!name.is_empty()).then(|| FridaDevice {
                id: id.into(),
                kind: kind.into(),
                name,
            })
        })
        .collect()
}

pub fn parse_applications(output: &str) -> Vec<FridaApplication> {
    output
        .lines()
        .skip(1)
        .filter_map(|line| {
            let columns = line.split_whitespace().collect::<Vec<_>>();
            if columns.len() < 3 {
                return None;
            }
            let pid = columns[0].parse::<u32>().ok();
            let identifier = columns.last()?.to_string();
            let name = columns[1..columns.len() - 1].join(" ");
            Some(FridaApplication {
                pid,
                name,
                identifier,
            })
        })
        .collect()
}
