//! Result tabs, navigation bar, virtualized thumbnail grid, context menu, and hover preview popup.

use std::collections::HashMap;
use std::path::PathBuf;

use egui::{
    Align2, Color32, CornerRadius, FontId, Id, Order, Pos2, Rect, RichText, ScrollArea, Sense,
    Stroke, TextureHandle, Ui, Vec2,
};

use crate::api::{artist_page_url, Post};
use crate::tags::rating_color;
use crate::ui::inspector::{fit_size, format_post_details};
use crate::ui::state::{ResultTab, SourceKind, UiAction};
use crate::ui::theme;

pub const TILE: f32 = 150.0;
const CELL_W: f32 = TILE + 18.0;
const CELL_H: f32 = TILE + 34.0;

pub fn show_results_tab_bar(
    ui: &mut Ui,
    tabs: &[ResultTab],
    active_tab_idx: usize,
    actions: &mut Vec<UiAction>,
) {
    ui.horizontal(|ui| {
        ScrollArea::horizontal()
            .id_salt("result_tabs_scroll")
            .max_width((ui.available_width() - 36.0).max(100.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (idx, tab) in tabs.iter().enumerate() {
                        let is_active = idx == active_tab_idx;
                        let frame_fill = if is_active {
                            Color32::from_rgb(0x28, 0x33, 0x45)
                        } else {
                            theme::BG_SURFACE
                        };
                        let stroke = if is_active {
                            Stroke::new(1.0_f32, theme::ACCENT_BLUE)
                        } else {
                            Stroke::new(1.0_f32, theme::BORDER_SUBTLE)
                        };
                        egui::Frame::NONE
                            .fill(frame_fill)
                            .stroke(stroke)
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(egui::Margin::symmetric(8, 3))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    let text = RichText::new(tab.title()).color(if is_active {
                                        Color32::WHITE
                                    } else {
                                        theme::TEXT_PRIMARY
                                    });
                                    if ui
                                        .selectable_label(false, text)
                                        .on_hover_text(tab.tooltip())
                                        .clicked()
                                    {
                                        actions.push(UiAction::SwitchTab(idx));
                                    }
                                    if ui
                                        .small_button("×")
                                        .on_hover_text("Close tab (Ctrl+W)")
                                        .clicked()
                                    {
                                        actions.push(UiAction::CloseTab(idx));
                                    }
                                });
                            });
                    }
                });
            });

        if ui
            .button(RichText::new("+").strong().size(15.0))
            .on_hover_text("Open new tab (Ctrl+T)")
            .clicked()
        {
            actions.push(UiAction::NewTab);
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub fn show_tab_contents(
    ui: &mut Ui,
    tab: &mut ResultTab,
    pending_scroll_restore: &mut Option<f32>,
    source_in_bank: bool,
    thumb_textures: &HashMap<u64, TextureHandle>,
    preview_textures: &HashMap<u64, TextureHandle>,
    bank_has_post_fn: &dyn Fn(u64) -> bool,
    bank_has_pool_fn: &dyn Fn(u64) -> bool,
    bank_has_artist_fn: &dyn Fn(&str) -> bool,
    history_path_fn: &dyn Fn(u64) -> Option<String>,
    suppress_hover: bool,
    actions: &mut Vec<UiAction>,
) {
    // Nav bar inside top sub-panel so grid occupies the full remaining viewport
    egui::TopBottomPanel::top(Id::new(("tab_nav_bar_panel", tab.id)))
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(2, 4)))
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!tab.history_back.is_empty(), egui::Button::new("< Back"))
                    .on_hover_text("Go back to previous view (Alt+Left)")
                    .clicked()
                {
                    actions.push(UiAction::NavigateBack);
                }
                if ui
                    .add_enabled(
                        !tab.history_forward.is_empty(),
                        egui::Button::new("Forward >"),
                    )
                    .on_hover_text("Go forward (Alt+Right)")
                    .clicked()
                {
                    actions.push(UiAction::NavigateForward);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_pool = tab.source.kind == SourceKind::Pool;
                    let is_artist = tab.source.kind == SourceKind::Artist;
                    let is_bank = tab.source.kind == SourceKind::SpankBank;

                    if is_pool || is_bank {
                        let dl_txt = if is_pool {
                            if let Some(ref p) = tab.source.pool {
                                format!("Download Pool ({} posts)", p.post_count())
                            } else {
                                "Download Pool".to_string()
                            }
                        } else {
                            "Download Spank Bank".to_string()
                        };
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(dl_txt).strong().color(Color32::WHITE),
                                )
                                .fill(theme::ACCENT_GREEN_BTN),
                            )
                            .clicked()
                        {
                            actions.push(UiAction::DownloadCurrentPoolOrBank);
                        }
                    }

                    if is_pool || is_artist {
                        let txt = if source_in_bank {
                            if is_pool {
                                "In Bank"
                            } else {
                                "Artist In Bank"
                            }
                        } else if is_pool {
                            "+ Bank"
                        } else {
                            "+ Bank Artist"
                        };
                        let btn = if source_in_bank {
                            egui::Button::new(RichText::new(txt).strong().color(Color32::WHITE))
                                .fill(theme::ACCENT_AMBER_BG)
                        } else {
                            egui::Button::new(
                                RichText::new(txt).strong().color(theme::ACCENT_AMBER),
                            )
                        };
                        if ui.add(btn).clicked() {
                            actions.push(UiAction::ToggleCurrentSourceInBank);
                        }
                    }

                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Slideshow").strong().color(Color32::WHITE),
                            )
                            .fill(theme::ACCENT_BLUE_BTN),
                        )
                        .on_hover_text("Play full screen slideshow of current posts (Space)")
                        .clicked()
                    {
                        actions.push(UiAction::StartTabSlideshow);
                    }

                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        if let Some(ref err) = tab.error_msg {
                            ui.label(
                                RichText::new(format!("Error loading results: {err}"))
                                    .color(theme::TEXT_ERROR),
                            );
                        } else if tab.posts.is_empty()
                            && tab.exhausted
                            && tab.source.query.is_empty()
                            && tab.source.kind == SourceKind::Search
                        {
                            ui.label("Search for tags or open a pool to get started.");
                        } else {
                            show_result_header(ui, tab, actions);
                        }
                    });
                });
            });
        });

    // Virtualized post grid inside nested CentralPanel
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show_inside(ui, |ui| {
            let avail_w = ui.available_width().max(CELL_W);
            let spacing_x = 6.0;
            let cols = ((avail_w + spacing_x) / (CELL_W + spacing_x))
                .floor()
                .max(1.0) as usize;
            let total_rows = tab.posts.len().div_ceil(cols);

            let mut scroll = ScrollArea::vertical()
                .id_salt(("post_grid_scroll", tab.id))
                .auto_shrink([false, false]);
            if let Some(y) = pending_scroll_restore.take() {
                scroll = scroll.vertical_scroll_offset(y);
            }

            let mut hovered_post: Option<(Post, Pos2)> = None;
            let pointer_pos = ui.input(|i| i.pointer.hover_pos());
            let scrolling_now = ui.input(|i| i.smooth_scroll_delta.length_sq() > 0.1);
            let any_popup = ui.memory(|m| m.any_popup_open());

            let scroll_out = scroll.show_rows(ui, CELL_H, total_rows, |ui, row_range| {
                for row in row_range {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = spacing_x;
                        let start = row * cols;
                        let end = (start + cols).min(tab.posts.len());
                        for idx in start..end {
                            let post = &tab.posts[idx];
                            let is_selected = tab.selected_ids.contains(&post.id);
                            let is_active = tab.active_post_id == Some(post.id);
                            let is_downloaded = tab.downloaded_ids.contains(&post.id);
                            let tex = thumb_textures.get(&post.id);

                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::new(CELL_W, CELL_H), Sense::click());

                            if ui.is_rect_visible(rect) {
                                paint_tile(
                                    ui,
                                    rect,
                                    post,
                                    tex,
                                    is_selected,
                                    is_active,
                                    resp.hovered(),
                                    is_downloaded,
                                );
                            }

                            if resp.double_clicked() {
                                actions.push(UiAction::OpenViewer {
                                    post: post.clone(),
                                    posts: None,
                                    start_slideshow: false,
                                    shuffle: false,
                                });
                            } else if resp.clicked() {
                                let (ctrl, shift) =
                                    ui.input(|i| (i.modifiers.command, i.modifiers.shift));
                                actions.push(UiAction::GridPostSelected {
                                    post: post.clone(),
                                    ctrl,
                                    shift,
                                    index: idx,
                                });
                            }

                            if resp.hovered() && !suppress_hover && !scrolling_now && !any_popup {
                                if let Some(pos) = pointer_pos {
                                    hovered_post = Some((post.clone(), pos));
                                }
                            }

                            let post_cloned = post.clone();
                            resp.context_menu(|ui| {
                                show_grid_context_menu(
                                    ui,
                                    &post_cloned,
                                    bank_has_post_fn,
                                    bank_has_pool_fn,
                                    bank_has_artist_fn,
                                    history_path_fn,
                                    actions,
                                );
                            });
                        }
                    });
                }
            });

            tab.scroll_offset = scroll_out.state.offset.y;

            let content_h = scroll_out.content_size.y;
            let viewport_h = scroll_out.inner_rect.height();
            let near_bottom = content_h > viewport_h
                && scroll_out.state.offset.y + viewport_h >= content_h - TILE * 2.0;
            let not_scrollable = content_h <= viewport_h + 10.0;

            if !tab.loading
                && !tab.exhausted
                && tab.source.kind != SourceKind::Pool
                && (near_bottom || not_scrollable)
            {
                actions.push(UiAction::LoadNextPage);
            }

            if let Some((post, pos)) = hovered_post {
                let tex = preview_textures
                    .get(&post.id)
                    .or_else(|| thumb_textures.get(&post.id));
                show_hover_preview_popup(ui.ctx(), &post, tex, pos);
            }
        });
}

fn show_result_header(ui: &mut Ui, tab: &ResultTab, actions: &mut Vec<UiAction>) {
    let s = &tab.source;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        match s.kind {
            SourceKind::Pool => {
                if let Some(ref pool) = s.pool {
                    ui.label("Pool:");
                    ui.label(RichText::new(pool.display_name()).strong());
                    ui.label(format!("({} posts)", pool.post_count()));
                } else {
                    ui.label(RichText::new("Pool").strong());
                }
            }
            SourceKind::Favorites => {
                let u = s.label.strip_prefix("favorites_").unwrap_or(&s.label);
                ui.label("Favorites of");
                ui.label(RichText::new(u).strong());
            }
            SourceKind::SpankBank => {
                ui.label("Spank Bank ·");
                ui.label(RichText::new(format!("{} saved", tab.posts.len())).strong());
            }
            SourceKind::Artist => {
                ui.label("Artist:");
                ui.label(RichText::new(&s.label).strong());
                ui.label("(");
                if ui
                    .link(RichText::new("e621 page").color(theme::ACCENT_ARTIST))
                    .clicked()
                {
                    actions.push(UiAction::OpenUrl(artist_page_url(&s.label)));
                }
                ui.label(")");
            }
            SourceKind::ArtistFeed => {
                ui.label("Latest from");
                ui.label(RichText::new("saved artists").strong());
            }
            SourceKind::Search => {
                ui.label("Search:");
                let q = if s.query.is_empty() {
                    "(latest posts)"
                } else {
                    s.query.as_str()
                };
                ui.label(RichText::new(q).strong());
            }
        }
        let more = if tab.exhausted {
            ""
        } else {
            " (scroll for more)"
        };
        let hidden = if tab.hidden > 0 {
            format!(" · {} hidden by blacklist", tab.hidden)
        } else {
            String::new()
        };
        ui.label(format!("· {} shown{more}{hidden}", tab.posts.len()));
    });
}

#[allow(clippy::too_many_arguments)]
fn paint_tile(
    ui: &Ui,
    rect: Rect,
    post: &Post,
    tex: Option<&TextureHandle>,
    selected: bool,
    active: bool,
    hovered: bool,
    downloaded: bool,
) {
    let p = ui.painter();
    if selected || active || hovered {
        let fill = if selected {
            Color32::from_rgba_unmultiplied(0x26, 0x4f, 0x78, 160)
        } else {
            Color32::from_rgba_unmultiplied(0x28, 0x2c, 0x34, 140)
        };
        let stroke = if active {
            Stroke::new(2.0_f32, theme::ACCENT_BLUE)
        } else if selected {
            Stroke::new(1.5_f32, theme::ACCENT_BLUE)
        } else {
            Stroke::new(1.0_f32, theme::BORDER_SUBTLE)
        };
        p.rect(rect, CornerRadius::same(5), fill, stroke, egui::StrokeKind::Inside);
    }

    let img_box = Rect::from_min_size(
        Pos2::new(rect.left() + (CELL_W - TILE) * 0.5, rect.top() + 4.0),
        Vec2::new(TILE, TILE),
    );

    if let Some(t) = tex {
        let fitted = fit_size(t.size_vec2(), Vec2::splat(TILE));
        let img_rect = Rect::from_center_size(img_box.center(), fitted);
        p.image(
            t.id(),
            img_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        p.rect_filled(
            img_box,
            CornerRadius::same(3),
            Color32::from_rgb(0x2b, 0x30, 0x3b),
        );
        let placeholder = if post.preview_url.is_some() {
            "loading…"
        } else {
            "no preview\n(login?)"
        };
        p.text(
            img_box.center(),
            Align2::CENTER_CENTER,
            placeholder,
            FontId::proportional(11.0),
            Color32::from_rgb(0x99, 0xaa, 0xaa),
        );
    }

    // Corner badges matching render_tile()
    if !post.rating.is_empty() {
        draw_badge(
            p,
            img_box,
            true,
            true,
            &post.rating.to_uppercase(),
            rating_color(&post.rating),
        );
    }
    if matches!(post.ext.as_str(), "webm" | "mp4" | "gif" | "swf") {
        draw_badge(
            p,
            img_box,
            true,
            false,
            &post.ext.to_uppercase(),
            Color32::from_rgb(0x33, 0x33, 0x44),
        );
    }
    if !post.pools.is_empty() {
        draw_badge(
            p,
            img_box,
            false,
            false,
            "POOL",
            Color32::from_rgb(0x5a, 0x3e, 0x9e),
        );
    }
    if downloaded {
        draw_badge(
            p,
            img_box,
            false,
            true,
            "SAVED",
            Color32::from_rgb(0x1f, 0x8f, 0x3a),
        );
    }

    let caption = format!("#{}  Score {}", post.id, post.score);
    let cap_pos = Pos2::new(rect.center().x, img_box.bottom() + 14.0);
    p.text(
        cap_pos,
        Align2::CENTER_CENTER,
        caption,
        FontId::proportional(11.5),
        if selected {
            Color32::WHITE
        } else {
            theme::TEXT_PRIMARY
        },
    );
}

fn draw_badge(
    p: &egui::Painter,
    box_rect: Rect,
    left: bool,
    top: bool,
    text: &str,
    bg: Color32,
) {
    let font = FontId::proportional(10.0);
    let galley = p.layout_no_wrap(text.to_string(), font, Color32::WHITE);
    let w = galley.size().x + 8.0;
    let h = 16.0;
    let x = if left {
        box_rect.left() + 2.0
    } else {
        box_rect.right() - 2.0 - w
    };
    let y = if top {
        box_rect.top() + 2.0
    } else {
        box_rect.bottom() - 2.0 - h
    };
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
    p.rect_filled(r, CornerRadius::same(4), bg);
    p.galley(
        Pos2::new(r.left() + 4.0, r.top() + (h - galley.size().y) * 0.5),
        galley,
        Color32::WHITE,
    );
}

fn show_hover_preview_popup(
    ctx: &egui::Context,
    post: &Post,
    tex: Option<&TextureHandle>,
    pointer: Pos2,
) {
    let screen = ctx.screen_rect();
    let popup_w = 396.0;
    let popup_h = 376.0;
    let mut x = pointer.x + 24.0;
    if x + popup_w > screen.right() - 10.0 {
        x = (pointer.x - popup_w - 24.0).max(screen.left() + 10.0);
    }
    let y = (pointer.y - popup_h * 0.5).clamp(
        screen.top() + 10.0,
        (screen.bottom() - popup_h - 10.0).max(screen.top() + 10.0),
    );

    egui::Area::new(Id::new("hover_preview_popup"))
        .order(Order::Tooltip)
        .interactable(false)
        .fixed_pos(Pos2::new(x, y))
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(Color32::from_rgb(0x1a, 0x1d, 0x24))
                .stroke(Stroke::new(2.0_f32, theme::ACCENT_BLUE))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.set_width(380.0);
                    egui::Frame::NONE
                        .fill(Color32::from_rgb(0x12, 0x14, 0x1a))
                        .corner_radius(CornerRadius::same(4))
                        .show(ui, |ui| {
                            ui.set_min_size(Vec2::new(380.0, 320.0));
                            ui.set_max_size(Vec2::new(380.0, 320.0));
                            ui.centered_and_justified(|ui| {
                                if let Some(t) = tex {
                                    let sz = fit_size(t.size_vec2(), Vec2::new(380.0, 320.0));
                                    ui.image((t.id(), sz));
                                } else {
                                    let txt = if post.preview_url.is_none() {
                                        "No preview image"
                                    } else {
                                        "Loading…"
                                    };
                                    ui.label(txt);
                                }
                            });
                        });
                    let artists = {
                        let a = post.artists();
                        if a.is_empty() {
                            "unknown".to_string()
                        } else {
                            a.join(", ")
                        }
                    };
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(format!("#{} · {}", post.id, format_post_details(post)))
                                .small()
                                .color(Color32::from_rgb(0xcb, 0xd5, 0xe1)),
                        );
                        ui.label(
                            RichText::new(format!("Artist: {artists}"))
                                .small()
                                .color(Color32::from_rgb(0xa0, 0xae, 0xc0)),
                        );
                    });
                });
        });
}

fn show_grid_context_menu(
    ui: &mut Ui,
    post: &Post,
    bank_has_post_fn: &dyn Fn(u64) -> bool,
    bank_has_pool_fn: &dyn Fn(u64) -> bool,
    bank_has_artist_fn: &dyn Fn(&str) -> bool,
    history_path_fn: &dyn Fn(u64) -> Option<String>,
    actions: &mut Vec<UiAction>,
) {
    if ui.button("Full Screen View").clicked() {
        actions.push(UiAction::OpenViewer {
            post: post.clone(),
            posts: None,
            start_slideshow: false,
            shuffle: false,
        });
        ui.close_menu();
    }
    if ui.button("Start Slideshow from here").clicked() {
        actions.push(UiAction::OpenViewer {
            post: post.clone(),
            posts: None,
            start_slideshow: true,
            shuffle: false,
        });
        ui.close_menu();
    }
    let in_bank = bank_has_post_fn(post.id);
    if ui
        .button(if in_bank {
            "Remove from Spank Bank"
        } else {
            "Save to Spank Bank"
        })
        .clicked()
    {
        actions.push(UiAction::TogglePostInBank(post.clone()));
        ui.close_menu();
    }
    if ui
        .button(if post.is_favorited {
            "Remove favorite from e621"
        } else {
            "Favorite on e621"
        })
        .clicked()
    {
        actions.push(UiAction::ToggleFavorite(post.clone()));
        ui.close_menu();
    }
    if ui
        .button(if post.own_vote == 1 {
            "Remove upvote on e621"
        } else {
            "Upvote on e621 (+1)"
        })
        .clicked()
    {
        actions.push(UiAction::ToggleUpvote(post.clone()));
        ui.close_menu();
    }
    if ui.button("Download").clicked() {
        actions.push(UiAction::DownloadPosts(vec![post.clone()]));
        ui.close_menu();
    }
    if ui.button("Open on e621").clicked() {
        actions.push(UiAction::OpenUrl(post.page_url()));
        ui.close_menu();
    }
    if let Some(saved) = history_path_fn(post.id) {
        let p = PathBuf::from(&saved);
        if p.exists() && ui.button("Show saved file").clicked() {
            if let Some(parent) = p.parent() {
                actions.push(UiAction::OpenPath(parent.to_path_buf()));
            }
            ui.close_menu();
        }
    }

    let artists = post.artists();
    if !artists.is_empty() || !post.pools.is_empty() {
        ui.separator();
    }
    for a in artists.iter().take(3) {
        if ui.button(format!("View artist “{a}”")).clicked() {
            actions.push(UiAction::OpenArtist {
                name: a.clone(),
                new_tab: false,
            });
            ui.close_menu();
        }
        ui.menu_button(format!("More for artist “{a}”"), |ui| {
            if ui.button("Open artist in new tab").clicked() {
                actions.push(UiAction::OpenArtist {
                    name: a.clone(),
                    new_tab: true,
                });
                ui.close_menu();
            }
            let artist_bank = bank_has_artist_fn(a);
            if ui
                .button(if artist_bank {
                    "Remove artist from Spank Bank"
                } else {
                    "Save artist to Spank Bank"
                })
                .clicked()
            {
                actions.push(UiAction::ToggleArtistInBank(a.clone()));
                ui.close_menu();
            }
            if ui.button("Open artist page on e621").clicked() {
                actions.push(UiAction::OpenUrl(artist_page_url(a)));
                ui.close_menu();
            }
            ui.separator();
            if ui.button(format!("Add “{a}” to search")).clicked() {
                actions.push(UiAction::ToggleTagInSearch(a.clone()));
                ui.close_menu();
            }
            if ui.button(format!("Search ONLY “{a}”")).clicked() {
                actions.push(UiAction::SearchOnlyTag(a.clone()));
                ui.close_menu();
            }
        });
    }
    for &pid in post.pools.iter().take(3) {
        if ui.button(format!("Open pool #{pid}")).clicked() {
            actions.push(UiAction::OpenPoolById {
                pool_id: pid,
                new_tab: false,
            });
            ui.close_menu();
        }
        if ui.button(format!("Open pool #{pid} in new tab")).clicked() {
            actions.push(UiAction::OpenPoolById {
                pool_id: pid,
                new_tab: true,
            });
            ui.close_menu();
        }
        let pool_bank = bank_has_pool_fn(pid);
        if ui
            .button(if pool_bank {
                format!("Remove pool #{pid} from Spank Bank")
            } else {
                format!("Save pool #{pid} to Spank Bank")
            })
            .clicked()
        {
            actions.push(UiAction::TogglePoolByIdInBank(pid));
            ui.close_menu();
        }
        if ui.button(format!("Download pool #{pid}")).clicked() {
            actions.push(UiAction::DownloadPoolById(pid));
            ui.close_menu();
        }
    }
}
