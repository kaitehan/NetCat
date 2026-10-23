// TCP tables start with a DWORD count followed by fixed-size OWNER_PID rows.
// Decode bytes rather than taking references into the OS-owned variable-length table.
fn listener_pid(
    bytes: &[u8],
    row_size: usize,
    port_offset: usize,
    pid_offset: usize,
    port: u16,
) -> Option<u32> {
    let count = u32::from_ne_bytes(bytes.get(..4)?.try_into().ok()?) as usize;
    let rows = bytes.get(4..4usize.checked_add(count.checked_mul(row_size)?)?)?;
    for row in rows.chunks_exact(row_size) {
        let local_port =
            u16::from_be_bytes(row.get(port_offset..port_offset + 2)?.try_into().ok()?);
        if local_port == port {
            return Some(u32::from_ne_bytes(
                row.get(pid_offset..pid_offset + 4)?.try_into().ok()?,
            ));
        }
    }
    None
}

fn process_name(path: &str) -> Option<String> {
    path.rsplit(['\\', '/'])
        .next()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}

#[cfg(target_os = "windows")]
pub use native::{platform_info, port_listener};

#[cfg(target_os = "windows")]
mod native {
    use super::{listener_pid, process_name};
    use crate::{models::PlatformInfo, platform::PortProcess};
    use std::{
        mem::{offset_of, size_of},
        slice,
    };
    use windows::{
        core::PWSTR,
        Wdk::System::SystemServices::RtlGetVersion,
        Win32::{
            Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS},
            NetworkManagement::IpHelper::{
                GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
                TCP_TABLE_OWNER_PID_LISTENER,
            },
            Networking::WinSock::{AF_INET, AF_INET6},
            System::{
                SystemInformation::OSVERSIONINFOW,
                Threading::{
                    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                    PROCESS_QUERY_LIMITED_INFORMATION,
                },
            },
        },
    };

    pub fn platform_info() -> PlatformInfo {
        let mut version = OSVERSIONINFOW {
            dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32,
            ..Default::default()
        };
        // RtlGetVersion reports the actual OS version without compatibility-manifest virtualization.
        let version = if unsafe { RtlGetVersion(&mut version) }.0 >= 0 {
            format!(
                "{}.{}.{}",
                version.dwMajorVersion, version.dwMinorVersion, version.dwBuildNumber
            )
        } else {
            "unknown".into()
        };
        PlatformInfo {
            os: "windows".into(),
            version,
            arch: std::env::consts::ARCH.into(),
            certificate_trust_supported: true,
        }
    }

    pub fn port_listener(port: u16) -> Option<PortProcess> {
        let pid = table_pid(
            AF_INET.0 as u32,
            port,
            size_of::<MIB_TCPROW_OWNER_PID>(),
            offset_of!(MIB_TCPROW_OWNER_PID, dwLocalPort),
            offset_of!(MIB_TCPROW_OWNER_PID, dwOwningPid),
        )
        .or_else(|| {
            table_pid(
                AF_INET6.0 as u32,
                port,
                size_of::<MIB_TCP6ROW_OWNER_PID>(),
                offset_of!(MIB_TCP6ROW_OWNER_PID, dwLocalPort),
                offset_of!(MIB_TCP6ROW_OWNER_PID, dwOwningPid),
            )
        })?;
        Some(PortProcess {
            pid,
            name: image_name(pid),
        })
    }

    fn table_pid(
        family: u32,
        port: u16,
        row_size: usize,
        port_offset: usize,
        pid_offset: usize,
    ) -> Option<u32> {
        let mut size = 0;
        let result = unsafe {
            GetExtendedTcpTable(
                None,
                &mut size,
                false,
                family,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if result != ERROR_INSUFFICIENT_BUFFER.0 {
            return None;
        }
        // The listener set can grow between sizing and reading. Retry with the new size.
        for _ in 0..3 {
            let mut buffer = vec![0u32; (size as usize).div_ceil(size_of::<u32>())];
            let capacity = buffer.len() * size_of::<u32>();
            let result = unsafe {
                GetExtendedTcpTable(
                    Some(buffer.as_mut_ptr().cast()),
                    &mut size,
                    false,
                    family,
                    TCP_TABLE_OWNER_PID_LISTENER,
                    0,
                )
            };
            if result == ERROR_INSUFFICIENT_BUFFER.0 {
                continue;
            }
            if result != ERROR_SUCCESS.0 || size as usize > capacity {
                return None;
            }
            let bytes =
                unsafe { slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), size as usize) };
            return listener_pid(bytes, row_size, port_offset, pid_offset, port);
        }
        None
    }

    fn image_name(pid: u32) -> Option<String> {
        // Protected/elevated processes can deny this right; retain the PID in that case.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
        let mut path = vec![0u16; 32768];
        let mut size = path.len() as u32;
        let result = unsafe {
            QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(path.as_mut_ptr()),
                &mut size,
            )
        };
        let _ = unsafe { CloseHandle(handle) };
        result.ok()?;
        process_name(&String::from_utf16_lossy(path.get(..size as usize)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tcp_rows_use_network_order_port_and_reject_truncated_tables() {
        // IPv4 OWNER_PID: state, local address/port, remote address/port, PID.
        let mut bytes = 1u32.to_ne_bytes().to_vec();
        bytes.extend_from_slice(&2u32.to_ne_bytes());
        bytes.extend_from_slice(&0u32.to_ne_bytes());
        bytes.extend_from_slice(&[0x6D, 0xB6, 0, 0]); // port 28086
        bytes.extend_from_slice(&[0; 8]);
        bytes.extend_from_slice(&42u32.to_ne_bytes());
        assert_eq!(listener_pid(&bytes, 24, 8, 20, 28086), Some(42));
        assert_eq!(listener_pid(&bytes, 24, 8, 20, 8888), None);
        assert_eq!(listener_pid(&bytes[..27], 24, 8, 20, 28086), None);
        bytes[..4].copy_from_slice(&2u32.to_ne_bytes());
        assert_eq!(listener_pid(&bytes, 24, 8, 20, 28086), None);
    }

    #[test]
    fn tcp6_rows_resolve_pid_without_exposing_process_path() {
        let mut bytes = vec![0; 60];
        bytes[..4].copy_from_slice(&1u32.to_ne_bytes());
        bytes[24..28].copy_from_slice(&[0x01, 0xBB, 0, 0]); // port 443
        bytes[56..60].copy_from_slice(&77u32.to_ne_bytes());
        assert_eq!(listener_pid(&bytes, 56, 20, 52, 443), Some(77));
        assert_eq!(
            process_name(r"C:\Program Files\Example\process.exe"),
            Some("process.exe".into())
        );
        assert_eq!(process_name(""), None);
    }
}
