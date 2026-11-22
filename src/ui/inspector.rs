//! Right Tag Inspector panel and reusable artist/pool/tag-list widgets.

use std::collections::HashSet;

use egui::{Color32, RichText, ScrollArea, TextEdit, TextureHandle, Ui};

use crate::api::{artist_page_url, is_non_artist_tag, tag_category_id, Post};
use crate::tags::category_color;
use crate::ui::state::UiAction;
use crate::ui::theme;

#[allow(clippy::too_many_arguments)]
pub fn show_inspector(
    ui: &mut Ui,
    post: Option<&Post>,
    thumb_tex: Option<&TextureHandle>,
    active_query: &str,
    filter_text: &mut String,
    in_bank: bool,
    pool_in_bank: bool,
    bank_has_artist_fn: &dyn Fn(&str) -> bool,
    actions: &mut Vec<UiAction>,
) {
    let Some(post) = post else {
        ui.label(RichText::new("No post selected").strong());
        let box_w = (ui.available_width() - 16.0).max(100.0);
        egui::Frame::group(ui.style())
            .fill(Color32::from_rgb(0x25, 0x28, 0x30))
            .show(ui, |ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(box_w, 140.0),
                    egui::Layout::centered_and_justified(egui::Direction::TopDown),
                    |ui| {
                        ui.label(
                            RichText::new("Select a post")
                                .color(Color32::from_rgb(0x77, 0x77, 0x88)),
                        );
                    },
                );
            });
        ui.label(RichText::new("Click an image in the grid to view tags.").color(theme::TEXT_MUTED));
        return;
    };

    // Title + clickable artist names
    let artists = post.artists();
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("Post #{}", post.id)).strong());
        ui.label("·");
        if artists.is_empty() {
            ui.label(RichText::new("unknown").color(theme::ACCENT_ARTIST));
        } else {
            for (idx, artist) in artists.iter().enumerate() {
                if idx > 0 {
                    ui.label(",");
                }
                if ui
                    .link(RichText::new(artist).color(theme::ACCENT_ARTIST).strong())
                    .on_hover_text("Click to view all posts by this artist")
                    .clicked()
                {
                    actions.push(UiAction::OpenArtist {
                        name: artist.clone(),
                        new_tab: false,
                    });
                }
            }
        }
    });

    // Thumbnail preview box (max 240x140)
    let box_w = (ui.available_width() - 16.0).max(100.0);
    let box_h = 140.0;
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(0x25, 0x28, 0x30))
        .show(ui, |ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(box_w, box_h),
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    if let Some(tex) = thumb_tex {
                        let sz =
                            fit_size(tex.size_vec2(), egui::vec2(240.0_f32.min(box_w), box_h));
                        ui.image((tex.id(), sz));
                    } else {
                        let msg = if post.preview_url.is_none() {
                            "No preview"
                        } else {
                            "Loading…"
                        };
                        ui.label(RichText::new(msg).color(Color32::from_rgb(0x77, 0x77, 0x88)));
                    }
                },
            );
        });

    // Post metadata details
    ui.label(
        RichText::new(format_post_details(post))
            .small()
            .color(theme::TEXT_MUTED),
    );

    // Preview / Bank / Download row
    ui.horizontal(|ui| {
        if ui.button("Preview").clicked() {
            actions.push(UiAction::OpenViewer {
                post: post.clone(),
                posts: None,
                start_slideshow: false,
                shuffle: false,
            });
        }
        let bank_label = if in_bank { "In Bank" } else { "+ Bank" };
        let bank_color = if in_bank {
            theme::ACCENT_AMBER
        } else {
            Color32::from_rgb(0x94, 0xa3, 0xb8)
        };
        if ui
            .button(RichText::new(bank_label).color(bank_color).strong())
            .on_hover_text(if in_bank {
                "In Spank Bank (click to remove)"
            } else {
                "Save to Spank Bank for online viewing"
            })
            .clicked()
        {
            actions.push(UiAction::TogglePostInBank(post.clone()));
        }
        if ui.button("Download").clicked() {
            actions.push(UiAction::DownloadPosts(vec![post.clone()]));
        }
    });

    // Favorite / Upvote row
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
        if ui
            .add(fav_btn)
            .on_hover_text(if post.is_favorited {
                "Favorited on e621 (click to remove favorite)"
            } else {
                "Like and favorite on e621 (credits the artist)"
            })
            .clicked()
        {
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
        if ui
            .add(vote_btn)
            .on_hover_text(if post.own_vote == 1 {
                "Upvoted on e621 (click to remove vote)"
            } else {
                "Upvote on e621 (increases post score)"
            })
            .clicked()
        {
            actions.push(UiAction::ToggleUpvote(post.clone()));
        }
    });

    // Artist actions row
    if !artists.is_empty() {
        show_artist_actions_row(ui, &artists, bank_has_artist_fn, "insp_artist", actions);
    }

    // Pool actions row
    if !post.pools.is_empty() {
        show_pool_actions_row(ui, &post.pools, pool_in_bank, "insp_pool", actions);
    }

    ui.label(RichText::new("Tags (click to add to search):").strong());
    ui.horizontal(|ui| {
        ui.add(
            TextEdit::singleline(filter_text)
                .hint_text("Filter tags…")
                .desired_width(ui.available_width() - 28.0),
        );
        if !filter_text.is_empty() && ui.small_button("×").clicked() {
            filter_text.clear();
        }
    });

    show_tag_list(
        ui,
        post,
        active_query,
        filter_text,
        bank_has_artist_fn,
        "inspector_tags",
        None,
        actions,
    );
}

pub fn show_artist_actions_row(
    ui: &mut Ui,
    artists: &[String],
    bank_has_artist_fn: &dyn Fn(&str) -> bool,
    salt: &str,
    actions: &mut Vec<UiAction>,
) {
    let many = artists.len() > 1;
    let any_in_bank = artists.iter().any(|a| bank_has_artist_fn(a));
    ui.horizontal_wrapped(|ui| {
        let view_label = if many { "View Artist…" } else { "View Artist" };
        let view_resp = ui
            .button(RichText::new(view_label).strong().color(theme::ACCENT_ARTIST))
            .on_hover_text(if many {
                "Show all posts by one of this post's artists".to_string()
            } else {
                format!("Show all posts by “{}”", artists[0])
            });
        if !many {
            if view_resp.clicked() {
                actions.push(UiAction::OpenArtist {
                    name: artists[0].clone(),
                    new_tab: false,
                });
            }
        } else {
            let pop_id = ui.make_persistent_id((salt, "view"));
            if view_resp.clicked() {
                ui.memory_mut(|m| m.toggle_popup(pop_id));
            }
            egui::popup_below_widget(
                ui,
                pop_id,
                &view_resp,
                egui::PopupCloseBehavior::CloseOnClick,
                |ui| {
                    for a in artists {
                        if ui.button(format!("View posts by “{a}”")).clicked() {
                            actions.push(UiAction::OpenArtist {
                                name: a.clone(),
                                new_tab: false,
                            });
                        }
                    }
                },
            );
        }

        let tab_resp = ui
            .button(RichText::new("+ Tab").strong().color(theme::ACCENT_ARTIST))
            .on_hover_text("Open this artist's posts in a new tab");
        if !many {
            if tab_resp.clicked() {
                actions.push(UiAction::OpenArtist {
                    name: artists[0].clone(),
                    new_tab: true,
                });
            }
        } else {
            let pop_id = ui.make_persistent_id((salt, "tab"));
            if tab_resp.clicked() {
                ui.memory_mut(|m| m.toggle_popup(pop_id));
            }
            egui::popup_below_widget(
                ui,
                pop_id,
                &tab_resp,
                egui::PopupCloseBehavior::CloseOnClick,
                |ui| {
                    for a in artists {
                        if ui.button(format!("Open “{a}” in new tab")).clicked() {
                            actions.push(UiAction::OpenArtist {
                                name: a.clone(),
                                new_tab: true,
                            });
                        }
                    }
                },
            );
        }

        let bank_text = match (any_in_bank, many) {
            (true, true) => "Artist In Bank…",
            (true, false) => "Artist In Bank",
            (false, true) => "+ Bank Artist…",
            (false, false) => "+ Bank Artist",
        };
        let bank_btn = if any_in_bank {
            egui::Button::new(RichText::new(bank_text).strong().color(Color32::WHITE))
                .fill(theme::ACCENT_AMBER_BG)
        } else {
            egui::Button::new(RichText::new(bank_text).strong().color(theme::ACCENT_AMBER))
        };
        let bank_resp = ui.add(bank_btn);
        if !many {
            if bank_resp.clicked() {
                actions.push(UiAction::ToggleArtistInBank(artists[0].clone()));
            }
        } else {
            let pop_id = ui.make_persistent_id((salt, "bank"));
            if bank_resp.clicked() {
                ui.memory_mut(|m| m.toggle_popup(pop_id));
            }
            egui::popup_below_widget(
                ui,
                pop_id,
                &bank_resp,
                egui::PopupCloseBehavior::CloseOnClick,
                |ui| {
                    for a in artists {
                        let txt = if bank_has_artist_fn(a) {
                            format!("Remove “{a}” from Spank Bank")
                        } else {
                            format!("Save “{a}” to Spank Bank")
                        };
                        if ui.button(txt).clicked() {
                            actions.push(UiAction::ToggleArtistInBank(a.clone()));
                        }
                    }
                },
            );
        }
    });
}

pub fn show_pool_actions_row(
    ui: &mut Ui,
    pools: &[u64],
    pool_in_bank: bool,
    salt: &str,
    actions: &mut Vec<UiAction>,
) {
    let single = pools.len() == 1;
    let pid = pools[0];
    ui.horizontal_wrapped(|ui| {
        let view_txt = if single {
            format!("View Pool #{pid}")
        } else {
            "View Pool…".to_string()
        };
        let view_resp = ui.button(RichText::new(view_txt).strong().color(theme::ACCENT_POOL));
        if single {
            if view_resp.clicked() {
                actions.push(UiAction::OpenPoolById {
                    pool_id: pid,
                    new_tab: false,
                });
            }
        } else {
            let pop_id = ui.make_persistent_id((salt, "view"));
            if view_resp.clicked() {
                ui.memory_mut(|m| m.toggle_popup(pop_id));
            }
            egui::popup_below_widget(
                ui,
                pop_id,
                &view_resp,
                egui::PopupCloseBehavior::CloseOnClick,
                |ui| {
                    for &p in pools {
                        if ui.button(format!("View Pool #{p}")).clicked() {
                            actions.push(UiAction::OpenPoolById {
                                pool_id: p,
                                new_tab: false,
                            });
                        }
                    }
                },
            );
        }

        let tab_resp = ui
            .button(RichText::new("+ Tab").strong().color(theme::ACCENT_POOL))
            .on_hover_text("Open pool in a new tab");
        if single {
            if tab_resp.clicked() {
                actions.push(UiAction::OpenPoolById {
                    pool_id: pid,
                    new_tab: true,
                });
            }
        } else {
            let pop_id = ui.make_persistent_id((salt, "tab"));
            if tab_resp.clicked() {
                ui.memory_mut(|m| m.toggle_popup(pop_id));
            }
            egui::popup_below_widget(
                ui,
                pop_id,
                &tab_resp,
                egui::PopupCloseBehavior::CloseOnClick,
                |ui| {
                    for &p in pools {
                        if ui.button(format!("Open Pool #{p} in new tab")).clicked() {
                            actions.push(UiAction::OpenPoolById {
                                pool_id: p,
                                new_tab: true,
                            });
                        }
                    }
                },
            );
        }

        let bank_txt = if pool_in_bank {
            if single {
                format!("In Bank #{pid}")
            } else {
                "In Bank…".to_string()
            }
        } else if single {
            format!("+ Bank #{pid}")
        } else {
            "+ Bank…".to_string()
        };
        let bank_btn = if pool_in_bank {
            egui::Button::new(RichText::new(bank_txt).strong().color(Color32::WHITE))
                .fill(theme::ACCENT_AMBER_BG)
        } else {
            egui::Button::new(RichText::new(bank_txt).strong().color(theme::ACCENT_POOL))
        };
        let bank_resp = ui.add(bank_btn).on_hover_text("Save/remove pool from Spank Bank");
        if single {
            if bank_resp.clicked() {
                actions.push(UiAction::TogglePoolByIdInBank(pid));
            }
        } else {
            let pop_id = ui.make_persistent_id((salt, "bank"));
            if bank_resp.clicked() {
                ui.memory_mut(|m| m.toggle_popup(pop_id));
            }
            egui::popup_below_widget(
                ui,
                pop_id,
                &bank_resp,
                egui::PopupCloseBehavior::CloseOnClick,
                |ui| {
                    for &p in pools {
                        if ui.button(format!("Save/Remove Pool #{p} in Spank Bank")).clicked() {
                            actions.push(UiAction::TogglePoolByIdInBank(p));
                        }
                    }
                },
            );
        }

        let dl_txt = if single {
            format!("Download Pool #{pid}")
        } else {
            "Download Pool…".to_string()
        };
        let dl_resp = ui.button(
            RichText::new(dl_txt)
                .strong()
                .color(Color32::from_rgb(0x60, 0xa5, 0xfa)),
        );
        if single {
            if dl_resp.clicked() {
                actions.push(UiAction::DownloadPoolById(pid));
            }
        } else {
            let pop_id = ui.make_persistent_id((salt, "dl"));
            if dl_resp.clicked() {
                ui.memory_mut(|m| m.toggle_popup(pop_id));
            }
            egui::popup_below_widget(
                ui,
                pop_id,
                &dl_resp,
                egui::PopupCloseBehavior::CloseOnClick,
                |ui| {
                    for &p in pools {
                        if ui.button(format!("Download Pool #{p}")).clicked() {
                            actions.push(UiAction::DownloadPoolById(p));
                        }
                    }
                },
            );
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub fn show_tag_list(
    ui: &mut Ui,
    post: &Post,
    active_query: &str,
    filter_text: &str,
    bank_has_artist_fn: &dyn Fn(&str) -> bool,
    salt: &str,
    mut selected_tag: Option<&mut Option<String>>,
    actions: &mut Vec<UiAction>,
) {
    let active_tokens: HashSet<&str> = active_query.split_whitespace().collect();
    let filter_q = filter_text.trim().to_lowercase();
    let category_order = [
        "artist",
        "copyright",
        "character",
        "species",
        "general",
        "lore",
        "meta",
    ];

    ScrollArea::vertical()
        .id_salt(salt)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for cat in category_order {
                let Some(tag_list) = post.tags.get(cat) else {
                    continue;
                };
                let mut sorted: Vec<&String> = tag_list.iter().collect();
                sorted.sort();
                let cat_id = tag_category_id(cat);
                let color = category_color(cat_id);

                for tag in sorted {
                    if !filter_q.is_empty() && !tag.to_lowercase().contains(&filter_q) {
                        continue;
                    }
                    let is_inc = active_tokens.contains(tag.as_str());
                    let neg = format!("-{tag}");
                    let is_exc = active_tokens.contains(neg.as_str());
                    let prefix = if is_inc {
                        "[+] "
                    } else if is_exc {
                        "[-] "
                    } else {
                        ""
                    };
                    let is_selected = selected_tag
                        .as_deref()
                        .and_then(|o| o.as_deref())
                        == Some(tag.as_str());

                    let mut rt = RichText::new(format!("{prefix}{tag}")).color(color);
                    if is_inc || is_exc {
                        rt = rt.strong();
                    }
                    let resp = ui.selectable_label(is_selected, rt).on_hover_text(format!(
                        "[{}] {}\nClick to add/remove from search\nRight-click for more options",
                        cat, tag
                    ));

                    if resp.clicked() {
                        if let Some(ref mut sel) = selected_tag {
                            **sel = Some(tag.clone());
                        }
                        actions.push(UiAction::ToggleTagInSearch(tag.clone()));
                    }

                    resp.context_menu(|ui| {
                        if cat == "artist" && !is_non_artist_tag(tag) {
                            if ui.button(format!("View artist “{tag}”")).clicked() {
                                actions.push(UiAction::OpenArtist {
                                    name: tag.clone(),
                                    new_tab: false,
                                });
                                ui.close_menu();
                            }
                            if ui.button("Open artist in new tab").clicked() {
                                actions.push(UiAction::OpenArtist {
                                    name: tag.clone(),
                                    new_tab: true,
                                });
                                ui.close_menu();
                            }
                            let in_bank = bank_has_artist_fn(tag);
                            let bank_lbl = if in_bank {
                                "Remove artist from Spank Bank"
                            } else {
                                "Save artist to Spank Bank"
                            };
                            if ui.button(bank_lbl).clicked() {
                                actions.push(UiAction::ToggleArtistInBank(tag.clone()));
                                ui.close_menu();
                            }
                            if ui.button("Open artist page on e621").clicked() {
                                actions.push(UiAction::OpenUrl(artist_page_url(tag)));
                                ui.close_menu();
                            }
                            ui.separator();
                        }

                        let add_lbl = if is_inc {
                            format!("Remove “{tag}” from search")
                        } else {
                            format!("Add to search (+{tag})")
                        };
                        if ui.button(add_lbl).clicked() {
                            actions.push(UiAction::ToggleTagInSearch(tag.clone()));
                            ui.close_menu();
                        }

                        let exc_lbl = if is_exc {
                            format!("Remove exclusion (-{tag})")
                        } else {
                            format!("Exclude from search (-{tag})")
                        };
                        if ui.button(exc_lbl).clicked() {
                            actions.push(UiAction::ExcludeTagInSearch(tag.clone()));
                            ui.close_menu();
                        }

                        ui.separator();
                        if ui.button(format!("Search only “{tag}”")).clicked() {
                            actions.push(UiAction::SearchOnlyTag(tag.clone()));
                            ui.close_menu();
                        }
                        if ui.button("Copy tag name").clicked() {
                            actions.push(UiAction::CopyText(tag.clone()));
                            ui.close_menu();
                        }
                    });
                }
            }
        });
}

pub fn format_post_details(post: &Post) -> String {
    let mut parts = vec![
        post.ext.to_uppercase(),
        format!("{}×{}", post.width, post.height),
    ];
    if post.size > 0 {
        parts.push(format!("{:.1} MB", post.size as f64 / 1_048_576.0));
    }
    parts.push(format!("Score {}", post.score));
    parts.push(format!("Favs {}", post.fav_count));
    if !post.rating.is_empty() {
        parts.push(format!("Rating {}", post.rating.to_uppercase()));
    }
    parts.join(" · ")
}

pub fn fit_size(img: egui::Vec2, max_box: egui::Vec2) -> egui::Vec2 {
    if img.x <= 0.0 || img.y <= 0.0 || max_box.x <= 0.0 || max_box.y <= 0.0 {
        return max_box;
    }
    let scale = (max_box.x / img.x).min(max_box.y / img.y);
    egui::vec2((img.x * scale).max(1.0), (img.y * scale).max(1.0))
}
