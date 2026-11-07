//! The NetworkManager backend: Wi-Fi over its D-Bus API (`org.freedesktop.NetworkManager`).
//! One blocking D-Bus connection; every call is a round trip to the daemon.

use crate::{Backend, Error, Network, Result, Security, Status, merge};
use std::collections::HashMap;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

const DEST: &str = "org.freedesktop.NetworkManager";
const ROOT: &str = "/org/freedesktop/NetworkManager";
const IF_ROOT: &str = "org.freedesktop.NetworkManager";
const IF_DEVICE: &str = "org.freedesktop.NetworkManager.Device";
const IF_WIRELESS: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const IF_AP: &str = "org.freedesktop.NetworkManager.AccessPoint";
const SETTINGS_PATH: &str = "/org/freedesktop/NetworkManager/Settings";
const IF_SETTINGS: &str = "org.freedesktop.NetworkManager.Settings";
const IF_CONNECTION: &str = "org.freedesktop.NetworkManager.Settings.Connection";

/// `NMDeviceType` of a Wi-Fi interface.
const DEVICE_TYPE_WIFI: u32 = 2;
/// `NM80211ApFlags`: the access point needs a password (WEP when no WPA flags are set).
const AP_FLAGS_PRIVACY: u32 = 0x1;
/// `NM80211ApSecurityFlags`: 802.1X key management.
const AP_SEC_KEY_MGMT_802_1X: u32 = 0x200;

type Settings<'a> = HashMap<&'a str, HashMap<&'a str, Value<'a>>>;
type SavedSettings = HashMap<String, HashMap<String, OwnedValue>>;

pub struct Nm {
    conn: Connection,
    dest: String,
}

/// The security of an access point from its flags.
pub fn security_of(flags: u32, wpa: u32, rsn: u32) -> Security {
    let both = wpa | rsn;
    if both == 0 {
        if flags & AP_FLAGS_PRIVACY != 0 { Security::Wep } else { Security::Open }
    } else if both & AP_SEC_KEY_MGMT_802_1X != 0 {
        Security::Enterprise
    } else {
        Security::Psk
    }
}

/// An SSID as text (it is bytes, usually UTF-8).
pub fn ssid_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

impl Nm {
    /// Connect to the system bus, where NetworkManager lives.
    pub fn system() -> Result<Nm> {
        let conn = Connection::system()
            .map_err(|e| Error(format!("cannot reach the system D-Bus: {e}")))?;
        Ok(Nm { conn, dest: DEST.to_string() })
    }

    /// Use another bus and bus name (tests run a pretend NetworkManager on a private bus).
    pub fn with_connection(conn: Connection, dest: &str) -> Nm {
        Nm { conn, dest: dest.to_string() }
    }

    fn proxy(&self, path: &str, interface: &str) -> Result<Proxy<'static>> {
        Ok(Proxy::new(&self.conn, self.dest.clone(), path.to_string(), interface.to_string())?)
    }

    /// The first Wi-Fi device: its path and interface name.
    fn wifi_device(&self) -> Result<Option<(OwnedObjectPath, String)>> {
        let root = self.proxy(ROOT, IF_ROOT)?;
        let devices: Vec<OwnedObjectPath> = root.call("GetDevices", &())?;
        for path in devices {
            let dev = self.proxy(path.as_str(), IF_DEVICE)?;
            let kind: u32 = dev.get_property("DeviceType")?;
            if kind == DEVICE_TYPE_WIFI {
                let name: String = dev.get_property("Interface")?;
                return Ok(Some((path, name)));
            }
        }
        Ok(None)
    }

    fn need_wifi_device(&self) -> Result<OwnedObjectPath> {
        self.wifi_device()?.map(|(p, _)| p).ok_or_else(|| Error("no Wi-Fi device".into()))
    }

    /// Saved Wi-Fi connections: SSID and the connection's path.
    fn saved(&self) -> Result<Vec<(String, OwnedObjectPath)>> {
        let settings = self.proxy(SETTINGS_PATH, IF_SETTINGS)?;
        let paths: Vec<OwnedObjectPath> = settings.call("ListConnections", &())?;
        let mut out = Vec::new();
        for path in paths {
            let conn = self.proxy(path.as_str(), IF_CONNECTION)?;
            let Ok(all): std::result::Result<SavedSettings, _> = conn.call("GetSettings", &())
            else {
                continue;
            };
            let Some(wireless) = all.get("802-11-wireless") else { continue };
            let Some(ssid) = wireless.get("ssid").and_then(|v| Vec::<u8>::try_from(v.clone()).ok())
            else {
                continue;
            };
            out.push((ssid_text(&ssid), path));
        }
        Ok(out)
    }

    /// Every visible access point as a network, with the active one marked.
    fn access_points(&self, device: &OwnedObjectPath) -> Result<Vec<(OwnedObjectPath, Network)>> {
        let wireless = self.proxy(device.as_str(), IF_WIRELESS)?;
        let active: OwnedObjectPath = wireless.get_property("ActiveAccessPoint")?;
        let paths: Vec<OwnedObjectPath> = wireless.call("GetAccessPoints", &())?;
        let saved: Vec<String> = self.saved()?.into_iter().map(|(s, _)| s).collect();
        let mut out = Vec::new();
        for path in paths {
            let ap = self.proxy(path.as_str(), IF_AP)?;
            let ssid = ssid_text(&ap.get_property::<Vec<u8>>("Ssid")?);
            let flags: u32 = ap.get_property("Flags")?;
            let wpa: u32 = ap.get_property("WpaFlags")?;
            let rsn: u32 = ap.get_property("RsnFlags")?;
            out.push((
                path.clone(),
                Network {
                    known: saved.contains(&ssid),
                    active: active.as_str() != "/" && path == active,
                    ssid,
                    strength: ap.get_property("Strength")?,
                    security: security_of(flags, wpa, rsn),
                    frequency_mhz: ap.get_property("Frequency")?,
                    bssid: ap.get_property("HwAddress")?,
                },
            ));
        }
        Ok(out)
    }

    fn strongest_ap(&self, device: &OwnedObjectPath, ssid: &str) -> Result<OwnedObjectPath> {
        self.access_points(device)?
            .into_iter()
            .filter(|(_, n)| n.ssid == ssid)
            .max_by_key(|(_, n)| n.strength)
            .map(|(p, _)| p)
            .ok_or_else(|| Error(format!("{ssid} is not in range")))
    }
}

impl Backend for Nm {
    fn status(&self) -> Result<Status> {
        let root = self.proxy(ROOT, IF_ROOT)?;
        let enabled: bool = root.get_property("WirelessEnabled")?;
        let Some((device, name)) = self.wifi_device()? else {
            return Ok(Status { device: None, enabled, active: None });
        };
        let active =
            self.access_points(&device)?.into_iter().find(|(_, n)| n.active).map(|(_, n)| n.ssid);
        Ok(Status { device: Some(name), enabled, active })
    }

    fn networks(&self) -> Result<Vec<Network>> {
        let Some((device, _)) = self.wifi_device()? else { return Ok(Vec::new()) };
        Ok(merge(self.access_points(&device)?.into_iter().map(|(_, n)| n).collect()))
    }

    fn scan(&self) -> Result<()> {
        let device = self.need_wifi_device()?;
        let wireless = self.proxy(device.as_str(), IF_WIRELESS)?;
        let options: HashMap<&str, Value<'_>> = HashMap::new();
        wireless.call::<_, _, ()>("RequestScan", &(options,))?;
        Ok(())
    }

    fn set_enabled(&self, on: bool) -> Result<()> {
        let root = self.proxy(ROOT, IF_ROOT)?;
        root.set_property("WirelessEnabled", on)?;
        Ok(())
    }

    fn connect(&self, ssid: &str, password: Option<&str>) -> Result<()> {
        let device = self.need_wifi_device()?;
        let ap = self.strongest_ap(&device, ssid)?;
        let network = self
            .access_points(&device)?
            .into_iter()
            .find(|(p, _)| *p == ap)
            .map(|(_, n)| n)
            .ok_or_else(|| Error(format!("{ssid} is not in range")))?;
        let root = self.proxy(ROOT, IF_ROOT)?;
        // A saved connection is activated as it is.
        if let Some((_, saved)) = self.saved()?.into_iter().find(|(s, _)| s == ssid) {
            root.call::<_, _, OwnedObjectPath>("ActivateConnection", &(saved, device, ap))?;
            return Ok(());
        }
        if network.security == Security::Enterprise {
            return Err(Error("802.1X networks are not supported yet".into()));
        }
        // A new one is created and activated in one step; NetworkManager keeps it for next time.
        let mut connection: Settings<'_> = HashMap::new();
        connection.insert(
            "connection",
            HashMap::from([("type", Value::from("802-11-wireless")), ("id", Value::from(ssid))]),
        );
        connection.insert(
            "802-11-wireless",
            HashMap::from([
                ("ssid", Value::from(ssid.as_bytes().to_vec())),
                ("mode", Value::from("infrastructure")),
            ]),
        );
        if !network.security.is_open() {
            let password = password
                .filter(|p| !p.is_empty())
                .ok_or_else(|| Error("a password is needed".into()))?;
            let key_mgmt = if network.security == Security::Wep { "none" } else { "wpa-psk" };
            let mut security = HashMap::from([("key-mgmt", Value::from(key_mgmt))]);
            security.insert(
                if network.security == Security::Wep { "wep-key0" } else { "psk" },
                Value::from(password),
            );
            connection.insert("802-11-wireless-security", security);
        }
        root.call::<_, _, (OwnedObjectPath, OwnedObjectPath)>(
            "AddAndActivateConnection",
            &(connection, device, ap),
        )?;
        Ok(())
    }

    fn disconnect(&self) -> Result<()> {
        let device = self.need_wifi_device()?;
        self.proxy(device.as_str(), IF_DEVICE)?.call::<_, _, ()>("Disconnect", &())?;
        Ok(())
    }

    fn forget(&self, ssid: &str) -> Result<()> {
        let paths: Vec<OwnedObjectPath> =
            self.saved()?.into_iter().filter(|(s, _)| s == ssid).map(|(_, p)| p).collect();
        if paths.is_empty() {
            return Err(Error(format!("no saved connection for {ssid}")));
        }
        for path in paths {
            self.proxy(path.as_str(), IF_CONNECTION)?.call::<_, _, ()>("Delete", &())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
