//! Full-screen image viewer and automated slideshow overlay.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use egui::{
    Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Key, Modifiers, Pos2, Rect, RichText,
    Sense, TextEdit, TextureHandle, Vec2, ViewportCommand,
};

use crate::api::Post;
use crate::settings::SLIDESHOW_SPEED_PRESETS;
use crate::ui::inspector::{
    fit_size, format_post_details, show_artist_actions_row, show_pool_actions_row, show_tag_list,
};
use crate::ui::state::{UiAction, ViewerState};
use crate::ui::theme;

#[allow(clippy::too_many_arguments)]
pub fn show_viewer(
    ctx: &egui::Context,
    viewer: &mut ViewerState,
    active_query: &str,
    preview_textures: &HashMap<u64, TextureHandle>,
    thumb_textures: &HashMap<u64, TextureHandle>,
    bank_has_post_fn: &dyn Fn(u64) -> bool,
    bank_has_pool_fn: &dyn Fn(u64) -> bool,
    bank_has_artist_fn: &dyn Fn(&str) -> bool,
    actions: &mut Vec<UiAction>,
) {
    let Some(post) = viewer.current_post().cloned() else {
        actions.push(UiAction::CloseViewer);
        return;
    };

    // Slideshow timer check
    if viewer.slideshow_playing {
        if let Some(next_tick) = viewer.next_tick {
            let now = Instant::now();
            if now >= next_tick && viewer.tick_slideshow() {
                if let Some(np) = viewer.current_post().cloned() {
                    actions.push(UiAction::ViewerPostChanged(np));
                }
            }
            if let Some(next) = viewer.next_tick {
                let rem = next.saturating_duration_since(Instant::now());
                ctx.request_repaint_after(rem.min(Duration::from_millis(250)));
            }
        }
    }

    // Keyboard shortcuts
    let filter_id = Id::new("viewer_tag_filter_edit");
    let filter_focused = ctx.memory(|m| m.has_focus(filter_id));
    handle_viewer_keys(ctx, viewer, &post, filter_focused, actions);

    // Reload current post in case keyboard shortcut changed it
    let Some(post) = viewer.current_post().cloned() else {
        return;
    };
    let in_bank = bank_has_post_fn(post.id);
    let pool_in_bank = post.pools.iter().any(|&pid| bank_has_pool_fn(pid));

    if !viewer.true_fullscreen {
        egui::TopBottomPanel::top("viewer_top_bar")
            .frame(
                egui::Frame::NONE
                    .fill(Color32::from_rgb(0x12, 0x14, 0x1a))
                    .inner_margin(egui::Margin::symmetric(10, 6)),
            )
            .show(ctx, |ui| {
                show_top_bar(ui, viewer, &post, in_bank, actions);
                ui.add_space(4.0);
                show_play_bar(ui, viewer, &post, pool_in_bank, actions);
            });

        if viewer.show_tags_panel {
            egui::SidePanel::right("viewer_tags_panel")
                .default_width(300.0)
                .min_width(240.0)
                .frame(
                    egui::Frame::NONE
                        .fill(Color32::from_rgb(0x16, 0x19, 0x20))
                        .inner_margin(egui::Margin::symmetric(8, 8)),
                )
                .show(ctx, |ui| {
                    show_sidebar(
                        ui,
                        viewer,
                        &post,
                        active_query,
                        in_bank,
                        pool_in_bank,
                        filter_id,
                        bank_has_artist_fn,
                        actions,
                    );
                });
        }
    }

    let bg_color = if viewer.true_fullscreen {
        Color32::BLACK
    } else {
        theme::BG_IMAGE
    };

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(bg_color))
        .show(ctx, |ui| {
            let avail = ui.available_size();
            let (rect, resp) = ui.allocate_exact_size(avail, Sense::click());

            if let Some(ref err) = viewer.loading_error {
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    format!("Could not load image:\n{err}"),
                    FontId::proportional(15.0),
                    theme::TEXT_ERROR,
                );
            } else if let Some(tex) = preview_textures
                .get(&post.id)
                .or_else(|| thumb_textures.get(&post.id))
            {
                let fitted = fit_size(tex.size_vec2(), rect.size());
                let img_rect = Rect::from_center_size(rect.center(), fitted);
                ui.painter().image(
                    tex.id(),
                    img_rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
                if post.is_non_image_media() {
                    let banner = format!(
                        "{} media file. Showing preview image. Use “Open on e621” to play video.",
                        post.ext.to_uppercase()
                    );
                    let banner_rect = Rect::from_center_size(
                        Pos2::new(rect.center().x, rect.bottom() - 28.0),
                        Vec2::new((rect.width() - 40.0).min(540.0), 28.0),
                    );
                    ui.painter().rect_filled(
                        banner_rect,
                        CornerRadius::same(6),
                        Color32::from_rgba_unmultiplied(0, 0, 0, 190),
                    );
                    ui.painter().text(
                        banner_rect.center(),
                        Align2::CENTER_CENTER,
                        banner,
                        FontId::proportional(12.5),
                        theme::ACCENT_AMBER,
                    );
                }
            } else {
                let msg = if post.is_non_image_media() {
                    format!(
                        "{} media file. Showing preview image.\nUse “Open on e621” to play video.",
                        post.ext.to_uppercase()
                    )
                } else {
                    "Loading high-res preview…".to_string()
                };
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    msg,
                    FontId::proportional(15.0),
                    theme::TEXT_MUTED,
                );
            }

            // Edge-zone pointing hand cursor & click navigation
            let w = rect.width().max(1.0);
            let edge = ViewerState::edge_zone_width(w);
            if resp.hovered() {
                if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let local_x = pos.x - rect.left();
                    if local_x <= edge || local_x >= w - edge {
                        ctx.set_cursor_icon(CursorIcon::PointingHand);
                    }
                }
            }

            if resp.clicked() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let local_x = pos.x - rect.left();
                    if viewer.click_edge_at(local_x, w) == Some(true) {
                        if let Some(np) = viewer.current_post().cloned() {
                            actions.push(UiAction::ViewerPostChanged(np));
                        }
                    }
                }
            }

            resp.context_menu(|ui| {
                if ui.button("Previous (Left)").clicked() {
                    if viewer.prev_post() {
                        if let Some(np) = viewer.current_post().cloned() {
                            actions.push(UiAction::ViewerPostChanged(np));
                        }
                    }
                    ui.close_menu();
                }
                if ui.button("Next (Right)").clicked() {
                    if viewer.next_post() {
                        if let Some(np) = viewer.current_post().cloned() {
                            actions.push(UiAction::ViewerPostChanged(np));
                        }
                    }
                    ui.close_menu();
                }
                ui.separator();
                let slide_txt = if viewer.slideshow_playing {
                    "Pause Slideshow (Space)"
                } else {
                    "Play Slideshow (Space)"
                };
                if ui.button(slide_txt).clicked() {
                    viewer.toggle_slideshow();
                    ui.close_menu();
                }
                ui.menu_button("Slideshow Speed", |ui| {
                    for (label, sec_val) in SLIDESHOW_SPEED_PRESETS {
                        let active = (viewer.slideshow_interval - *sec_val).abs() < 0.05;
                        if ui.selectable_label(active, *label).clicked() {
                            viewer.set_interval(*sec_val);
                            actions.push(UiAction::ViewerIntervalChanged(*sec_val));
                            ui.close_menu();
                        }
                    }
                });
                let bank_txt = if in_bank {
                    "Remove from Spank Bank (B)"
                } else {
                    "+ Save to Spank Bank (B)"
                };
                if ui.button(bank_txt).clicked() {
                    actions.push(UiAction::TogglePostInBank(post.clone()));
                    ui.close_menu();
                }
                let fav_txt = if post.is_favorited {
                    "Remove Favorite (L)"
                } else {
                    "Favorite on e621 (L)"
                };
                if ui.button(fav_txt).clicked() {
                    actions.push(UiAction::ToggleFavorite(post.clone()));
                    ui.close_menu();
                }
                let vote_txt = if post.own_vote == 1 {
                    "Remove Upvote (U)"
                } else {
                    "Upvote +1 on e621 (U)"
                };
                if ui.button(vote_txt).clicked() {
                    actions.push(UiAction::ToggleUpvote(post.clone()));
                    ui.close_menu();
                }
                if ui.button("Download Post").clicked() {
                    actions.push(UiAction::DownloadPosts(vec![post.clone()]));
                    ui.close_menu();
                }
                ui.separator();
                let ui_txt = if viewer.true_fullscreen {
                    "Show Controls & Tags (F11 / T)"
                } else {
                    "True Fullscreen (F11)"
                };
                if ui.button(ui_txt).clicked() {
                    viewer.true_fullscreen = !viewer.true_fullscreen;
                    ctx.send_viewport_cmd(ViewportCommand::Fullscreen(viewer.true_fullscreen));
                    ui.close_menu();
                }
                if ui.button("Close (Esc)").clicked() {
                    actions.push(UiAction::CloseViewer);
                    ui.close_menu();
                }
            });
        });
}

fn handle_viewer_keys(
    ctx: &egui::Context,
    viewer: &mut ViewerState,
    post: &Post,
    filter_focused: bool,
    actions: &mut Vec<UiAction>,
) {
    let mut go_prev = false;
    let mut go_next = false;
    let mut toggle_slide = false;
    let mut inc_speed = false;
    let mut dec_speed = false;
    let mut toggle_bank = false;
    let mut toggle_fav = false;
    let mut toggle_vote = false;
    let mut toggle_fs = false;
    let mut toggle_tags = false;
    let mut close = false;

    ctx.input_mut(|i| {
        if i.consume_key(Modifiers::NONE, Key::Escape) {
            close = true;
            return;
        }
        if i.consume_key(Modifiers::NONE, Key::F11) {
            toggle_fs = true;
            return;
        }
        if filter_focused {
            if viewer.tag_filter.is_empty() {
                if i.consume_key(Modifiers::NONE, Key::ArrowLeft) {
                    go_prev = true;
                } else if i.consume_key(Modifiers::NONE, Key::ArrowRight) {
                    go_next = true;
                }
            }
            return;
        }

        if i.consume_key(Modifiers::NONE, Key::ArrowLeft) || i.consume_key(Modifiers::NONE, Key::A)
        {
            go_prev = true;
        } else if i.consume_key(Modifiers::NONE, Key::ArrowRight)
            || i.consume_key(Modifiers::NONE, Key::D)
        {
            go_next = true;
        } else if i.consume_key(Modifiers::NONE, Key::Space)
            || i.consume_key(Modifiers::NONE, Key::S)
        {
            toggle_slide = true;
        } else if i.consume_key(Modifiers::NONE, Key::Plus)
            || i.consume_key(Modifiers::NONE, Key::Equals)
            || i.consume_key(Modifiers::SHIFT, Key::Equals)
            || i.consume_key(Modifiers::NONE, Key::CloseBracket)
        {
            inc_speed = true;
        } else if i.consume_key(Modifiers::NONE, Key::Minus)
            || i.consume_key(Modifiers::SHIFT, Key::Minus)
            || i.consume_key(Modifiers::NONE, Key::OpenBracket)
        {
            dec_speed = true;
        } else if i.consume_key(Modifiers::NONE, Key::B) {
            toggle_bank = true;
        } else if i.consume_key(Modifiers::NONE, Key::L) {
            toggle_fav = true;
        } else if i.consume_key(Modifiers::NONE, Key::U) {
            toggle_vote = true;
        } else if i.consume_key(Modifiers::NONE, Key::F) {
            toggle_fs = true;
        } else if i.consume_key(Modifiers::NONE, Key::T) {
            toggle_tags = true;
        } else if i.consume_key(Modifiers::NONE, Key::Q) {
            close = true;
        }
    });

    if close {
        actions.push(UiAction::CloseViewer);
        return;
    }
    if go_prev && viewer.prev_post() {
        if let Some(np) = viewer.current_post().cloned() {
            actions.push(UiAction::ViewerPostChanged(np));
        }
    }
    if go_next && viewer.next_post() {
        if let Some(np) = viewer.current_post().cloned() {
            actions.push(UiAction::ViewerPostChanged(np));
        }
    }
    if toggle_slide {
        viewer.toggle_slideshow();
    }
    if inc_speed {
        let v = viewer.increase_speed();
        actions.push(UiAction::ViewerIntervalChanged(v));
    }
    if dec_speed {
        let v = viewer.decrease_speed();
        actions.push(UiAction::ViewerIntervalChanged(v));
    }
    if toggle_bank {
        actions.push(UiAction::TogglePostInBank(post.clone()));
    }
    if toggle_fav {
        actions.push(UiAction::ToggleFavorite(post.clone()));
    }
    if toggle_vote {
        actions.push(UiAction::ToggleUpvote(post.clone()));
    }
    if toggle_fs {
        viewer.true_fullscreen = !viewer.true_fullscreen;
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(viewer.true_fullscreen));
    }
    if toggle_tags {
        if viewer.true_fullscreen {
            viewer.true_fullscreen = false;
            viewer.show_tags_panel = true;
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
        } else {
            viewer.show_tags_panel = !viewer.show_tags_panel;
        }
    }
}

fn show_top_bar(
    ui: &mut egui::Ui,
    viewer: &mut ViewerState,
    post: &Post,
    in_bank: bool,
    actions: &mut Vec<UiAction>,
) {
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(viewer.index > 0 || viewer.slideshow_loop, egui::Button::new("Previous (Left)"))
            .clicked()
            && viewer.prev_post()
        {
            if let Some(np) = viewer.current_post().cloned() {
                actions.push(UiAction::ViewerPostChanged(np));
            }
        }
        if ui
            .add_enabled(
                viewer.index + 1 < viewer.posts.len() || viewer.slideshow_loop,
                egui::Button::new("Next (Right)"),
            )
            .clicked()
            && viewer.next_post()
        {
            if let Some(np) = viewer.current_post().cloned() {
                actions.push(UiAction::ViewerPostChanged(np));
            }
        }
        ui.label(
            RichText::new(format!("{} / {}", viewer.index + 1, viewer.posts.len()))
                .strong()
                .size(13.0)
                .color(theme::ACCENT_BLUE),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Close (Esc)").clicked() {
                actions.push(UiAction::CloseViewer);
            }
            if ui
                .button(RichText::new("Download (D)").strong().color(theme::TEXT_SUCCESS))
                .clicked()
            {
                actions.push(UiAction::DownloadPosts(vec![post.clone()]));
            }
            let bank_btn = if in_bank {
                egui::Button::new(RichText::new("In Bank (B)").strong().color(Color32::WHITE))
                    .fill(theme::ACCENT_AMBER_BG)
            } else {
                egui::Button::new(RichText::new("+ Spank Bank (B)").strong())
            };
            if ui.add(bank_btn).clicked() {
                actions.push(UiAction::TogglePostInBank(post.clone()));
            }
            let vote_btn = if post.own_vote == 1 {
                egui::Button::new(
                    RichText::new(format!("▲ Voted ({}) (U)", post.score))
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(theme::ACCENT_VOTE_BG)
            } else {
                egui::Button::new(
                    RichText::new(format!("▲ +1 ({}) (U)", post.score))
                        .strong()
                        .color(Color32::from_rgb(0x86, 0xef, 0xac)),
                )
            };
            if ui.add(vote_btn).clicked() {
                actions.push(UiAction::ToggleUpvote(post.clone()));
            }
            let fav_btn = if post.is_favorited {
                egui::Button::new(
                    RichText::new(format!("Favorited ({}) (L)", post.fav_count))
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(theme::ACCENT_FAV_BG)
            } else {
                egui::Button::new(
                    RichText::new(format!("Favorite ({}) (L)", post.fav_count))
                        .strong()
                        .color(Color32::from_rgb(0xfd, 0xa4, 0xaf)),
                )
            };
            if ui.add(fav_btn).clicked() {
                actions.push(UiAction::ToggleFavorite(post.clone()));
            }
            if ui.button("Fullscreen (F11)").clicked() {
                viewer.true_fullscreen = true;
                ui.ctx().send_viewport_cmd(ViewportCommand::Fullscreen(true));
            }
            if ui
                .selectable_label(viewer.show_tags_panel, "Tags (T)")
                .clicked()
            {
                viewer.show_tags_panel = !viewer.show_tags_panel;
            }
        });
    });
}

fn show_play_bar(
    ui: &mut egui::Ui,
    viewer: &mut ViewerState,
    post: &Post,
    pool_in_bank: bool,
    actions: &mut Vec<UiAction>,
) {
    ui.horizontal_wrapped(|ui| {
        let slide_btn = if viewer.slideshow_playing {
            egui::Button::new(
                RichText::new("Pause Slideshow (Space)")
                    .strong()
                    .color(Color32::WHITE),
            )
            .fill(theme::ACCENT_RED_BTN)
        } else {
            egui::Button::new(
                RichText::new("Play Slideshow (Space)")
                    .strong()
                    .color(Color32::WHITE),
            )
            .fill(theme::ACCENT_BLUE_BTN)
        };
        if ui.add(slide_btn).clicked() {
            viewer.toggle_slideshow();
        }

        ui.label("Order:");
        let order_label = if viewer.slideshow_shuffle {
            "Shuffle"
        } else {
            "In Order"
        };
        egui::ComboBox::from_id_salt("viewer_order_combo")
            .selected_text(order_label)
            .width(90.0)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(!viewer.slideshow_shuffle, "In Order")
                    .clicked()
                {
                    viewer.slideshow_shuffle = false;
                    viewer.shuffle_queue.clear();
                }
                if ui
                    .selectable_label(viewer.slideshow_shuffle, "Shuffle")
                    .clicked()
                {
                    viewer.slideshow_shuffle = true;
                    viewer.shuffle_queue.clear();
                }
            });

        ui.label("Speed:");
        let preset_text = SLIDESHOW_SPEED_PRESETS
            .iter()
            .find(|(_, v)| (viewer.slideshow_interval - *v).abs() < 0.05)
            .map(|(l, _)| *l)
            .unwrap_or("Custom");
        egui::ComboBox::from_id_salt("viewer_speed_preset_combo")
            .selected_text(preset_text)
            .width(120.0)
            .show_ui(ui, |ui| {
                for (label, sec) in SLIDESHOW_SPEED_PRESETS {
                    let active = (viewer.slideshow_interval - *sec).abs() < 0.05;
                    if ui.selectable_label(active, *label).clicked() {
                        viewer.set_interval(*sec);
                        actions.push(UiAction::ViewerIntervalChanged(*sec));
                    }
                }
            });

        let mut sec_val = viewer.slideshow_interval;
        if ui
            .add(
                egui::DragValue::new(&mut sec_val)
                    .range(0.5..=300.0)
                    .speed(0.5)
                    .max_decimals(1)
                    .suffix(" s"),
            )
            .changed()
        {
            viewer.set_interval(sec_val);
            actions.push(UiAction::ViewerIntervalChanged(sec_val));
        }

        ui.checkbox(&mut viewer.slideshow_loop, "Loop")
            .on_hover_text("Loop back to the beginning when finished");

        if !viewer.slideshow_status.is_empty() {
            ui.label(
                RichText::new(&viewer.slideshow_status)
                    .small()
                    .color(Color32::from_rgb(0x38, 0xbd, 0xf8)),
            );
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Open on e621").clicked() {
                actions.push(UiAction::OpenUrl(post.page_url()));
            }
            if !post.pools.is_empty() {
                show_pool_actions_row(ui, &post.pools, pool_in_bank, "viewer_bar_pool", actions);
            }
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn show_sidebar(
    ui: &mut egui::Ui,
    viewer: &mut ViewerState,
    post: &Post,
    active_query: &str,
    in_bank: bool,
    pool_in_bank: bool,
    filter_id: Id,
    bank_has_artist_fn: &dyn Fn(&str) -> bool,
    actions: &mut Vec<UiAction>,
) {
    let artists = post.artists();
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("#{}", post.id)).strong());
        ui.label(RichText::new(format!("· {}", format_post_details(post))).small());
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Artist:");
        if artists.is_empty() {
            ui.label(RichText::new("unknown").strong().color(theme::ACCENT_ARTIST));
        } else {
            for (idx, a) in artists.iter().enumerate() {
                if idx > 0 {
                    ui.label(",");
                }
                if ui
                    .link(RichText::new(a).strong().color(theme::ACCENT_ARTIST))
                    .clicked()
                {
                    actions.push(UiAction::OpenArtist {
                        name: a.clone(),
                        new_tab: false,
                    });
                }
            }
        }
    });
    if !post.pools.is_empty() {
        let pool_txt = post
            .pools
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        ui.label(RichText::new(format!("Pools: {pool_txt}")).small());
    }

    ui.horizontal(|ui| {
        let fav_btn = if post.is_favorited {
            egui::Button::new(
                RichText::new(format!("Favorited ({})", post.fav_count))
                    .strong()
                    .color(Color32::WHITE),
            )
            .fill(theme::ACCENT_FAV_BG)
        } else {
            egui::Button::new(
                RichText::new(format!("Favorite ({})", post.fav_count))
                    .strong()
                    .color(Color32::from_rgb(0xfd, 0xa4, 0xaf)),
            )
        };
        if ui.add(fav_btn).clicked() {
            actions.push(UiAction::ToggleFavorite(post.clone()));
        }
        let vote_btn = if post.own_vote == 1 {
            egui::Button::new(
                RichText::new(format!("▲ Voted ({})", post.score))
                    .strong()
                    .color(Color32::WHITE),
            )
            .fill(theme::ACCENT_VOTE_BG)
        } else {
            egui::Button::new(
                RichText::new(format!("▲ +1 ({})", post.score))
                    .strong()
                    .color(Color32::from_rgb(0x86, 0xef, 0xac)),
            )
        };
        if ui.add(vote_btn).clicked() {
            actions.push(UiAction::ToggleUpvote(post.clone()));
        }
    });

    let side_bank_btn = if in_bank {
        egui::Button::new(RichText::new("In Spank Bank").strong().color(Color32::WHITE))
            .fill(theme::ACCENT_AMBER_BG)
    } else {
        egui::Button::new(RichText::new("+ Spank Bank").strong())
    };
    if ui
        .add_sized([ui.available_width(), 24.0], side_bank_btn)
        .clicked()
    {
        actions.push(UiAction::TogglePostInBank(post.clone()));
    }

    if !artists.is_empty() {
        show_artist_actions_row(ui, &artists, bank_has_artist_fn, "viewer_side_artist", actions);
    }
    if !post.pools.is_empty() {
        show_pool_actions_row(ui, &post.pools, pool_in_bank, "viewer_side_pool", actions);
    }

    ui.label("Tags (click to add/remove from search):");
    ui.horizontal(|ui| {
        ui.add(
            TextEdit::singleline(&mut viewer.tag_filter)
                .id(filter_id)
                .hint_text("Filter tags…")
                .desired_width(ui.available_width() - 28.0),
        );
        if !viewer.tag_filter.is_empty() && ui.small_button("×").clicked() {
            viewer.tag_filter.clear();
        }
    });

    egui::TopBottomPanel::bottom("viewer_side_tag_btns")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 4)))
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let sel = viewer.selected_tag.clone();
                if ui
                    .add_enabled(sel.is_some(), egui::Button::new("+ Add selected tag"))
                    .clicked()
                {
                    if let Some(t) = sel.clone() {
                        actions.push(UiAction::ToggleTagInSearch(t));
                    }
                }
                if ui
                    .add_enabled(sel.is_some(), egui::Button::new("Search only this tag"))
                    .clicked()
                {
                    if let Some(t) = sel {
                        actions.push(UiAction::SearchOnlyTag(t));
                    }
                }
            });
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show_inside(ui, |ui| {
            show_tag_list(
                ui,
                post,
                active_query,
                &viewer.tag_filter,
                bank_has_artist_fn,
                "viewer_side_tags",
                Some(&mut viewer.selected_tag),
                actions,
            );
        });
}
