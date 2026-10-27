# NetCat

[简体中文](README.md) | [English](README.en.md)

> Watching over every request, locally.

NetCat is a local HTTP/HTTPS debugging proxy for developers. It combines traffic inspection, editable replay, mocks, QuickJS proxy scripts, proxy-layer network simulation, HAR export, system proxy management, and local CA management.

NetCat is built with Vue 3, Tauri 2, and Rust. Request history, mock rules, and application settings stay on your device by default. No account, cloud service, telemetry, or remote data upload is required.

NetCat is not affiliated with the traditional `netcat` / `nc` command-line networking utility. This project focuses on desktop HTTP/HTTPS API debugging.

> Only capture devices, applications, and networks that you own or are explicitly authorized to test. Do not use this tool for unauthorized traffic interception, credential collection, or access to private data.

## Core Features

- Local HTTP/1.1 and HTTPS CONNECT proxy, listening on `127.0.0.1:28086` by default
- Mutually exclusive computer and mobile capture modes; mobile mode accepts only paired private-network devices and never captures computer traffic
- Dynamic HTTPS leaf certificates and a local root CA; the private key is stored in macOS Keychain or encrypted with current-user DPAPI on Windows
- Editable request replay for GET, POST, JSON, form, and binary bodies
- Exact, substring, and regular-expression URL mocks with method, priority, status code, headers, body, delay, match testing, and hit statistics
- SQLite persistence with a rolling limit of 5,000 records and a 10 MB truncation limit for each request or response body
- Local and per-device tabs plus combined keyword, method, status, and domain filters
- HAR 1.2 export and executable cURL copy
- HTTP/HTTPS system proxy configuration and recovery for the current network service on macOS and the current user on Windows
- Embedded QuickJS request/response scripts for header, URL, body, and synthetic-response changes
- Fast 4G, Slow 4G, 3G, Offline, and custom proxy-layer network profiles with immediate switching

The current release does not parse HTTP/2, QUIC, WebSocket frames, or non-HTTP TCP traffic. NetCat does not provide certificate pinning bypass or code injection.

## Technology Stack

- Vue 3, TypeScript, Vite, and Monaco Editor
- Tauri 2 and Rust
- SQLite local persistence
- Vitest, Vue Testing Library, and the Rust test suite

## Platform Status

| Platform | Status | Notes |
| --- | --- | --- |
| macOS 12+ | Primary support | Current-user CA trust, system proxy integration, Keychain-backed private keys, and `.app` / DMG packaging are implemented |
| Windows 10 22H2 / Windows 11 | Experimental support | x86_64 MSVC only; current-user proxy integration, DPAPI private keys, certificate trust, and NSIS build configuration are implemented, but native Windows acceptance testing is still required |

The project is currently an internal development build. It is not recommended for production environments or uncontrolled networks.

## Development Environment

- macOS 12+ on Apple Silicon, or Windows 10 22H2 / Windows 11 on x86_64
- Node.js 22 or later
- Rust 1.85 or later
- macOS: Xcode Command Line Tools
- Windows: Visual Studio 2022 Build Tools with the Desktop development with C++ workload and Windows SDK, the Rust `x86_64-pc-windows-msvc` toolchain, and Microsoft Edge WebView2 Runtime

Install dependencies and start the browser preview:

```bash
npm install
npm run dev
```

The browser preview runs at `http://127.0.0.1:1420`. To run the desktop application:

```bash
npm run tauri dev
```

Run tests and builds:

```bash
npm test
cargo test --manifest-path src-tauri/Cargo.toml
npm run build
npm run tauri build
```

The Windows NSIS installer must be built in a Windows development environment:

```powershell
rustup target add x86_64-pc-windows-msvc
npm ci
npm run tauri:build:windows
```

The installer is written to `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/`. The Windows CI workflow runs on `windows-2022`, installs dependencies, runs frontend tests, checks bundle budgets, runs Rust tests, builds the actual NSIS installer, and uploads the `.exe` artifact. macOS continues to use `npm run tauri -- build --bundles app,dmg` for `.app` and DMG packaging.

The Windows installer runs per user. NetCat requires WebView2; when the runtime is missing, NSIS uses `downloadBootstrapper` to download and install it. The first installation therefore requires access to Microsoft download services. Install the WebView2 Evergreen Runtime in advance on restricted networks. The current macOS development host has not built or run the Windows installer. Treat Windows CI as the build source of truth, and validate installation, startup, and uninstallation on Windows 10 22H2 and Windows 11.

The 60-second stress acceptance test is ignored by default. Run it separately:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test proxy_stress sustains_200_qps_for_60_seconds_and_keeps_5000_rows -- --ignored --exact
```

## First Run

1. Read and accept the authorized-use notice.
2. Open Certificate and select Generate Certificate. The public certificate is written to the application data directory. The private key is stored in macOS Keychain or encrypted with current-user DPAPI in `ca-private-key.dpapi` on Windows. Copying that file to another account is not sufficient to use it.
3. On macOS, select Install Trust in the certificate panel. After authorization, NetCat writes only the current CA to the current user's login Keychain. On Windows, select Install Trust to write only to the current user's Trusted Root Certification Authorities store. On either platform, you may export the public CA for manual recovery and should verify its SHA-256 fingerprint.
4. Computer mode starts only when the current CA is trusted. Otherwise NetCat refuses to start and leaves the system proxy unchanged. Install trust before enabling the proxy to avoid `NET::ERR_CERT_AUTHORITY_INVALID` in browsers.
5. Start the proxy. Enable System Proxy only when applications that follow the operating system proxy should connect automatically. On Windows, NetCat changes only the current user's manual HTTP/HTTPS proxy in Internet Settings. It preserves the bypass list, PAC, and automatic detection settings. It does not configure WinHTTP; applications with independent proxy or certificate stores require separate configuration.
6. Stop the proxy when testing is complete. NetCat attempts to restore the saved HTTP and HTTPS proxy configuration. If another program or user changed the proxy, NetCat preserves its recovery record and asks for manual resolution instead of overwriting the newer configuration.

Use the CA only in controlled development environments. Do not install it on production devices or distribute it to unrelated users.

## Upgrading from NetCapture on macOS

- Stop the proxy and fully quit the previous NetCapture application before upgrading so both versions cannot modify the system proxy at the same time.
- On first launch, when the NetCat database does not already exist, NetCat copies traffic, mock rules, and settings from `~/Library/Application Support/com.netcapture.desktop/netcapture.sqlite3` into the new database.
- The old directory, database, and Keychain records are preserved for rollback. Existing NetCat data is never overwritten.
- NetCat generates a new `NetCat Local Root CA`. Install trust again from the certificate panel. After confirming HTTPS capture works, you may manually delete the old `NetCapture Local Root CA` from Keychain Access.
- The certificate download endpoint is now `/__netcat/ca.crt`, and HAR extension fields now use `_netcat_*`. Legacy names are not supported.

## Mobile Device Setup

1. Connect the computer and mobile device to the same trusted Wi-Fi network. On Windows, allow inbound proxy traffic only for trusted private networks if the firewall prompts you.
2. Under Settings > Proxy, select Mobile Capture and start the proxy. Mobile mode does not enable the system proxy and does not record computer requests.
3. Set the mobile device's Wi-Fi HTTP proxy to the computer IP and port shown in NetCat.
4. Select Generate Pairing QR Code and scan it with the mobile device. The QR code expires after five minutes and can be used only once. A successful device immediately appears in Settings and in a request-page device tab.
5. Download the CA from the pairing success page, or visit `http://<computer-IP>:28086/__netcat/ca.crt`. After installing the profile on iOS, open Settings > General > About > Certificate Trust Settings and enable full trust.
6. Android 7+ applications may not trust user CAs by default. The application under test must explicitly allow user certificates in its Network Security Config. Some vendor interfaces place Wi-Fi proxy settings under Modify network > Advanced options.
7. Devices disconnect after 15 idle minutes by default; the setting accepts 5–120 minutes. Stopping the proxy, manually disconnecting, or timing out clears the current pairing, so the device must scan a new code next time. Captured history remains available.

Use the All, Local, and per-device tabs on the request page to switch traffic sources. A disconnected device remains visible while it has history. Settings provides rename, re-pair, disconnect, and confirmed deletion of all history for that device.

## JavaScript Scripts

The Scripts workspace manages QuickJS proxy scripts. Enabled proxy scripts run by descending priority and creation time. Live traffic and replay share this order: request scripts → upload network conditions/faults → mock or upstream → response scripts → download network conditions → persistence. Mock-aware replay uses the full pipeline; direct replay only bypasses mocks.

Scripts may define `onRequest(ctx)` and `onResponse(ctx)` and return `method`, `url`, `headers`, `bodyText`, or `bodyBase64`. `onRequest` may return `response: { status, headers, bodyText }`. Each execution is limited to 100 ms and 64 MB. A failing script is rolled back and traffic continues. Bodies over 10 MiB after decoding, or unsupported encodings, cannot be rewritten, but headers and routing remain editable.

```javascript
function onRequest(ctx) {
  return { headers: { ...ctx.headers, 'x-netcat-script': 'enabled' } };
}

function onResponse(ctx) {
  return ctx.status === 200 ? { headers: { ...ctx.headers, 'cache-control': 'no-store' } } : {};
}
```

The QuickJS sandbox exposes no Node.js, npm, filesystem, network, or process APIs. Scripts stay local, are never uploaded, and are not fetched from the network.

## Network Simulation

The traffic toolbar offers Unlimited, Fast 4G, Slow 4G, 3G, Offline, and custom throttling profiles. Presets are Fast 4G: 20 ms, 9000/1500 Kbps down/up; Slow 4G: 150 ms, 1600/750; 3G: 100 ms, 750/250; Offline: 100% timeout.

Custom profiles support latency, jitter, upload/download rates, timeout/disconnect probability, method filters, and exact/substring/regex URL matching. This is proxy-layer simulation; it does not emulate actual IP packet loss, TCP reordering, congestion windows, or link-layer behavior.

## Recovery

### Manually Disable the System Proxy

On macOS, if NetCat exits unexpectedly and automatic recovery fails, list the network service names first:

```bash
networksetup -listallnetworkservices
```

Then disable the proxies for the service in use, for example Wi-Fi:

```bash
networksetup -setwebproxystate "Wi-Fi" off
networksetup -setsecurewebproxystate "Wi-Fi" off
```

If a custom proxy was configured before NetCat started, restore its original host, port, and enabled state under System Settings > Network > active network > Details > Proxies instead of simply turning the proxy off.

On Windows, sign in as the same user who ran NetCat and open Settings > Network & Internet > Proxy. Turn off Use a proxy server under Manual proxy setup only if no proxy was configured before NetCat started. If a custom proxy was already configured, restore its original address, port, exceptions, and enabled state. Preserve the setup script and automatic detection values under Automatic proxy setup. Reopen affected applications after changing the settings. Do not use `netsh winhttp reset proxy` to restore this current-user proxy configuration.

After manually restoring the complete original configuration, select Retry System Proxy Recovery in NetCat settings. If the current values exactly match the saved original values, NetCat clears its recovery record without rewriting the proxy and can take control again. If any value still differs from both the original value and the value written by NetCat, the application preserves the recovery record and continues to report a conflict.

To inspect the current-user settings without changing them, run this read-only PowerShell command:

```powershell
Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings' |
  Select-Object ProxyEnable, ProxyServer, ProxyOverride, AutoConfigURL, AutoDetect
```

### Common Errors

- `Port unavailable`: choose a different port or stop the process using it.
- `CA not generated`: open the certificate panel and generate a local CA.
- HTTPS handshake failure: verify that the CA is trusted; the target application may also use certificate pinning.
- Upstream connection failure or timeout: verify the target server, DNS, and current network connectivity.
- System proxy recovery failure: follow the manual recovery steps for the platform. On macOS, verify the network service name. On Windows, verify the user account and whether another program changed the settings.
- Windows CA is not trusted: verify that the current NetCat CA was imported into the current user's root certificate store. Some applications use independent certificate stores or certificate pinning.
- Windows UI does not start: install the Microsoft Edge WebView2 Evergreen Runtime and try again.
- Script timeout or syntax error: inspect script test results and diagnostics. Executions exceeding 100 ms are rolled back and traffic continues.
- Unexpectedly slow requests or timeouts: check the toolbar's throttling profile and retry with Unlimited.

## Uninstalling and Removing the CA

Before uninstalling, stop the proxy, disable the system proxy, confirm that networking has been restored, and quit NetCat. The uninstaller does not replace the application's proxy recovery process and does not automatically remove trusted CAs.

### macOS

1. Complete the stop and recovery steps above.
2. Search for and delete `NetCat Local Root CA` in Keychain Access.
3. Remove any same-name entries that remain under My Certificates or Certificates.
4. To remove the CA private-key record from Keychain, run:

```bash
security delete-generic-password -s com.kaitehan.netcat -a root-ca-private-key
```

5. Delete `~/Library/Application Support/com.kaitehan.netcat`, then move `NetCat.app` to the Trash.

### Windows

1. Stop the proxy and disable the system proxy. Confirm that recovery succeeded; otherwise complete the manual recovery steps first.
2. Remove trust for the current CA from the certificate panel, then quit NetCat. Alternatively, run `certmgr.msc`, open Current User > Trusted Root Certification Authorities > Certificates, verify the fingerprint, and delete only the `NetCat Local Root CA` generated by this application. Do not delete unrelated certificates with the same display name.
3. Uninstall NetCat under Settings > Apps > Installed apps, or Apps & features on Windows 10.
4. To remove all local data, delete `%APPDATA%\com.kaitehan.netcat`. It contains the database, public CA, and DPAPI-encrypted private-key file. Remove certificate trust before deleting the data so you can still verify the certificate identity.

Deleting application data permanently removes local traffic, mock rules, and settings. HAR exports and exported public CA files stored elsewhere must be removed separately.

## Data and Security Boundaries

- No account, cloud synchronization, telemetry, or remote upload.
- The current-user data directory is `~/Library/Application Support/com.kaitehan.netcat` on macOS and `%APPDATA%\com.kaitehan.netcat` on Windows. The database is `netcat.sqlite3`, and the public CA is stored under the `certificates` directory.
- The CA private key is never written to SQLite, logs, HAR files, cURL output, or public certificate exports.
- Pausing capture only stops the creation of new display records; proxy forwarding continues.
- Invalid regular expressions, headers, status codes, JSON, or delay values are rejected before a mock rule can become active.
- A TLS failure is reported only as an untrusted certificate or possible certificate pinning. NetCat does not claim to identify the exact cause.
- QuickJS proxy scripts are for owned or explicitly authorized network traffic only. Local scripts are never uploaded or downloaded.

## Roadmap

- Full HTTP/2 parsing, WebSocket frame-level debugging, and QUIC / HTTP/3 support
- Automatic updates, macOS signing and notarization, and compliance work required for public distribution
- Basic request and response breakpoints remain gated by performance and stability requirements. If they do not pass the 0.3.0 gate, they remain a future candidate and are not considered delivered in 0.3.0.

## Contributing

Bug reports, feature proposals, documentation improvements, and code contributions are welcome. Read the [contribution guide](CONTRIBUTING.md) before starting. Report security issues privately according to the [security policy](SECURITY.md); do not disclose vulnerability details in a public issue.

Suggested GitHub Topics: `http-proxy`, `https-proxy`, `network-debugging`, `api-testing`, `mock-server`, `request-replay`, `tauri`, `rust`, `vue`, `developer-tools`, `macos`, and `windows`.

## License

NetCat is open source under the [Apache License 2.0](LICENSE). Use, modification, and distribution of this project must comply with the license terms.
