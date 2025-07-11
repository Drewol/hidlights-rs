use std::{
    ffi::CString,
    ops::{Range, RangeInclusive},
    rc::Rc,
};

use bitvec::{order::Msb0, view::BitView};
use extfn::extfn;
use hidapi::{HidApi, HidDevice};
use hidparser::report_data_types::StringIndex;
use thiserror::Error;

type Result<T> = std::result::Result<T, HidLightError>;

#[derive(Debug, Error)]
pub enum HidLightError {
    #[error("HIDAPI Failure")]
    HidApi(#[from] hidapi::HidError),
    #[error("Descriptor parse error")]
    DescriptorError,
}

#[extfn]
fn indexed_name(
    self: Option<StringIndex>,
    dev: &HidDevice,
    usage: hidparser::report_data_types::Usage,
) -> Option<String> {
    self.and_then(|i| unsafe {
        // Unsafe to transmute string index as crate doesnt expose the inner value
        let i = std::mem::transmute::<_, u32>(i) as i32;
        dev.get_indexed_string(i).ok().flatten()
    })
    .or_else(|| {
        hut::Usage::new_from_page_and_id(usage.page(), usage.id())
            .map(|x| x.to_string())
            .ok()
    })
}

#[extfn]
fn usage_name(self: hidparser::report_data_types::Usage) -> Option<String> {
    hut::Usage::new_from_page_and_id(self.page(), self.id())
        .map(|x| x.name())
        .ok()
}

#[extfn]
fn parent_name(self: &Vec<hidparser::ReportCollection>, dev: &HidDevice) -> Option<String> {
    let rc = self.first()?;

    rc.string
        .map(|s| unsafe { std::mem::transmute::<_, u32>(s) })
        .and_then(|i| dev.get_indexed_string(i as _).ok())
        .flatten()
        .or_else(|| rc.usage.usage_name())
}

#[extfn]
fn is_vendor_usage(self: hidparser::report_data_types::Usage) -> bool {
    let Ok(usage) = hut::Usage::new_from_page_and_id(self.page(), self.id()) else {
        return true; // We don't know so be safe
    };
    match usage {
        hut::Usage::GenericDesktop(_) => false,
        hut::Usage::SimulationControls(_) => false,
        hut::Usage::VRControls(_) => false,
        hut::Usage::SportControls(_) => false,
        hut::Usage::GameControls(_) => false,
        hut::Usage::GenericDeviceControls(_) => false,
        hut::Usage::KeyboardKeypad(_) => false,
        hut::Usage::LED(_) => false,
        hut::Usage::Button(_) => false,
        hut::Usage::Ordinal(_) => false,
        hut::Usage::TelephonyDevice(_) => false,
        hut::Usage::Consumer(_) => false,
        hut::Usage::Digitizers(_) => false,
        hut::Usage::Haptics(_) => false,
        hut::Usage::PhysicalInputDevice(_) => false,
        hut::Usage::Unicode(_) => false,
        hut::Usage::SoC(_) => false,
        hut::Usage::EyeandHeadTrackers(_) => false,
        hut::Usage::AuxiliaryDisplay(_) => false,
        hut::Usage::Sensors(_) => false,
        hut::Usage::MedicalInstrument(_) => false,
        hut::Usage::BrailleDisplay(_) => false,
        hut::Usage::LightingAndIllumination(_) => false,
        hut::Usage::Monitor(_) => false,
        hut::Usage::MonitorEnumerated(_) => false,
        hut::Usage::VESAVirtualControls(_) => false,
        hut::Usage::Power(_) => false,
        hut::Usage::BatterySystem(_) => false,
        hut::Usage::BarcodeScanner(_) => false,
        hut::Usage::Scales(_) => false,
        hut::Usage::MagneticStripeReader(_) => false,
        hut::Usage::CameraControl(_) => false,
        hut::Usage::Arcade(_) => false,
        hut::Usage::FIDOAlliance(_) => false,
        hut::Usage::Wacom(_) => false,
        hut::Usage::ReservedUsagePage { .. } => true,
        hut::Usage::VendorDefinedPage { .. } => true,
        _ => true,
    }
}

pub struct HidLights {
    hidapi: Rc<hidapi::HidApi>,
}

pub struct DeviceInfo {
    pub vid: u16,
    pub pid: u16,
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    pub usage: Option<String>,
    pub serial: Option<String>,
    path: CString,
    api: Rc<HidApi>,
}

pub struct DeviceHandle {
    device: HidDevice,
}

impl HidLights {
    pub fn new() -> Result<Self> {
        Ok(Self {
            hidapi: Rc::new(hidapi::HidApi::new()?),
        })
    }

    pub fn devices(&self) -> Vec<DeviceInfo> {
        self.hidapi
            .device_list()
            .map(|x| DeviceInfo {
                name: x
                    .product_string()
                    .map(|x| x.to_string())
                    .filter(|x| !x.is_empty()),
                manufacturer: x
                    .manufacturer_string()
                    .filter(|x| !x.is_empty())
                    .map(|x| x.to_string()),
                usage: hut::Usage::new_from_page_and_id(x.usage_page(), x.usage())
                    .map(|x| x.name())
                    .ok(),
                serial: x
                    .serial_number()
                    .filter(|x| !x.is_empty())
                    .map(|x| x.to_string()),

                pid: x.product_id(),
                vid: x.vendor_id(),
                path: x.path().to_owned(),
                api: self.hidapi.clone(),
            })
            .collect()
    }
}

impl DeviceInfo {
    pub fn open(&self) -> Result<DeviceHandle> {
        let dev = self.api.open_path(&self.path)?;
        Ok(DeviceHandle { device: dev })
    }
}

#[derive(Debug)]
pub enum DeviceOutputValue {
    Toggle,
    Signed(RangeInclusive<i32>),
    Unsigned(RangeInclusive<i32>),
}

#[derive(Debug)]
pub struct DeviceOutput {
    kind: DeviceOutputValue,
    pub real_value: f32,
    bits: Range<u32>,
    pub name: Option<String>,
}

#[derive(Debug)]
pub struct Report {
    id: u32,
    pub outputs: Vec<DeviceOutput>,
    size_in_bits: usize,
}

impl DeviceHandle {
    pub fn reports(&self) -> Result<Vec<Report>> {
        {
            let dev = &self.device;
            let mut report_buffer = [0u8; 4096];

            let descriptor_len = dev.get_report_descriptor(&mut report_buffer)?;

            let descriptor = hidparser::parse_report_descriptor(&report_buffer[0..descriptor_len])
                .map_err(|_| HidLightError::DescriptorError)?;

            let mut result = vec![];

            for rep in descriptor.output_reports {
                let report_id: u32 = rep.report_id.map(|x| x.into()).unwrap_or_default();
                let mut report = Report {
                    id: report_id,
                    size_in_bits: rep.size_in_bits,
                    outputs: vec![],
                };

                for rep_field in rep.fields {
                    match rep_field {
                        hidparser::ReportField::Variable(variable_field) => {
                            if variable_field.usage.is_vendor_usage()
                                || !variable_field.attributes.variable
                            {
                                continue;
                            }

                            let name = variable_field
                                .string_index
                                .indexed_name(&dev, variable_field.usage)
                                .unwrap_or_else(|| "Unk".into());

                            report.outputs.push(DeviceOutput {
                                kind: if variable_field.bits.len() == 1 {
                                    DeviceOutputValue::Toggle
                                } else {
                                    DeviceOutputValue::Unsigned(
                                        variable_field.logical_minimum.into()
                                            ..=variable_field.logical_maximum.into(),
                                    )
                                },
                                real_value: 0.0,
                                bits: variable_field.bits,
                                name: Some(name),
                            });
                        }
                        hidparser::ReportField::Array(array_field) => {
                            let designators = array_field.designator_list.iter();
                            let usages = array_field.usage_list.iter();
                            let strings = array_field.string_list.iter();
                            let size = array_field.bits.end - array_field.bits.start;
                            let size = size / array_field.usage_list.len() as u32;
                            for (i, ((_designator, usage), string)) in
                                designators.zip(usages).zip(strings).enumerate()
                            {
                                let usage =
                                    hidparser::report_data_types::Usage::from(usage.start());
                                if usage.is_vendor_usage() {
                                    continue;
                                }
                                let mut name = string
                                    .range()
                                    .next()
                                    .map(|x| StringIndex::from(x))
                                    .indexed_name(&dev, usage)
                                    .unwrap_or_else(|| "Unk".into());
                                name.push(' ');
                                name.push(char::from_digit(i as _, 10).unwrap());
                                let start_bit = array_field.bits.start + i as u32 * size;
                                let bits = start_bit..(start_bit + size);
                                report.outputs.push(DeviceOutput {
                                    kind: if bits.len() == 1 {
                                        DeviceOutputValue::Toggle
                                    } else {
                                        DeviceOutputValue::Unsigned(
                                            array_field.logical_minimum.into()
                                                ..=array_field.logical_maximum.into(),
                                        )
                                    },
                                    real_value: 0.0,
                                    bits,
                                    name: Some(name),
                                });
                            }
                        }
                        hidparser::ReportField::Padding(_) => {}
                    }
                }

                if !report.outputs.is_empty() {
                    result.push(report);
                }
            }

            Ok(result)
        }
    }

    pub fn write_report(&self, report: &Report) -> Result<()> {
        let mut buffer = vec![0u8; report.size_in_bits.div_ceil(8)];
        buffer[0] = report.id as u8;
        let bits = buffer.view_bits_mut::<Msb0>();
        //TODO: Shouldn't have to set each bit individually, could set it using far fewer operations
        for out in &report.outputs {
            let real_value = out.real_value.clamp(0.0, 1.0);
            match &out.kind {
                DeviceOutputValue::Toggle => {
                    let enabled = real_value > f32::EPSILON;
                    for bit in out.bits.clone() {
                        bits.set(bit as _, enabled);
                    }
                }
                DeviceOutputValue::Signed(x) => {
                    // This doesn't actually work, need to consider compliment depending on bit count
                    let value = x.start() + ((x.end() - x.start()) as f32 * real_value) as i32;
                    let value = value as i32;

                    for (src_bit, dst_bit) in out.bits.clone().enumerate() {
                        bits.set(dst_bit as _, (value & (1 << src_bit)) != 0);
                    }
                }
                DeviceOutputValue::Unsigned(x) => {
                    let value = x.start() + ((x.end() - x.start()) as f32 * real_value) as i32;
                    let value = value as u32;
                    if value > 0 {
                        for (src_bit, dst_bit) in out.bits.clone().rev().enumerate() {
                            let set = (value & (1 << src_bit)) != 0;
                            bits.set(dst_bit as _, set);
                        }
                    }
                }
            }
        }

        self.device.write(&buffer)?;
        Ok(())
    }
}

impl DeviceOutput {
    pub fn is_toggle(&self) -> bool {
        matches!(self.kind, DeviceOutputValue::Toggle)
    }
}
