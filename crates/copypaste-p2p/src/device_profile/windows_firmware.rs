//! Windows chassis identity from SMBIOS, without a shell or WMI service.

use copypaste_ipc::DeviceClass;

#[cfg(target_os = "windows")]
pub(super) fn device_class() -> DeviceClass {
    use windows_sys::Win32::System::SystemInformation::GetSystemFirmwareTable;

    // Bound the temporary firmware allocation. Only the derived class is cached
    // by the caller; neither raw firmware nor its identifying strings survive.
    const MAX_FIRMWARE_BYTES: u32 = 1024 * 1024;
    const RSMB: u32 = u32::from_be_bytes(*b"RSMB");
    // SAFETY: Windows documents a null buffer with size zero as a size query.
    let size = unsafe { GetSystemFirmwareTable(RSMB, 0, std::ptr::null_mut(), 0) };
    if !(8..=MAX_FIRMWARE_BYTES).contains(&size) {
        return DeviceClass::Unknown;
    }
    let mut buffer = vec![0u8; size as usize];
    // SAFETY: The initialized buffer is writable for exactly `size` bytes.
    // A changed size is rejected below, never used to extend the allocation.
    let written = unsafe { GetSystemFirmwareTable(RSMB, 0, buffer.as_mut_ptr().cast(), size) };
    if written != size {
        return DeviceClass::Unknown;
    }
    class_from_raw_smbios(&buffer)
}

/// RawSMBIOSData has an eight-byte Windows header, then length-delimited
/// structures followed by double-NUL-terminated string sets (DSP0134, 6.1).
fn class_from_raw_smbios(raw: &[u8]) -> DeviceClass {
    let Some(length) = raw.get(4..8) else {
        return DeviceClass::Unknown;
    };
    let length = u32::from_le_bytes(length.try_into().expect("four-byte length")) as usize;
    let Some(mut table) = raw.get(8..).and_then(|table| table.get(..length)) else {
        return DeviceClass::Unknown;
    };
    while table.len() >= 4 {
        let kind = table[0];
        let length = usize::from(table[1]);
        if kind == 127 || length < 4 || length > table.len() {
            break;
        }
        let Some(strings_end) = table[length..].windows(2).position(|pair| pair == [0, 0]) else {
            break;
        };
        if kind == 3 && length >= 9 {
            // Bit 7 means chassis lock present, not part of the enclosure type.
            let class = match table[5] & 0x7f {
                3..=7 | 13 | 15..=17 | 23 | 24 | 35 | 36 => DeviceClass::Desktop,
                8..=10 | 14 | 31 | 32 => DeviceClass::Laptop,
                30 => DeviceClass::Tablet,
                _ => DeviceClass::Unknown,
            };
            if class != DeviceClass::Unknown {
                return class;
            }
        }
        table = &table[length + strings_end + 2..];
    }
    DeviceClass::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_table(table: &[u8]) -> Vec<u8> {
        let mut raw = vec![0, 3, 9, 0];
        raw.extend_from_slice(&(table.len() as u32).to_le_bytes());
        raw.extend_from_slice(table);
        raw
    }

    fn enclosure(kind: u8) -> Vec<u8> {
        vec![3, 9, 0, 0, 0, kind, 0, 0, 0, 0, 0]
    }

    #[test]
    fn reads_windows_chassis_without_a_powershell_probe() {
        assert_eq!(
            class_from_raw_smbios(&raw_table(&enclosure(3))),
            DeviceClass::Desktop
        );
        assert_eq!(
            class_from_raw_smbios(&raw_table(&enclosure(10 | 0x80))),
            DeviceClass::Laptop
        );
        assert_eq!(
            class_from_raw_smbios(&raw_table(&enclosure(30))),
            DeviceClass::Tablet
        );
    }

    #[test]
    fn distinguishes_all_in_one_and_convertible_hardware() {
        assert_eq!(
            class_from_raw_smbios(&raw_table(&enclosure(13))),
            DeviceClass::Desktop
        );
        assert_eq!(
            class_from_raw_smbios(&raw_table(&enclosure(31))),
            DeviceClass::Laptop
        );
        assert_eq!(
            class_from_raw_smbios(&raw_table(&enclosure(35))),
            DeviceClass::Desktop
        );
    }

    #[test]
    fn skips_unrelated_structures_and_their_strings() {
        let mut table = vec![1, 4, 0, 0, b'M', b'o', b'd', b'e', b'l', 0, 0];
        table.extend_from_slice(&enclosure(2));
        table.extend_from_slice(&enclosure(9));
        assert_eq!(
            class_from_raw_smbios(&raw_table(&table)),
            DeviceClass::Laptop
        );
    }

    #[test]
    fn missing_or_malformed_firmware_does_not_guess_a_class() {
        assert_eq!(class_from_raw_smbios(&[]), DeviceClass::Unknown);
        let mut truncated = raw_table(&enclosure(3));
        truncated.pop();
        assert_eq!(class_from_raw_smbios(&truncated), DeviceClass::Unknown);
        assert_eq!(
            class_from_raw_smbios(&raw_table(&[3, 2, 0, 0, 0, 3, 0, 0])),
            DeviceClass::Unknown
        );
        assert_eq!(
            class_from_raw_smbios(&raw_table(&enclosure(1))),
            DeviceClass::Unknown
        );
        assert_eq!(
            class_from_raw_smbios(&raw_table(&enclosure(12))),
            DeviceClass::Unknown
        );
        assert_eq!(
            class_from_raw_smbios(&raw_table(&[127, 4, 0, 0, 0, 0])),
            DeviceClass::Unknown
        );
    }
}
