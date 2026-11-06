//! The BlueZ backend: Bluetooth over its D-Bus API (`org.bluez`). Adapters and devices are
//! objects under `/org/bluez`, listed with the ObjectManager at `/`.

use crate::Result;
use crate::bluetooth::{Bluetooth, BtStatus, Device, Kind, sort};
use crate::Error;
use std::collections::HashMap;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

const DEST: &str = "org.bluez";
const IF_ADAPTER: &str = "org.bluez.Adapter1";
const IF_DEVICE: &str = "org.bluez.Device1";
const IF_MANAGER: &str = "org.freedesktop.DBus.ObjectManager";

type Props = HashMap<String, OwnedValue>;
type Managed = HashMap<OwnedObjectPath, HashMap<String, Props>>;

pub struct BlueZ {
    conn: Connection,
    dest: String,
}

fn get<T: TryFrom<OwnedValue>>(props: &Props, key: &str) -> Option<T> {
    props.get(key).and_then(|v| v.try_clone().ok()).and_then(|v| T::try_from(v).ok())
}

/// The device behind an object's `Device1` properties.
pub fn device_of(props: &Props) -> Option<Device> {
    let address: String = get(props, "Address")?;
    let alias: Option<String> = get(props, "Alias");
    let name: Option<String> = get(props, "Name");
    let name = alias.filter(|a| !a.is_empty()).or(name).unwrap_or_else(|| address.clone());
    Some(Device {
        kind: Kind::from_icon(&get::<String>(props, "Icon").unwrap_or_default()),
        paired: get(props, "Paired").unwrap_or(false),
        trusted: get(props, "Trusted").unwrap_or(false),
        connected: get(props, "Connected").unwrap_or(false),
        rssi: get(props, "RSSI"),
        name,
        address,
    })
}

impl BlueZ {
    /// Connect to the system bus, where BlueZ lives.
    pub fn system() -> Result<BlueZ> {
        let conn = Connection::system().map_err(|e| Error(format!("cannot reach the system D-Bus: {e}")))?;
        Ok(BlueZ { conn, dest: DEST.to_string() })
    }

    /// Use another bus and bus name (tests run a pretend BlueZ on a private bus).
    pub fn with_connection(conn: Connection, dest: &str) -> BlueZ {
        BlueZ { conn, dest: dest.to_string() }
    }

    fn proxy(&self, path: &str, interface: &str) -> Result<Proxy<'static>> {
        Ok(Proxy::new(&self.conn, self.dest.clone(), path.to_string(), interface.to_string())?)
    }

    fn managed(&self) -> Result<Managed> {
        Ok(self.proxy("/", IF_MANAGER)?.call("GetManagedObjects", &())?)
    }

    /// The first adapter: its path, interface name and properties.
    fn adapter(&self) -> Result<Option<(OwnedObjectPath, Props)>> {
        let mut found: Vec<(OwnedObjectPath, Props)> = self.managed()?.into_iter().filter_map(|(p, mut ifaces)| ifaces.remove(IF_ADAPTER).map(|props| (p, props))).collect();
        found.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
        Ok(found.into_iter().next())
    }

    fn need_adapter(&self) -> Result<OwnedObjectPath> {
        self.adapter()?.map(|(p, _)| p).ok_or_else(|| Error("no Bluetooth adapter".into()))
    }

    /// The object path of the device with this address.
    fn device_path(&self, address: &str) -> Result<(OwnedObjectPath, Device)> {
        self.managed()?
            .into_iter()
            .filter_map(|(p, mut ifaces)| ifaces.remove(IF_DEVICE).and_then(|props| device_of(&props)).map(|d| (p, d)))
            .find(|(_, d)| d.address.eq_ignore_ascii_case(address))
            .ok_or_else(|| Error(format!("{address} is not known")))
    }
}

impl Bluetooth for BlueZ {
    fn status(&self) -> Result<BtStatus> {
        let Some((path, props)) = self.adapter()? else { return Ok(BtStatus::default()) };
        let name = path.as_str().rsplit('/').next().unwrap_or("").to_string();
        Ok(BtStatus { adapter: Some(name), powered: get(&props, "Powered").unwrap_or(false), discovering: get(&props, "Discovering").unwrap_or(false) })
    }

    fn devices(&self) -> Result<Vec<Device>> {
        let mut out: Vec<Device> = self.managed()?.into_values().filter_map(|mut ifaces| ifaces.remove(IF_DEVICE)).filter_map(|props| device_of(&props)).collect();
        // Devices BlueZ remembers only because they were seen once and are neither paired nor in range are noise.
        out.retain(|d| d.paired || d.connected || d.rssi.is_some());
        sort(&mut out);
        Ok(out)
    }

    fn scan(&self, on: bool) -> Result<()> {
        let adapter = self.need_adapter()?;
        let proxy = self.proxy(adapter.as_str(), IF_ADAPTER)?;
        proxy.call::<_, _, ()>(if on { "StartDiscovery" } else { "StopDiscovery" }, &())?;
        Ok(())
    }

    fn set_powered(&self, on: bool) -> Result<()> {
        let adapter = self.need_adapter()?;
        self.proxy(adapter.as_str(), IF_ADAPTER)?.set_property("Powered", on)?;
        Ok(())
    }

    fn connect(&self, address: &str) -> Result<()> {
        let (path, device) = self.device_path(address)?;
        let proxy = self.proxy(path.as_str(), IF_DEVICE)?;
        if !device.paired {
            proxy.call::<_, _, ()>("Pair", &())?;
        }
        if !device.trusted {
            proxy.set_property("Trusted", true)?;
        }
        proxy.call::<_, _, ()>("Connect", &())?;
        Ok(())
    }

    fn disconnect(&self, address: &str) -> Result<()> {
        let (path, _) = self.device_path(address)?;
        self.proxy(path.as_str(), IF_DEVICE)?.call::<_, _, ()>("Disconnect", &())?;
        Ok(())
    }

    fn set_trusted(&self, address: &str, on: bool) -> Result<()> {
        let (path, _) = self.device_path(address)?;
        self.proxy(path.as_str(), IF_DEVICE)?.set_property("Trusted", on)?;
        Ok(())
    }

    fn remove(&self, address: &str) -> Result<()> {
        let (path, _) = self.device_path(address)?;
        let adapter = self.need_adapter()?;
        self.proxy(adapter.as_str(), IF_ADAPTER)?.call::<_, _, ()>("RemoveDevice", &(path,))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
