use eframe::{egui, App, NativeOptions};

struct HidLightGui {
    _api: hidlights::HidLights,
    open_device: Option<hidlights::DeviceHandle>,
    reports: Option<Vec<hidlights::Report>>,
    devices: Vec<hidlights::DeviceInfo>,
}

impl App for HidLightGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ctx.available_rect().width());
                egui::Grid::new("devicelist").show(ui, |ui| {
                    if let (Some(dev), Some(reports)) =
                        (self.open_device.as_ref(), self.reports.as_mut())
                    {
                        for rep in reports.iter_mut() {
                            let mut changed = false;
                            ui.collapsing(format!("Report {}", rep.id()), |ui| {
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
                                _ = dev.write_report(rep);
                            }
                            ui.end_row();
                        }

                        if ui.button("Close").clicked() {
                            self.open_device = None;
                            self.reports = None;
                        }
                    } else {
                        ui.label("Name");
                        ui.label("Mfg");
                        ui.label("Usage");
                        ui.label("VID/PID");
                        ui.end_row();
                        for dev in &self.devices {
                            for ele in [
                                dev.name.clone(),
                                dev.manufacturer.clone(),
                                dev.usage.as_ref().map(|x| x.name()),
                            ]
                            .into_iter()
                            {
                                ui.label(ele.unwrap_or_default());
                            }

                            let vidpid = { format!("{:04x}/{:04x}", dev.vid, dev.pid) };
                            ui.label(vidpid);

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
                _api: api,
                devices,
                open_device: None,
                reports: None,
            }))
        }),
    )
    .unwrap();
}
