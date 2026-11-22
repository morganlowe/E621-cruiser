//! Bottom download controls, progress bar, and activity log panel.

use egui::{Align, Layout, ProgressBar, RichText, ScrollArea, TextEdit, Ui};

use crate::settings::Settings;
use crate::storage::{organize_mode_label, ORGANIZE_MODES};
use crate::ui::state::UiAction;
use crate::ui::theme;

#[allow(clippy::too_many_arguments)]
pub fn show_download_panel(
    ui: &mut Ui,
    settings: &mut Settings,
    selected_count: usize,
    dl_done: usize,
    dl_total: usize,
    logs: &[String],
    actions: &mut Vec<UiAction>,
) {
    egui::Frame::group(ui.style())
        .fill(theme::BG_PANEL)
        .show(ui, |ui| {
            ui.label(RichText::new("Download").strong());

            // Row 1: Folder path + Browse / Open
            ui.horizontal(|ui| {
                ui.label("Folder:");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Open").clicked() {
                        actions.push(UiAction::OpenDownloadDir);
                    }
                    if ui.button("Browse…").clicked() {
                        actions.push(UiAction::BrowseDownloadDir);
                    }
                    let rem = ui.available_width().max(100.0);
                    if ui
                        .add(TextEdit::singleline(&mut settings.download_dir).desired_width(rem))
                        .changed()
                    {
                        actions.push(UiAction::SaveFiltersToSettings);
                    }
                });
            });

            // Row 2: Organize mode + Save tags .txt
            ui.horizontal_wrapped(|ui| {
                ui.label("Organize:");
                let org_label = organize_mode_label(&settings.organize_mode).to_string();
                egui::ComboBox::from_id_salt("organize_mode_combo")
                    .selected_text(org_label)
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        for (key, label) in ORGANIZE_MODES {
                            if ui
                                .selectable_label(settings.organize_mode == *key, *label)
                                .clicked()
                            {
                                settings.organize_mode = (*key).to_string();
                                actions.push(UiAction::OrganizeModeChanged);
                            }
                        }
                    });
                if ui
                    .checkbox(&mut settings.write_tags_file, "Save tags .txt")
                    .changed()
                {
                    actions.push(UiAction::SaveTagsFileToggled);
                }
            });

            // Row 3: Selection & Download actions
            if ui.available_width() > 600.0 {
                ui.horizontal(|ui| {
                    if ui.button("Select all").clicked() {
                        actions.push(UiAction::SelectAllPosts);
                    }
                    if ui.button("Select none").clicked() {
                        actions.push(UiAction::SelectNoPosts);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Cancel downloads").clicked() {
                            actions.push(UiAction::CancelDownloads);
                        }
                        if ui
                            .button(RichText::new("Download ALL results").strong())
                            .clicked()
                        {
                            actions.push(UiAction::DownloadAllResults);
                        }
                        if ui
                            .button(
                                RichText::new(format!("Download selected ({selected_count})"))
                                    .strong(),
                            )
                            .on_hover_text("Download selected posts (Ctrl+D)")
                            .clicked()
                        {
                            actions.push(UiAction::DownloadSelectedPosts);
                        }
                    });
                });
            } else {
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Select all").clicked() {
                        actions.push(UiAction::SelectAllPosts);
                    }
                    if ui.button("Select none").clicked() {
                        actions.push(UiAction::SelectNoPosts);
                    }
                    if ui
                        .button(
                            RichText::new(format!("Download selected ({selected_count})")).strong(),
                        )
                        .on_hover_text("Download selected posts (Ctrl+D)")
                        .clicked()
                    {
                        actions.push(UiAction::DownloadSelectedPosts);
                    }
                    if ui
                        .button(RichText::new("Download ALL results").strong())
                        .clicked()
                    {
                        actions.push(UiAction::DownloadAllResults);
                    }
                    if ui.button("Cancel downloads").clicked() {
                        actions.push(UiAction::CancelDownloads);
                    }
                });
            }

            // Row 4: Progress bar
            let frac = if dl_total > 0 {
                (dl_done as f32 / dl_total as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let text = if dl_total > 0 {
                format!("{dl_done} / {dl_total} downloaded")
            } else {
                "Idle".to_string()
            };
            ui.add(ProgressBar::new(frac).text(text).desired_width(ui.available_width()));

            // Row 5: Log area
            egui::Frame::group(ui.style())
                .fill(theme::BG_IMAGE)
                .show(ui, |ui| {
                    let log_w = ui.available_width().max(100.0);
                    ui.allocate_ui(egui::vec2(log_w, 84.0), |ui| {
                        ScrollArea::vertical()
                            .id_salt("download_log_scroll")
                            .max_height(84.0)
                            .min_scrolled_height(84.0)
                            .stick_to_bottom(true)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for line in logs {
                                    ui.label(RichText::new(line).small().monospace());
                                }
                            });
                    });
                });
        });
}
