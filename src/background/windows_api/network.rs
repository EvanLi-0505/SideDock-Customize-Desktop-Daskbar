//! Internet connection of the machine, for the network status icon.

use serde::Serialize;
use windows::Networking::Connectivity::{NetworkConnectivityLevel, NetworkInformation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NetworkKind {
    /// not connected to any network
    None,
    Wifi,
    /// wired, or anything that is neither Wi-Fi nor mobile (VPN adapters included)
    Ethernet,
    Cellular,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatus {
    pub kind: NetworkKind,
    /// the connection reaches the internet (not only the local network)
    pub internet: bool,
    /// 0-5 bars, wireless connections only
    pub signal: Option<u8>,
    /// network name (SSID for Wi-Fi)
    pub name: String,
}

impl NetworkStatus {
    fn disconnected() -> Self {
        Self {
            kind: NetworkKind::None,
            internet: false,
            signal: None,
            name: String::new(),
        }
    }
}

pub fn status() -> NetworkStatus {
    // no profile (an error or a null object) = no connection
    let Ok(profile) = NetworkInformation::GetInternetConnectionProfile() else {
        return NetworkStatus::disconnected();
    };
    let level = profile
        .GetNetworkConnectivityLevel()
        .unwrap_or(NetworkConnectivityLevel::None);
    if level == NetworkConnectivityLevel::None {
        return NetworkStatus::disconnected();
    }
    let kind = if profile.IsWlanConnectionProfile().unwrap_or(false) {
        NetworkKind::Wifi
    } else if profile.IsWwanConnectionProfile().unwrap_or(false) {
        NetworkKind::Cellular
    } else {
        NetworkKind::Ethernet
    };
    NetworkStatus {
        kind,
        internet: level == NetworkConnectivityLevel::InternetAccess,
        signal: profile
            .GetSignalBars()
            .ok()
            .and_then(|bars| bars.Value().ok()),
        name: profile
            .ProfileName()
            .map(|n| n.to_string())
            .unwrap_or_default(),
    }
}
