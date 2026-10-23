use netcapture_lib::system_proxy::{
    parse_default_interface, parse_hardware_service, parse_proxy_state,
};

#[test]
fn parses_default_route_interface() {
    let output = "route to: default\ndestination: default\ninterface: en0\nflags: <UP,GATEWAY>";
    assert_eq!(parse_default_interface(output).as_deref(), Some("en0"));
}

#[test]
fn maps_device_to_hardware_service() {
    let output = "Hardware Port: Wi-Fi\nDevice: en0\nEthernet Address: aa:bb\n\nHardware Port: USB 10/100/1000 LAN\nDevice: en5\n";
    assert_eq!(
        parse_hardware_service(output, "en0").as_deref(),
        Some("Wi-Fi")
    );
}

#[test]
fn parses_networksetup_proxy_state() {
    let output = "Enabled: Yes\nServer: 127.0.0.1\nPort: 8888\nAuthenticated Proxy Enabled: 0\n";
    let state = parse_proxy_state(output).unwrap();
    assert!(state.enabled);
    assert_eq!(state.server, "127.0.0.1");
    assert_eq!(state.port, 8888);
}
