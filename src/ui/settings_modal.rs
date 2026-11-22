//! Settings modal dialog (credentials, blacklist, download & slideshow options).

use egui::{Color32, RichText, ScrollArea, TextEdit};

use crate::ui::state::{SettingsModalState, UiAction};
use crate::ui::theme;

pub fn show_settings_modal(
    ctx: &egui::Context,
    modal: &mut SettingsModalState,
    actions: &mut Vec<UiAction>,
) {
    let mut open = true;
    egui::Window::new("Settings")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(560.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Grid::new("settings_form_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label("e621 username:");
                    ui.add(TextEdit::singleline(&mut modal.username).desired_width(340.0));
                    ui.end_row();

                    ui.label("API key:");
                    ui.horizontal(|ui| {
                        ui.add(
                            TextEdit::singleline(&mut modal.api_key)
                                .password(!modal.show_api_key)
                                .desired_width(280.0),
                        );
                        let toggle_lbl = if modal.show_api_key { "Hide" } else { "Show" };
                        if ui.button(toggle_lbl).clicked() {
                            modal.show_api_key = !modal.show_api_key;
                        }
                    });
                    ui.end_row();

                    ui.label("");
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(!modal.testing, egui::Button::new("Test Connection"))
                            .clicked()
                        {
                            actions.push(UiAction::TestConnectionInModal);
                        }
                        if modal.testing {
                            ui.label(
                                RichText::new("Testing connection to e621…")
                                    .color(theme::ACCENT_BLUE),
                            );
                        } else if let Some((ok, ref msg)) = modal.test_status {
                            let color = if ok {
                                theme::TEXT_SUCCESS
                            } else {
                                Color32::from_rgb(0xf5, 0x65, 0x65)
                            };
                            let prefix = if ok { "[OK]" } else { "[Failed]" };
                            ui.label(RichText::new(format!("{prefix} {msg}")).strong().color(color));
                        }
                    });
                    ui.end_row();

                    ui.label("");
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label("Create an API key at e621.net → Account →");
                        ui.hyperlink_to("Manage API Access", "https://e621.net/users/home");
                        ui.label(
                            "Logging in unlocks posts that e621 hides from anonymous users and lets you load private favorites.",
                        );
                    });
                    ui.end_row();

                    ui.label("Blacklist:");
                    ScrollArea::vertical()
                        .id_salt("settings_blacklist_scroll")
                        .max_height(130.0)
                        .show(ui, |ui| {
                            ui.add(
                                TextEdit::multiline(&mut modal.blacklist)
                                    .hint_text("one rule per line, e.g.\ngore\nyoung -rating:s")
                                    .desired_rows(6)
                                    .desired_width(360.0),
                            );
                        });
                    ui.end_row();

                    ui.label("Parallel downloads:");
                    ui.add(
                        egui::DragValue::new(&mut modal.concurrent_downloads)
                            .range(1..=8)
                            .speed(0.1),
                    );
                    ui.end_row();

                    ui.label("Slideshow speed/interval:");
                    ui.add(
                        egui::DragValue::new(&mut modal.slideshow_interval)
                            .range(0.5..=300.0)
                            .speed(0.5)
                            .max_decimals(1)
                            .suffix(" s"),
                    );
                    ui.end_row();

                    ui.label("");
                    ui.checkbox(
                        &mut modal.skip_downloaded,
                        "Skip posts already downloaded on disk",
                    );
                    ui.end_row();

                    ui.label("");
                    ui.checkbox(
                        &mut modal.write_tags_file,
                        "Save a .txt file with each post's tags next to it",
                    );
                    ui.end_row();

                    ui.label("");
                    let clr_txt = if modal.clear_history_requested {
                        "History will be cleared on Save".to_string()
                    } else {
                        format!(
                            "Clear download history ({} posts)",
                            crate::tags::format_post_count(modal.history_count as u64)
                        )
                    };
                    if ui
                        .add_enabled(!modal.clear_history_requested, egui::Button::new(clr_txt))
                        .clicked()
                    {
                        modal.clear_history_requested = true;
                    }
                    ui.end_row();
                });

            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Save").clicked() {
                    actions.push(UiAction::SaveSettingsModal);
                }
                if ui.button("Cancel").clicked() {
                    actions.push(UiAction::CancelSettingsModal);
                }
            });
        });

    if !open {
        actions.push(UiAction::CancelSettingsModal);
    }
}
