//! Native macOS backends: Wi-Fi through CoreWLAN and Bluetooth through IOBluetooth, behind the
//! same traits as the Linux ones. Only built with the `macos` feature on macOS.
//!
//! Limits, from what the frameworks allow:
//! - Wi-Fi network names are hidden by macOS unless the app that runs `nu` (your terminal) may use
//!   Location Services (System Settings, Privacy & Security, Location Services).
//! - CoreWLAN cannot list or delete saved networks without administrator rights: `forget` is not
//!   supported, and `known` is read from the saved profiles when macOS lets us.
//! - Bluetooth: the paired devices are listed and connected or disconnected; pairing a new device,
//!   scanning and the power switch need delegates or private API and are not supported yet.

use crate::bluetooth::{Bluetooth, BtStatus, Device, Kind, sort};
use crate::{Backend, Error, Network, Result, Security, Status, merge};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_core_wlan::{CWInterface, CWNetwork, CWSecurity, CWWiFiClient};
use objc2_foundation::{NSError, NSString};
use objc2_io_bluetooth::{IOBluetoothDevice, IOBluetoothHostController};

fn ns_error(e: Retained<NSError>) -> Error {
    Error(e.localizedDescription().to_string())
}

/// Wi-Fi through CoreWLAN.
pub struct MacWifi;

impl MacWifi {
    pub fn new() -> MacWifi {
        MacWifi
    }

    fn interface(&self) -> Result<Retained<CWInterface>> {
        let client = unsafe { CWWiFiClient::sharedWiFiClient() };
        unsafe { client.interface() }.ok_or_else(|| Error("no Wi-Fi interface on this Mac".into()))
    }

    /// The saved SSIDs, when macOS lets us read the profiles.
    fn saved(&self, iface: &CWInterface) -> Vec<String> {
        let Some(config) = (unsafe { iface.configuration() }) else { return Vec::new() };
        let profiles = unsafe { config.networkProfiles() };
        (0..profiles.count()).filter_map(|i| unsafe { profiles.objectAtIndex(i).ssid() }).map(|s| s.to_string()).collect()
    }

    fn security_of(n: &CWNetwork) -> Security {
        let supports = |s: CWSecurity| unsafe { n.supportsSecurity(s) };
        if supports(CWSecurity::None) {
            Security::Open
        } else if supports(CWSecurity::WPAEnterprise) || supports(CWSecurity::WPAEnterpriseMixed) || supports(CWSecurity::WPA2Enterprise) || supports(CWSecurity::WPA3Enterprise) {
            Security::Enterprise
        } else if supports(CWSecurity::WEP) || supports(CWSecurity::DynamicWEP) {
            Security::Wep
        } else {
            Security::Psk
        }
    }

    /// The scanned networks: the last scan if there is one, else a new (blocking) scan.
    fn scanned(&self, iface: &CWInterface, fresh: bool) -> Result<Vec<Retained<CWNetwork>>> {
        if !fresh {
            if let Some(cached) = unsafe { iface.cachedScanResults() } {
                if cached.len() > 0 {
                    return Ok(cached.to_vec());
                }
            }
        }
        let set = unsafe { iface.scanForNetworksWithName_error(None) }.map_err(ns_error)?;
        Ok(set.to_vec())
    }

    fn network_of(&self, n: &CWNetwork, active: Option<&str>, saved: &[String]) -> Network {
        let ssid = unsafe { n.ssid() }.map(|s| s.to_string()).unwrap_or_default();
        let rssi = unsafe { n.rssiValue() } as i16;
        let (frequency_mhz, bssid) = (
            unsafe { n.wlanChannel() }.map(|c| channel_mhz(unsafe { c.channelNumber() } as u32)).unwrap_or(0),
            unsafe { n.bssid() }.map(|b| b.to_string()).unwrap_or_default(),
        );
        Network { active: active == Some(ssid.as_str()), known: saved.contains(&ssid), security: Self::security_of(n), strength: crate::bluetooth::rssi_percent(rssi), frequency_mhz, bssid, ssid }
    }
}

impl Default for MacWifi {
    fn default() -> Self {
        MacWifi::new()
    }
}

/// The centre frequency of a Wi-Fi channel number (2.4 GHz channels 1 to 14, else 5 GHz and up).
fn channel_mhz(channel: u32) -> u32 {
    match channel {
        14 => 2484,
        1..=13 => 2407 + 5 * channel,
        _ => 5000 + 5 * channel,
    }
}

impl Backend for MacWifi {
    fn status(&self) -> Result<Status> {
        let iface = self.interface()?;
        Ok(Status {
            device: unsafe { iface.interfaceName() }.map(|n| n.to_string()),
            enabled: unsafe { iface.powerOn() },
            active: unsafe { iface.ssid() }.map(|s| s.to_string()),
        })
    }

    fn networks(&self) -> Result<Vec<Network>> {
        let iface = self.interface()?;
        let active = unsafe { iface.ssid() }.map(|s| s.to_string());
        let saved = self.saved(&iface);
        let raw = self.scanned(&iface, false)?;
        // Without Location Services macOS returns networks with no name.
        if !raw.is_empty() && raw.iter().all(|n| unsafe { n.ssid() }.is_none()) {
            return Err(Error("macOS hides network names until Location Services allows this terminal (System Settings, Privacy & Security, Location Services)".into()));
        }
        Ok(merge(raw.iter().map(|n| self.network_of(n, active.as_deref(), &saved)).collect()))
    }

    fn scan(&self) -> Result<()> {
        // CoreWLAN scans synchronously: this blocks for a few seconds.
        let iface = self.interface()?;
        self.scanned(&iface, true).map(|_| ())
    }

    fn set_enabled(&self, on: bool) -> Result<()> {
        unsafe { self.interface()?.setPower_error(on) }.map_err(ns_error)
    }

    fn connect(&self, ssid: &str, password: Option<&str>) -> Result<()> {
        let iface = self.interface()?;
        let target = self.scanned(&iface, false)?.into_iter().find(|n| unsafe { n.ssid() }.is_some_and(|s| s.to_string() == ssid)).ok_or_else(|| Error(format!("{ssid} is not in range")))?;
        let password = password.map(NSString::from_str);
        unsafe { iface.associateToNetwork_password_error(&target, password.as_deref()) }.map_err(ns_error)
    }

    fn disconnect(&self) -> Result<()> {
        unsafe { self.interface()?.disassociate() };
        Ok(())
    }

    fn forget(&self, _ssid: &str) -> Result<()> {
        Err(Error("forgetting a network needs administrator rights on macOS: use System Settings, Wi-Fi, Advanced".into()))
    }
}

/// Bluetooth through IOBluetooth (classic devices).
pub struct MacBluetooth;

impl MacBluetooth {
    pub fn new() -> MacBluetooth {
        MacBluetooth
    }

    fn paired(&self) -> Vec<Retained<IOBluetoothDevice>> {
        let Some(list) = (unsafe { IOBluetoothDevice::pairedDevices() }) else { return Vec::new() };
        list.to_vec().into_iter().filter_map(|o: Retained<AnyObject>| o.downcast::<IOBluetoothDevice>().ok()).collect()
    }

    fn find(&self, address: &str) -> Result<Retained<IOBluetoothDevice>> {
        let wanted = normalize(address);
        self.paired().into_iter().find(|d| unsafe { d.addressString() }.is_some_and(|a| normalize(&a.to_string()) == wanted)).ok_or_else(|| Error(format!("{address} is not paired")))
    }
}

impl Default for MacBluetooth {
    fn default() -> Self {
        MacBluetooth::new()
    }
}

/// `aa-bb-cc` and `AA:BB:CC` are the same address.
fn normalize(address: &str) -> String {
    address.to_lowercase().replace('-', ":")
}

/// The kind of a device from its Bluetooth class of device (major class and minor class bits).
fn kind_of(major: u32, minor: u32) -> Kind {
    match major {
        0x01 => Kind::Computer,
        0x02 => Kind::Phone,
        0x04 => Kind::Audio,
        // Peripheral: bits 6 and 7 of the minor class say keyboard, pointing device or both.
        0x05 => match (minor >> 4) & 0x3 {
            1 => Kind::Keyboard,
            2 => Kind::Mouse,
            _ => match minor & 0xf {
                1 | 2 => Kind::Gamepad,
                _ => Kind::Other,
            },
        },
        _ => Kind::Other,
    }
}

impl Bluetooth for MacBluetooth {
    fn status(&self) -> Result<BtStatus> {
        let Some(controller) = (unsafe { IOBluetoothHostController::defaultController() }) else { return Ok(BtStatus::default()) };
        // BluetoothHCIPowerState: 1 is on.
        Ok(BtStatus { adapter: Some("default".into()), powered: unsafe { controller.powerState() }.0 == 1, discovering: false })
    }

    fn devices(&self) -> Result<Vec<Device>> {
        let mut out: Vec<Device> = self
            .paired()
            .iter()
            .map(|d| {
                let rssi = unsafe { d.rawRSSI() };
                let connected = unsafe { d.isConnected() };
                Device {
                    address: unsafe { d.addressString() }.map(|a| a.to_string()).unwrap_or_default(),
                    name: unsafe { d.nameOrAddress() }.map(|n| n.to_string()).unwrap_or_default(),
                    kind: kind_of(unsafe { d.deviceClassMajor() } as u32, unsafe { d.deviceClassMinor() } as u32),
                    paired: true,
                    trusted: unsafe { d.isFavorite() },
                    connected,
                    // 127 means "not available".
                    rssi: (connected && rssi != 127).then_some(i16::from(rssi)),
                }
            })
            .collect();
        sort(&mut out);
        Ok(out)
    }

    fn scan(&self, _on: bool) -> Result<()> {
        Err(Error("looking for new devices is not supported on macOS yet: pair them in System Settings, Bluetooth".into()))
    }

    fn set_powered(&self, _on: bool) -> Result<()> {
        Err(Error("the Bluetooth power switch is not available to apps on macOS: use Control Centre".into()))
    }

    fn connect(&self, address: &str) -> Result<()> {
        let device = self.find(address)?;
        // IOReturn 0 is success.
        match unsafe { device.openConnection() } {
            0 => Ok(()),
            code => Err(Error(format!("connecting failed (IOReturn {code:#x})"))),
        }
    }

    fn disconnect(&self, address: &str) -> Result<()> {
        match unsafe { self.find(address)?.closeConnection() } {
            0 => Ok(()),
            code => Err(Error(format!("disconnecting failed (IOReturn {code:#x})"))),
        }
    }

    fn set_trusted(&self, address: &str, on: bool) -> Result<()> {
        let device = self.find(address)?;
        let code = unsafe { if on { device.addToFavorites() } else { device.removeFromFavorites() } };
        if code == 0 { Ok(()) } else { Err(Error(format!("changing the favorite flag failed (IOReturn {code:#x})"))) }
    }

    fn remove(&self, _address: &str) -> Result<()> {
        Err(Error("removing a paired device is not available to apps on macOS: use System Settings, Bluetooth".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_map_to_frequencies() {
        assert_eq!((channel_mhz(1), channel_mhz(6), channel_mhz(14), channel_mhz(36), channel_mhz(149)), (2412, 2437, 2484, 5180, 5745));
    }

    #[test]
    fn addresses_compare_without_regard_to_case_or_separator() {
        assert_eq!(normalize("AA-BB-CC-00-11-22"), normalize("aa:bb:cc:00:11:22"));
    }

    #[test]
    fn device_kinds_come_from_the_class_of_device() {
        assert_eq!(kind_of(0x04, 0x06), Kind::Audio);
        assert_eq!(kind_of(0x05, 0x10), Kind::Keyboard);
        assert_eq!(kind_of(0x05, 0x20), Kind::Mouse);
        assert_eq!(kind_of(0x02, 0x03), Kind::Phone);
        assert_eq!(kind_of(0x01, 0x03), Kind::Computer);
        assert_eq!(kind_of(0x1f, 0), Kind::Other);
    }
}
