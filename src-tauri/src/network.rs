use crate::models::{MatchKind, NetworkProfile, NetworkProfileKind};
use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkFailure {
    Timeout,
    Disconnect,
}

#[derive(Debug, Clone, Default)]
pub struct NetworkConditioner {
    profile: Option<NetworkProfile>,
}

impl NetworkConditioner {
    pub fn new(profile: Option<NetworkProfile>) -> Self {
        Self { profile }
    }

    pub fn profile(&self) -> Option<&NetworkProfile> {
        self.profile.as_ref()
    }

    pub fn matches(&self, method: &str, url: &str) -> bool {
        let Some(profile) = &self.profile else {
            return false;
        };
        let method_matches = profile.methods.is_empty()
            || profile
                .methods
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(method));
        method_matches
            && match profile.match_kind {
                MatchKind::Exact => url == profile.url_pattern,
                MatchKind::Contains => url.contains(&profile.url_pattern),
                MatchKind::Regex => Regex::new(&profile.url_pattern)
                    .map(|pattern| pattern.is_match(url))
                    .unwrap_or(false),
            }
    }

    pub fn delay_ms(&self, direction: TransferDirection, bytes: usize, jitter_ms: u64) -> u64 {
        let Some(profile) = &self.profile else {
            return 0;
        };
        let kbps = match direction {
            TransferDirection::Upload => profile.upload_kbps,
            TransferDirection::Download => profile.download_kbps,
        };
        let transfer_ms = kbps
            .filter(|rate| *rate > 0)
            .map(|rate| (bytes as u64).saturating_mul(8).div_ceil(rate))
            .unwrap_or(0);
        profile
            .latency_ms
            .saturating_add(jitter_ms)
            .saturating_add(transfer_ms)
    }

    pub fn failure_for_sample(&self, sample: u8) -> Option<NetworkFailure> {
        let profile = self.profile.as_ref()?;
        let sample = sample.min(99);
        if sample < profile.timeout_percent {
            Some(NetworkFailure::Timeout)
        } else if sample
            < profile
                .timeout_percent
                .saturating_add(profile.disconnect_percent)
        {
            Some(NetworkFailure::Disconnect)
        } else {
            None
        }
    }
}

pub fn builtin_network_profiles() -> Vec<NetworkProfile> {
    vec![
        preset("fast-4g", "高速 4G", 20, Some(9_000), Some(1_500), 0),
        preset("slow-4g", "低速 4G", 150, Some(1_600), Some(750), 0),
        preset("3g", "3G", 100, Some(750), Some(250), 0),
        preset("offline", "离线", 0, None, None, 100),
    ]
}

fn preset(
    id: &str,
    name: &str,
    latency_ms: u64,
    download_kbps: Option<u64>,
    upload_kbps: Option<u64>,
    timeout_percent: u8,
) -> NetworkProfile {
    NetworkProfile {
        id: id.into(),
        name: name.into(),
        kind: NetworkProfileKind::Preset,
        latency_ms,
        jitter_ms: 0,
        download_kbps,
        upload_kbps,
        timeout_percent,
        disconnect_percent: 0,
        methods: Vec::new(),
        match_kind: MatchKind::Contains,
        url_pattern: String::new(),
        builtin: true,
        created_at: "2026-10-26T00:00:00Z".into(),
    }
}
