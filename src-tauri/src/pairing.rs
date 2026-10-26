use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex, RwLock},
};

use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};

use crate::{
    error::AppError,
    models::{DeviceIdentity, PairedDevice, PairingChallenge},
    storage::Store,
};

const DEFAULT_IDLE_TIMEOUT_MINUTES: u16 = 15;
const CHALLENGE_LIFETIME_MINUTES: i64 = 5;
const IDLE_TIMEOUT_KEY: &str = "pairing_idle_timeout_minutes";

pub trait PairingEventSink: Send + Sync + 'static {
    fn devices_changed(&self, _devices: &[PairedDevice]) {}
}

pub struct NoopPairingEventSink;
impl PairingEventSink for NoopPairingEventSink {}

struct Challenge {
    expires_at: DateTime<Utc>,
    target_device_token: Option<String>,
}

pub struct PairingService {
    store: Arc<Store>,
    event_sink: Arc<dyn PairingEventSink>,
    challenges: Mutex<HashMap<String, Challenge>>,
    devices_by_ip: RwLock<HashMap<IpAddr, PairedDevice>>,
    idle_timeout_minutes: RwLock<u16>,
}

impl PairingService {
    pub fn new(store: Arc<Store>, event_sink: Arc<dyn PairingEventSink>) -> Self {
        let timeout = store
            .get_setting(IDLE_TIMEOUT_KEY)
            .ok()
            .flatten()
            .and_then(|value| value.parse().ok())
            .filter(|value| (5..=120).contains(value))
            .unwrap_or(DEFAULT_IDLE_TIMEOUT_MINUTES);
        let devices_by_ip = store
            .list_paired_devices()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|device| device.bound_ip.parse().ok().map(|ip| (ip, device)))
            .collect();
        Self {
            store,
            event_sink,
            challenges: Mutex::new(HashMap::new()),
            devices_by_ip: RwLock::new(devices_by_ip),
            idle_timeout_minutes: RwLock::new(timeout),
        }
    }

    pub fn create_challenge(
        &self,
        lan_ip: &str,
        port: u16,
        target_device_token: Option<String>,
    ) -> Result<PairingChallenge, AppError> {
        if lan_ip.parse::<IpAddr>().is_err() {
            return Err(AppError::new("PAIRING_ADDRESS_INVALID", "手机配对地址无效"));
        }
        let raw = uuid::Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::minutes(CHALLENGE_LIFETIME_MINUTES);
        self.challenges
            .lock()
            .expect("pairing challenge lock poisoned")
            .insert(
                hash_token(&raw),
                Challenge {
                    expires_at,
                    target_device_token,
                },
            );
        Ok(PairingChallenge {
            url: format!("http://{lan_ip}:{port}/__netcat/pair/{raw}"),
            expires_at: expires_at.to_rfc3339(),
        })
    }

    pub fn consume_challenge(&self, raw: &str, ip: IpAddr) -> Result<PairedDevice, AppError> {
        let challenge = self
            .challenges
            .lock()
            .expect("pairing challenge lock poisoned")
            .remove(&hash_token(raw))
            .ok_or_else(|| AppError::new("PAIRING_CHALLENGE_INVALID", "配对二维码无效或已使用"))?;
        if challenge.expires_at < Utc::now() {
            return Err(AppError::new(
                "PAIRING_CHALLENGE_EXPIRED",
                "配对二维码已过期",
            ));
        }
        let existing = challenge.target_device_token.as_deref().and_then(|target| {
            self.store
                .list_paired_devices()
                .ok()?
                .into_iter()
                .find(|device| device.device_token == target)
                .or_else(|| self.store.historical_device(target).ok().flatten())
        });
        let now = Utc::now().to_rfc3339();
        let device = if let Some(existing) = existing {
            PairedDevice {
                bound_ip: ip.to_string(),
                last_seen_at: now,
                ..existing
            }
        } else {
            PairedDevice {
                device_token: uuid::Uuid::new_v4().to_string(),
                name: self.store.next_device_name()?,
                bound_ip: ip.to_string(),
                created_at: now.clone(),
                last_seen_at: now,
            }
        };
        self.store.save_paired_device(&device)?;
        let mut devices = self
            .devices_by_ip
            .write()
            .expect("paired device lock poisoned");
        devices.retain(|_, current| current.device_token != device.device_token);
        devices.insert(ip, device.clone());
        self.notify();
        Ok(device)
    }

    pub fn authorize(&self, ip: IpAddr) -> Option<DeviceIdentity> {
        let mut devices = self
            .devices_by_ip
            .write()
            .expect("paired device lock poisoned");
        let device = devices.get_mut(&ip)?;
        device.last_seen_at = Utc::now().to_rfc3339();
        Some(DeviceIdentity {
            device_token: device.device_token.clone(),
            device_name: device.name.clone(),
        })
    }

    pub fn list_devices(&self) -> Result<Vec<PairedDevice>, AppError> {
        self.store.list_paired_devices()
    }

    pub fn set_idle_timeout_minutes(&self, minutes: u16) -> Result<u16, AppError> {
        if !(5..=120).contains(&minutes) {
            return Err(AppError::new(
                "PAIRING_TIMEOUT_INVALID",
                "空闲断开时间必须在 5 到 120 分钟之间",
            ));
        }
        self.store
            .set_setting(IDLE_TIMEOUT_KEY, &minutes.to_string())?;
        *self
            .idle_timeout_minutes
            .write()
            .expect("pairing timeout lock poisoned") = minutes;
        self.cleanup_expired_at(Utc::now())?;
        Ok(minutes)
    }

    pub fn idle_timeout_minutes(&self) -> u16 {
        *self
            .idle_timeout_minutes
            .read()
            .expect("pairing timeout lock poisoned")
    }

    pub fn cleanup_expired_at(&self, now: DateTime<Utc>) -> Result<(), AppError> {
        let cutoff = now - Duration::minutes(self.idle_timeout_minutes() as i64);
        let devices = self
            .devices_by_ip
            .read()
            .expect("paired device lock poisoned")
            .values()
            .cloned()
            .collect::<Vec<_>>();
        self.store.save_paired_devices(&devices)?;
        let expired = devices
            .into_iter()
            .filter(|device| {
                DateTime::parse_from_rfc3339(&device.last_seen_at)
                    .map(|timestamp| timestamp.with_timezone(&Utc) < cutoff)
                    .unwrap_or(true)
            })
            .collect::<Vec<_>>();
        for device in expired {
            self.disconnect_device(&device.device_token)?;
        }
        Ok(())
    }

    pub fn disconnect_device(&self, device_token: &str) -> Result<(), AppError> {
        self.store.remove_paired_device(device_token)?;
        self.devices_by_ip
            .write()
            .expect("paired device lock poisoned")
            .retain(|_, device| device.device_token != device_token);
        self.notify();
        Ok(())
    }

    pub fn rename_device(
        &self,
        device_token: &str,
        requested_name: &str,
    ) -> Result<PairedDevice, AppError> {
        let name = requested_name.trim();
        if name.is_empty() || name.chars().count() > 40 {
            return Err(AppError::new(
                "PAIRED_DEVICE_NAME_INVALID",
                "设备名称必须为 1 到 40 个字符",
            ));
        }
        if self.store.list_paired_devices()?.iter().any(|device| {
            device.device_token != device_token && device.name.eq_ignore_ascii_case(name)
        }) {
            return Err(AppError::new(
                "PAIRED_DEVICE_NAME_CONFLICT",
                "设备名称已被使用",
            ));
        }
        self.store.rename_paired_device(device_token, name)?;
        let mut devices = self
            .devices_by_ip
            .write()
            .expect("paired device lock poisoned");
        let renamed = if let Some(device) = devices
            .values_mut()
            .find(|device| device.device_token == device_token)
        {
            device.name = name.to_string();
            device.clone()
        } else {
            self.store
                .historical_device(device_token)?
                .ok_or_else(|| AppError::new("PAIRED_DEVICE_NOT_FOUND", "未找到设备"))?
        };
        drop(devices);
        self.notify();
        Ok(renamed)
    }

    pub fn disconnect_all(&self) -> Result<(), AppError> {
        self.store.clear_paired_devices()?;
        self.devices_by_ip
            .write()
            .expect("paired device lock poisoned")
            .clear();
        self.challenges
            .lock()
            .expect("pairing challenge lock poisoned")
            .clear();
        self.notify();
        Ok(())
    }

    pub fn delete_device_data(&self, device_token: &str) -> Result<usize, AppError> {
        let deleted = self.store.delete_device_data(device_token)?;
        self.devices_by_ip
            .write()
            .expect("paired device lock poisoned")
            .retain(|_, device| device.device_token != device_token);
        self.notify();
        Ok(deleted)
    }

    fn notify(&self) {
        self.event_sink
            .devices_changed(&self.store.list_paired_devices().unwrap_or_default());
    }
}

fn hash_token(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
