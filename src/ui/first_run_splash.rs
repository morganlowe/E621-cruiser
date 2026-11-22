//! First-run onboarding splash screen for e621 username and API key setup.

use egui::{
    Align, Color32, CornerRadius, Key, Layout, Margin, Rect, RichText, ScrollArea, Stroke,
    TextEdit, TextureHandle,
};

use crate::ui::state::{FirstRunSplashState, UiAction};
use crate::ui::theme;

const MANAGE_API_URL: &str = "https://e621.net/users/home";
const MISSING_CREDS_MSG: &str = "Please enter both your e621 username and API key.";

pub fn show_first_run_splash(
    ctx: &egui::Context,
    splash: &mut FirstRunSplashState,
    logo_tex: Option<&TextureHandle>,
    actions: &mut Vec<UiAction>,
) -> Rect {
    let mut card_rect = Rect::NOTHING;

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::BG_WINDOW))
        .show(ctx, |ui| {
            ScrollArea::vertical()
                .id_salt("first_run_splash_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let avail_w = ui.available_width();
                    let avail_h = ui.available_height();
                    let card_w = (avail_w - 32.0).clamp(320.0, 680.0);
                    let side_pad = ((avail_w - card_w) * 0.5).max(16.0);
                    let top_pad = ((avail_h - 500.0) * 0.4).clamp(20.0, 220.0);

                    ui.add_space(top_pad);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.add_space(side_pad);
                        ui.vertical(|ui| {
                            ui.set_min_width(card_w);
                            ui.set_max_width(card_w);
                            let card_resp = egui::Frame::NONE
                                .fill(theme::BG_PANEL)
                                .stroke(Stroke::new(1.0_f32, theme::BORDER_SUBTLE))
                                .corner_radius(CornerRadius::same(10))
                                .inner_margin(Margin::same(26))
                                .show(ui, |ui| {
                                    let inner_w = (card_w - 52.0).max(260.0);
                                    ui.set_min_width(inner_w);
                                    ui.set_max_width(inner_w);

                                    render_header(ui, logo_tex, inner_w);
                                    ui.add_space(16.0);

                                    render_instructions(ui, inner_w, actions);
                                    ui.add_space(16.0);

                                    render_credentials_form(ui, splash, inner_w, actions);
                                    ui.add_space(10.0);

                                    render_test_row(ui, splash, actions);
                                    ui.add_space(14.0);

                                    render_footer(ui, splash, inner_w, actions);
                                });
                            card_rect = card_resp.response.rect;
                        });
                    });
                    ui.add_space(24.0);
                });
        });

    card_rect
}

fn render_header(ui: &mut egui::Ui, logo_tex: Option<&TextureHandle>, inner_w: f32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        let text_w = if let Some(tex) = logo_tex {
            ui.image((tex.id(), egui::vec2(52.0, 52.0)));
            (inner_w - 66.0).max(180.0)
        } else {
            inner_w
        };
        ui.vertical(|ui| {
            ui.set_max_width(text_w);
            ui.label(
                RichText::new("Welcome to E621 Cruiser")
                    .size(22.0)
                    .strong()
                    .color(Color32::WHITE),
            );
            ui.add_space(2.0);
            ui.label(
                RichText::new(
                    "Connect your e621 account to unlock full search results, favorites, and voting.",
                )
                .color(theme::TEXT_MUTED),
            );
        });
    });
}

fn render_instructions(ui: &mut egui::Ui, inner_w: f32, actions: &mut Vec<UiAction>) {
    egui::Frame::NONE
        .fill(theme::BG_SURFACE)
        .stroke(Stroke::new(1.0_f32, theme::BORDER_SUBTLE))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(14))
        .show(ui, |ui| {
            let box_w = (inner_w - 28.0).max(230.0);
            ui.set_min_width(box_w);
            ui.set_max_width(box_w);
            ui.label(
                RichText::new("How to get your e621 API key")
                    .size(14.5)
                    .strong()
                    .color(theme::ACCENT_BLUE),
            );
            ui.add_space(6.0);

            ui.label(
                "1. Sign in to your e621 account in a web browser (or create a free account at e621.net).",
            );
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label("2. Go to Account → Manage API Access:");
                if ui.button("Open e621 Account Page").clicked() {
                    actions.push(UiAction::OpenUrl(MANAGE_API_URL.to_string()));
                }
            });
            ui.label(
                "3. Re-enter your e621 password when prompted, then click View / Generate to reveal your API key.",
            );
            ui.label(
                "4. Copy your e621 username (not your email address) and API key into the fields below, then click Test Connection or Save & Continue.",
            );

            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "Note: e621 hides some posts from anonymous users, and favoriting or upvoting posts requires a logged-in account. Your credentials are saved locally in settings.json and sent only to e621.net over HTTPS.",
                )
                .small()
                .color(theme::TEXT_MUTED),
            );
        });
}

fn render_credentials_form(
    ui: &mut egui::Ui,
    splash: &mut FirstRunSplashState,
    inner_w: f32,
    actions: &mut Vec<UiAction>,
) {
    let field_w = (inner_w - 140.0).max(180.0);
    let mut edited = false;
    let mut submit_enter = false;
    let enter_pressed = ui.input(|i| i.key_pressed(Key::Enter));

    egui::Grid::new("first_run_creds_grid")
        .num_columns(2)
        .spacing([12.0, 10.0])
        .show(ui, |ui| {
            ui.label(RichText::new("e621 Username:").strong());
            let user_resp = ui.add(
                TextEdit::singleline(&mut splash.username)
                    .hint_text("Your e621 username (not email)")
                    .desired_width(field_w),
            );
            if user_resp.changed() {
                edited = true;
            }
            if user_resp.lost_focus() && enter_pressed {
                submit_enter = true;
            }
            ui.end_row();

            ui.label(RichText::new("API Key:").strong());
            ui.horizontal(|ui| {
                let key_resp = ui.add(
                    TextEdit::singleline(&mut splash.api_key)
                        .password(!splash.show_api_key)
                        .hint_text("Paste your e621 API key")
                        .desired_width((field_w - 64.0).max(120.0)),
                );
                if key_resp.changed() {
                    edited = true;
                }
                if key_resp.lost_focus() && enter_pressed {
                    submit_enter = true;
                }
                let toggle_lbl = if splash.show_api_key { "Hide" } else { "Show" };
                if ui.button(toggle_lbl).clicked() {
                    splash.show_api_key = !splash.show_api_key;
                }
            });
            ui.end_row();
        });

    if edited {
        splash.test_status = None;
    }
    if submit_enter && !splash.testing && splash.has_credentials() {
        actions.push(UiAction::SaveFirstRunSplash);
    }
}

fn render_test_row(
    ui: &mut egui::Ui,
    splash: &mut FirstRunSplashState,
    actions: &mut Vec<UiAction>,
) {
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(!splash.testing, egui::Button::new("Test Connection"))
            .clicked()
        {
            if !splash.has_credentials() {
                splash.test_status = Some((
                    false,
                    "Please enter both your e621 username and API key to test.".to_string(),
                ));
            } else {
                actions.push(UiAction::TestConnectionInSplash);
            }
        }

        if splash.testing {
            ui.label(
                RichText::new("Testing connection to e621…").color(theme::ACCENT_BLUE),
            );
        } else if let Some((ok, ref msg)) = splash.test_status {
            let (prefix, color) = if ok {
                ("[OK]", theme::TEXT_SUCCESS)
            } else {
                ("[Error]", theme::TEXT_ERROR)
            };
            ui.label(
                RichText::new(format!("{prefix} {msg}"))
                    .strong()
                    .color(color),
            );
        }
    });
}

fn render_footer(
    ui: &mut egui::Ui,
    splash: &mut FirstRunSplashState,
    inner_w: f32,
    actions: &mut Vec<UiAction>,
) {
    ui.separator();
    ui.add_space(8.0);
    if inner_w >= 420.0 {
        ui.horizontal(|ui| {
            render_secondary_footer_btn(ui, splash, actions);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                render_save_footer_btn(ui, splash, actions);
            });
        });
    } else {
        ui.horizontal_wrapped(|ui| {
            render_secondary_footer_btn(ui, splash, actions);
            render_save_footer_btn(ui, splash, actions);
        });
    }

    ui.add_space(6.0);
    ui.label(
        RichText::new("You can change or remove these credentials anytime in File → Settings (Ctrl+,).")
            .small()
            .color(theme::TEXT_MUTED),
    );
}

fn render_secondary_footer_btn(
    ui: &mut egui::Ui,
    splash: &FirstRunSplashState,
    actions: &mut Vec<UiAction>,
) {
    if splash.can_cancel {
        if ui.button("Cancel").clicked() {
            actions.push(UiAction::CancelFirstRunSplash);
        }
    } else if ui
        .button("Skip for now (Browse Anonymously)")
        .on_hover_text("You can add your API key anytime in File → Settings")
        .clicked()
    {
        actions.push(UiAction::SkipFirstRunSplash);
    }
}

fn render_save_footer_btn(
    ui: &mut egui::Ui,
    splash: &mut FirstRunSplashState,
    actions: &mut Vec<UiAction>,
) {
    let save_btn = egui::Button::new(
        RichText::new("Save & Continue")
            .strong()
            .color(Color32::WHITE),
    )
    .fill(theme::ACCENT_BLUE_BTN);
    if ui.add_enabled(!splash.testing, save_btn).clicked() {
        if !splash.has_credentials() {
            splash.test_status = Some((false, MISSING_CREDS_MSG.to_string()));
        } else {
            actions.push(UiAction::SaveFirstRunSplash);
        }
    }
}
