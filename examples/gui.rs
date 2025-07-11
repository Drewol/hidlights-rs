use eframe::{egui, App, NativeOptions};

struct HidLightGui {
    api: hidlights::HidLights,
    open_device: Option<hidlights::DeviceHandle>,
    reports: Option<Vec<hidlights::Report>>,
    devices: Vec<hidlights::DeviceInfo>,
}

impl App for HidLightGui {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ctx.available_rect().width());
                egui::Grid::new("devicelist").show(ui, |ui| {
                    if let (Some(dev), Some(reports)) =
                        (self.open_device.as_ref(), self.reports.as_mut())
                    {
                        for (i, rep) in reports.iter_mut().enumerate() {
                            let mut changed = false;
                            ui.collapsing(format!("Report {}", i), |ui| {
                                for out in rep.outputs.iter_mut() {
                                    if let Some(name) = out.name.as_ref() {
                                        ui.label(name);
                                    } else {
                                        ui.label("Unknown");
                                    }
                                    if out.is_toggle() {
                                        let mut checked = out.real_value > 0.0;
                                        changed |= ui.checkbox(&mut checked, ()).changed();
                                        out.real_value = checked.then_some(1.0).unwrap_or(0.0);
                                    } else {
                                        changed |= ui
                                            .add(
                                                egui::Slider::new(&mut out.real_value, 0.0..=1.0)
                                                    .clamping(egui::SliderClamping::Always),
                                            )
                                            .changed();
                                    }
                                    ui.end_row();
                                }
                            });
                            if changed {
                                println!("{:#?}", &rep);
                                dev.write_report(rep);
                            }
                            ui.end_row();
                        }

                        if ui.button("Close").clicked() {
                            self.open_device = None;
                            self.reports = None;
                        }
                    } else {
                        for dev in &self.devices {
                            let name = [
                                dev.name.as_ref(),
                                dev.manufacturer.as_ref(),
                                dev.usage.as_ref(),
                                dev.serial.as_ref(),
                            ]
                            .iter()
                            .filter(|x| x.is_some_and(|x| !x.is_empty()))
                            .map(|x| x.unwrap())
                            .map(|x| x.clone())
                            .collect::<Vec<_>>()
                            .join(", ");

                            let name = if name.is_empty() {
                                format!("VID/PID: {:04x}/{:04x}", dev.vid, dev.pid)
                            } else {
                                name
                            };

                            ui.label(name);

                            if ui.button("Select").clicked() {
                                if let Ok(dev) = dev.open() {
                                    if let Ok(reps) = dev.reports() {
                                        self.reports = Some(reps);
                                        self.open_device = Some(dev);
                                    }
                                }
                            }

                            ui.end_row();
                        }
                    }
                })
            })
        });
    }
}

fn main() {
    eframe::run_native(
        "HID Lights",
        NativeOptions::default(),
        Box::new(|_cc| {
            let api = hidlights::HidLights::new().unwrap();
            let devices = api.devices();

            Ok(Box::new(HidLightGui {
                api,
                devices,
                open_device: None,
                reports: None,
            }))
        }),
    )
    .unwrap();
}
