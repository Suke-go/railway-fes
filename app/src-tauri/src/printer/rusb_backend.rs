//! libusb / rusb backend for EPSON TM-T90II on macOS (and Linux).
//!
//! EPSON's USB vendor ID is 0x04B8. The TM-T90II's product ID needs to be
//! confirmed against the actual device once attached — placeholder below.
//! Run `system_profiler SPUSBDataType` on macOS to find it.

#![cfg(not(target_os = "windows"))]

use anyhow::{anyhow, Context, Result};
use rusb::{Device, DeviceHandle, GlobalContext};

use super::{escpos, PrinterBackend};

const EPSON_VID: u16 = 0x04B8;
/// TODO: confirm PID against the physical TM-T90II. Common Epson POS PIDs are
/// in the 0x0200 range. The driver should iterate and pick the first matching
/// EPSON device if this exact PID isn't found.
const TM_T90II_PID_GUESS: u16 = 0x0202;

pub struct RusbBackend {
    handle: DeviceHandle<GlobalContext>,
    bulk_out_endpoint: u8,
}

impl RusbBackend {
    pub fn open_default() -> Result<Self> {
        let device = find_printer()?;
        let mut handle = device.open().context("opening USB device")?;

        // Some macOS configs claim the device via a kernel driver — detach if
        // necessary so we can do raw bulk transfers.
        if handle.kernel_driver_active(0).unwrap_or(false) {
            handle.detach_kernel_driver(0).ok();
        }
        handle.claim_interface(0).context("claiming interface 0")?;

        let bulk_out_endpoint = find_bulk_out_endpoint(&device)?;

        let mut backend = Self { handle, bulk_out_endpoint };
        backend.write_raw(&escpos::session_open())?;
        Ok(backend)
    }

    fn write_raw(&mut self, bytes: &[u8]) -> Result<()> {
        let timeout = std::time::Duration::from_millis(500);
        let written = self
            .handle
            .write_bulk(self.bulk_out_endpoint, bytes, timeout)
            .context("bulk write to printer")?;
        if written != bytes.len() {
            return Err(anyhow!(
                "short write: {written} of {} bytes",
                bytes.len()
            ));
        }
        Ok(())
    }
}

impl PrinterBackend for RusbBackend {
    fn print_phrase(&mut self, text: &str) -> Result<()> {
        let mut bytes = escpos::encode_sjis(text);
        bytes.push(b'\n');
        self.write_raw(&bytes)
    }

    fn feed(&mut self, n: u8) -> Result<()> {
        self.write_raw(&escpos::feed_lines(n))
    }

    fn cut(&mut self) -> Result<()> {
        // Feed a few lines first so the cut doesn't slice the last text line.
        self.write_raw(&escpos::feed_lines(4))?;
        self.write_raw(escpos::CUT_FULL)
    }
}

fn find_printer() -> Result<Device<GlobalContext>> {
    // Exact match first.
    if let Some(d) = enumerate_with(|desc| {
        desc.vendor_id() == EPSON_VID && desc.product_id() == TM_T90II_PID_GUESS
    })? {
        return Ok(d);
    }
    // Fall back to any EPSON device (lets the operator plug in any TM-series
    // printer; the first one wins). Log the PID so we can hard-code it later.
    enumerate_with(|desc| desc.vendor_id() == EPSON_VID)?
        .ok_or_else(|| anyhow!("no EPSON USB device found (VID {:#06x})", EPSON_VID))
}

fn enumerate_with(
    mut pred: impl FnMut(&rusb::DeviceDescriptor) -> bool,
) -> Result<Option<Device<GlobalContext>>> {
    for device in rusb::devices().context("listing USB devices")?.iter() {
        let desc = device.device_descriptor().context("reading descriptor")?;
        if pred(&desc) {
            tracing::info!(
                vid = format!("{:#06x}", desc.vendor_id()),
                pid = format!("{:#06x}", desc.product_id()),
                "selected printer USB device",
            );
            return Ok(Some(device));
        }
    }
    Ok(None)
}

fn find_bulk_out_endpoint(device: &Device<GlobalContext>) -> Result<u8> {
    let config = device.active_config_descriptor().context("active config")?;
    for iface in config.interfaces() {
        for desc in iface.descriptors() {
            for ep in desc.endpoint_descriptors() {
                if ep.direction() == rusb::Direction::Out
                    && ep.transfer_type() == rusb::TransferType::Bulk
                {
                    return Ok(ep.address());
                }
            }
        }
    }
    Err(anyhow!("no bulk OUT endpoint found on printer"))
}
