//! Left sidebar panel: Search, Pools, Favorites, and Spank Bank tabs.

use std::collections::HashSet;
use std::time::Instant;

use egui::text::{CCursor, CCursorRange};
use egui::{Color32, Id, Key, Modifiers, Order, RichText, ScrollArea, TextEdit, Ui};

use crate::api::{artist_page_url, Pool, Post};
use crate::settings::{
    Settings, MEDIA_FILTERS, MIN_SCORE_ANY, ORDERS, QUICK_BLACKLIST_ITEMS,
};
use crate::tags::{category_color, format_post_count, TagAutocompleteState};
use crate::ui::state::{BankSubTab, LeftTab, SourceKind, UiAction};
use crate::ui::theme;

#[allow(clippy::too_many_arguments)]
pub fn show_left_panel(
    ui: &mut Ui,
    active_tab: &mut LeftTab,
    bank_subtab: &mut BankSubTab,
    tag_query: &mut String,
    autocomplete: &mut TagAutocompleteState,
    settings: &mut Settings,
    selected_saved_idx: &mut Option<usize>,
    pool_query: &mut String,
    pool_results: &[Pool],
    selected_pool_idx: &mut Option<usize>,
    fav_user: &mut String,
    // Spank Bank state
    bank_pools: &[Pool],
    bank_pools_filter: &mut String,
    selected_bank_pool_ids: &mut HashSet<u64>,
    last_bank_pool_id: &mut Option<u64>,
    bank_posts: &[Post],
    bank_posts_filter: &mut String,
    selected_bank_post_ids: &mut HashSet<u64>,
    bank_artists: &[String],
    bank_artists_filter: &mut String,
    bank_artist_add: &mut String,
    selected_bank_artists: &mut HashSet<String>,
    last_bank_artist: &mut Option<String>,
    bank_has_pool_fn: &dyn Fn(u64) -> bool,
    focus_search_requested: &mut bool,
    actions: &mut Vec<UiAction>,
) {
    ui.horizontal(|ui| {
        ui.selectable_value(active_tab, LeftTab::Search, "Search");
        ui.selectable_value(active_tab, LeftTab::Pools, "Pools");
        ui.selectable_value(active_tab, LeftTab::Favorites, "Favorites");
        ui.selectable_value(active_tab, LeftTab::SpankBank, "Spank Bank");
    });
    ui.separator();

    match *active_tab {
        LeftTab::Search => show_search_tab(
            ui,
            tag_query,
            autocomplete,
            settings,
            selected_saved_idx,
            focus_search_requested,
            actions,
        ),
        LeftTab::Pools => show_pools_tab(
            ui,
            pool_query,
            pool_results,
            selected_pool_idx,
            bank_has_pool_fn,
            actions,
        ),
        LeftTab::Favorites => show_favorites_tab(ui, fav_user, actions),
        LeftTab::SpankBank => show_spank_bank_tab(
            ui,
            bank_subtab,
            bank_pools,
            bank_pools_filter,
            selected_bank_pool_ids,
            last_bank_pool_id,
            bank_posts,
            bank_posts_filter,
            selected_bank_post_ids,
            bank_artists,
            bank_artists_filter,
            bank_artist_add,
            selected_bank_artists,
            last_bank_artist,
            actions,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn show_search_tab(
    ui: &mut Ui,
    tag_query: &mut String,
    autocomplete: &mut TagAutocompleteState,
    settings: &mut Settings,
    selected_saved_idx: &mut Option<usize>,
    focus_search_requested: &mut bool,
    actions: &mut Vec<UiAction>,
) {
    let search_edit_id = Id::new("main_tag_search_edit");
    let has_focus = ui.memory(|m| m.has_focus(search_edit_id));
    let mut pending_insert: Option<(String, bool, bool)> = None; // (tag, add_space, submit)

    if has_focus && autocomplete.popup_open && !autocomplete.suggestions.is_empty() {
        let mut close_popup = false;
        ui.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                if autocomplete.selected_row + 1 < autocomplete.suggestions.len() {
                    autocomplete.selected_row += 1;
                }
            } else if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                autocomplete.selected_row = autocomplete.selected_row.saturating_sub(1);
            } else if i.consume_key(Modifiers::NONE, Key::Tab) {
                if let Some(tag) = autocomplete.suggestions.get(autocomplete.selected_row) {
                    pending_insert = Some((tag.name.clone(), true, false));
                }
                close_popup = true;
            } else if i.consume_key(Modifiers::NONE, Key::Enter) {
                if let Some(tag) = autocomplete.suggestions.get(autocomplete.selected_row) {
                    pending_insert = Some((tag.name.clone(), false, true));
                }
                close_popup = true;
            } else if i.consume_key(Modifiers::NONE, Key::Escape) {
                close_popup = true;
            }
        });
        if close_popup {
            autocomplete.popup_open = false;
        }
    }

    if let Some((tag_name, add_space, submit)) = pending_insert.take() {
        let cursor = autocomplete
            .last_cursor_char
            .unwrap_or_else(|| tag_query.chars().count());
        let (new_text, new_cursor) =
            TagAutocompleteState::insert_tag(tag_query, cursor, &tag_name, add_space);
        *tag_query = new_text;
        autocomplete.last_cursor_char = Some(new_cursor);
        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), search_edit_id) {
            state
                .cursor
                .set_char_range(Some(CCursorRange::one(CCursor::new(new_cursor))));
            state.store(ui.ctx(), search_edit_id);
        }
        ui.memory_mut(|m| m.request_focus(search_edit_id));
        if submit {
            actions.push(UiAction::StartSearch {
                raw: None,
                label: None,
                kind: SourceKind::Search,
                new_tab: false,
            });
        }
    }

    ScrollArea::vertical()
        .id_salt("left_search_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.label("Tags:");
            let te_out = TextEdit::singleline(tag_query)
                .id(search_edit_id)
                .hint_text("e.g. wolf solo -comic order:score")
                .desired_width(f32::INFINITY)
                .show(ui);

            if *focus_search_requested {
                *focus_search_requested = false;
                te_out.response.request_focus();
            }

            let cursor_char = te_out
                .cursor_range
                .map(|r| r.primary.ccursor.index)
                .unwrap_or_else(|| tag_query.chars().count());
            autocomplete.last_cursor_char = Some(cursor_char);

            let now = Instant::now();
            if te_out.response.changed() {
                autocomplete.on_text_edited(tag_query, cursor_char, now);
            }
            if let Some(tok) = autocomplete.take_due_remote_query(tag_query, cursor_char, now) {
                actions.push(UiAction::TriggerAutocomplete(tok));
            } else if autocomplete.debounce_since.is_some() {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(60));
            }

            if te_out.response.lost_focus()
                && !autocomplete.popup_open
                && ui.input(|i| i.key_pressed(Key::Enter))
            {
                actions.push(UiAction::StartSearch {
                    raw: None,
                    label: None,
                    kind: SourceKind::Search,
                    new_tab: false,
                });
            }

            // Floating autocomplete popup anchored directly under the tag search input
            if autocomplete.popup_open && !autocomplete.suggestions.is_empty() {
                let rect = te_out.response.rect;
                let popup_pos = egui::pos2(rect.left(), rect.bottom() + 2.0);
                let popup_w = rect.width().max(340.0);
                let popup_area = egui::Area::new(Id::new("tag_autocomplete_popup"))
                    .order(Order::Foreground)
                    .fixed_pos(popup_pos)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style())
                            .fill(Color32::from_rgb(0x1e, 0x22, 0x2b))
                            .stroke(egui::Stroke::new(1.0_f32, theme::ACCENT_BLUE))
                            .show(ui, |ui| {
                                ui.set_min_width(popup_w);
                                ui.set_max_width(popup_w);
                                ScrollArea::vertical()
                                    .max_height(260.0)
                                    .show(ui, |ui| {
                                        let mut clicked_tag: Option<String> = None;
                                        for (idx, t) in autocomplete.suggestions.iter().enumerate() {
                                            let selected = idx == autocomplete.selected_row;
                                            let cnt = if t.post_count > 0 {
                                                format!("({})", format_post_count(t.post_count))
                                            } else {
                                                String::new()
                                            };
                                            let label = format!(
                                                "{:<24} {:>10} · {}",
                                                t.name,
                                                cnt,
                                                t.category_name()
                                            );
                                            let text = RichText::new(label)
                                                .color(category_color(t.category))
                                                .size(12.0);
                                            if ui.selectable_label(selected, text).clicked() {
                                                clicked_tag = Some(t.name.clone());
                                            }
                                        }
                                        if let Some(name) = clicked_tag {
                                            let (new_text, new_cursor) =
                                                TagAutocompleteState::insert_tag(
                                                    tag_query,
                                                    cursor_char,
                                                    &name,
                                                    false,
                                                );
                                            *tag_query = new_text;
                                            autocomplete.popup_open = false;
                                            autocomplete.last_cursor_char = Some(new_cursor);
                                            if let Some(mut state) =
                                                egui::TextEdit::load_state(ui.ctx(), search_edit_id)
                                            {
                                                state.cursor.set_char_range(Some(
                                                    CCursorRange::one(CCursor::new(new_cursor)),
                                                ));
                                                state.store(ui.ctx(), search_edit_id);
                                            }
                                            ui.memory_mut(|m| m.request_focus(search_edit_id));
                                        }
                                    });
                            });
                    });
                if !te_out.response.has_focus()
                    && ui.input(|i| i.pointer.any_click())
                    && !popup_area.response.hovered()
                {
                    autocomplete.popup_open = false;
                }
            }

            ui.label(
                RichText::new(
                    "Space-separated tags. -tag excludes, ~tag = OR. Metatags like score:>100 and artist:name work too.",
                )
                .small()
                .color(theme::TEXT_MUTED),
            );
            ui.add_space(4.0);

            let mut filter_changed = false;
            egui::Grid::new("search_filters_grid")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Rating:");
                    ui.horizontal(|ui| {
                        filter_changed |= ui.checkbox(&mut settings.rating_s, "Safe").changed();
                        filter_changed |= ui.checkbox(&mut settings.rating_q, "Questionable").changed();
                        filter_changed |= ui.checkbox(&mut settings.rating_e, "Explicit").changed();
                    });
                    ui.end_row();

                    ui.label("Media:");
                    let media_label = MEDIA_FILTERS
                        .iter()
                        .find(|(_, v)| *v == settings.media_filter)
                        .map(|(l, _)| *l)
                        .unwrap_or("All media");
                    egui::ComboBox::from_id_salt("media_filter_combo")
                        .selected_text(media_label)
                        .width(210.0)
                        .show_ui(ui, |ui| {
                            for (label, val) in MEDIA_FILTERS {
                                if ui
                                    .selectable_label(settings.media_filter == *val, *label)
                                    .clicked()
                                {
                                    settings.media_filter = (*val).to_string();
                                    filter_changed = true;
                                }
                            }
                        });
                    ui.end_row();

                    ui.label("Pools:");
                    filter_changed |= ui
                        .checkbox(&mut settings.pool_only, "In pools only")
                        .changed();
                    ui.end_row();

                    ui.label("Min score:");
                    ui.horizontal(|ui| {
                        let dv = egui::DragValue::new(&mut settings.min_score)
                            .range(MIN_SCORE_ANY..=100_000)
                            .speed(1.0)
                            .custom_formatter(|v, _| {
                                if (v as i64) <= MIN_SCORE_ANY {
                                    "Any".to_string()
                                } else {
                                    format!("{}", v as i64)
                                }
                            });
                        filter_changed |= ui.add(dv).changed();
                        if settings.min_score > MIN_SCORE_ANY && ui.small_button("Any").clicked() {
                            settings.min_score = MIN_SCORE_ANY;
                            filter_changed = true;
                        }
                    });
                    ui.end_row();

                    ui.label("Sort:");
                    let order_label = ORDERS
                        .iter()
                        .find(|(_, v)| *v == settings.order)
                        .map(|(l, _)| *l)
                        .unwrap_or("Newest");
                    egui::ComboBox::from_id_salt("order_combo")
                        .selected_text(order_label)
                        .width(210.0)
                        .show_ui(ui, |ui| {
                            for (label, val) in ORDERS {
                                if ui
                                    .selectable_label(settings.order == *val, *label)
                                    .clicked()
                                {
                                    settings.order = (*val).to_string();
                                    filter_changed = true;
                                }
                            }
                        });
                    ui.end_row();
                });

            if filter_changed {
                actions.push(UiAction::SaveFiltersToSettings);
            }

            ui.add_space(4.0);
            if ui
                .add_sized(
                    [ui.available_width(), 30.0],
                    egui::Button::new(RichText::new("Search").strong()),
                )
                .clicked()
            {
                actions.push(UiAction::StartSearch {
                    raw: None,
                    label: None,
                    kind: SourceKind::Search,
                    new_tab: false,
                });
            }

            ui.add_space(6.0);

            // Collapsible Quick Blacklist
            let bl_cnt = settings.quick_blacklist.len();
            let bl_badge = if bl_cnt > 0 {
                format!("  [{bl_cnt} active]")
            } else {
                String::new()
            };
            let bl_prefix = if settings.expand_blacklist {
                "[-] "
            } else {
                "[+] "
            };
            if ui
                .add_sized(
                    [ui.available_width(), 26.0],
                    egui::Button::new(
                        RichText::new(format!("{bl_prefix}Quick Blacklist (Exclude){bl_badge}"))
                            .strong(),
                    ),
                )
                .clicked()
            {
                settings.expand_blacklist = !settings.expand_blacklist;
                actions.push(UiAction::SaveFiltersToSettings);
            }

            if settings.expand_blacklist {
                egui::Frame::group(ui.style())
                    .fill(theme::BG_PANEL)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("One-click exclusions applied to query and local filter.")
                                .small()
                                .color(theme::TEXT_MUTED),
                        );
                        let mut bl_changed = false;
                        egui::Grid::new("quick_bl_grid")
                            .num_columns(2)
                            .spacing([8.0, 4.0])
                            .show(ui, |ui| {
                                for (idx, (tag_id, label)) in
                                    QUICK_BLACKLIST_ITEMS.iter().enumerate()
                                {
                                    let mut checked =
                                        settings.quick_blacklist.iter().any(|t| t == tag_id);
                                    if ui.checkbox(&mut checked, *label).changed() {
                                        if checked {
                                            if !settings.quick_blacklist.iter().any(|t| t == tag_id)
                                            {
                                                settings
                                                    .quick_blacklist
                                                    .push((*tag_id).to_string());
                                            }
                                        } else {
                                            settings.quick_blacklist.retain(|t| t != tag_id);
                                        }
                                        bl_changed = true;
                                    }
                                    if idx % 2 == 1 {
                                        ui.end_row();
                                    }
                                }
                            });
                        if bl_changed {
                            actions.push(UiAction::SaveFiltersToSettings);
                        }
                        ui.horizontal(|ui| {
                            if ui
                                .button("Select Common")
                                .on_hover_text("Select Gore, Scat, Watersports, Young/Cub, Flash")
                                .clicked()
                            {
                                actions.push(UiAction::SelectCommonBlacklist);
                            }
                            if ui.button("Clear All").clicked() {
                                actions.push(UiAction::ClearQuickBlacklist);
                            }
                        });
                    });
            }

            ui.add_space(4.0);

            // Collapsible Saved Searches
            let sv_cnt = settings.saved_searches.len();
            let sv_badge = if sv_cnt > 0 {
                format!("  [{sv_cnt} saved]")
            } else {
                String::new()
            };
            let sv_prefix = if settings.expand_saved {
                "[-] "
            } else {
                "[+] "
            };
            if ui
                .add_sized(
                    [ui.available_width(), 26.0],
                    egui::Button::new(
                        RichText::new(format!("{sv_prefix}Saved Searches{sv_badge}")).strong(),
                    ),
                )
                .clicked()
            {
                settings.expand_saved = !settings.expand_saved;
                actions.push(UiAction::SaveFiltersToSettings);
            }

            if settings.expand_saved {
                egui::Frame::group(ui.style())
                    .fill(theme::BG_PANEL)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Double-click search below to run.")
                                .small()
                                .color(theme::TEXT_MUTED),
                        );
                        ScrollArea::vertical()
                            .id_salt("saved_searches_list")
                            .max_height(140.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for (idx, item) in settings.saved_searches.iter().enumerate() {
                                    let selected = *selected_saved_idx == Some(idx);
                                    let resp = ui.selectable_label(selected, item);
                                    if resp.clicked() {
                                        *selected_saved_idx = Some(idx);
                                    }
                                    if resp.double_clicked() {
                                        *selected_saved_idx = Some(idx);
                                        actions.push(UiAction::RunSavedSearch(item.clone()));
                                    }
                                }
                            });
                        ui.horizontal(|ui| {
                            if ui.button("Save current").clicked() {
                                actions.push(UiAction::SaveSearch);
                            }
                            if let Some(idx) = *selected_saved_idx {
                                if ui.button("Remove").clicked() {
                                    actions.push(UiAction::RemoveSavedSearch(idx));
                                }
                            } else {
                                ui.add_enabled(false, egui::Button::new("Remove"));
                            }
                        });
                    });
            }
        });
}

fn show_pools_tab(
    ui: &mut Ui,
    pool_query: &mut String,
    pool_results: &[Pool],
    selected_pool_idx: &mut Option<usize>,
    bank_has_pool_fn: &dyn Fn(u64) -> bool,
    actions: &mut Vec<UiAction>,
) {
    egui::TopBottomPanel::top("pools_search_top")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 2)))
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let resp = ui.add(
                    TextEdit::singleline(pool_query)
                        .hint_text("Pool name or ID")
                        .desired_width((ui.available_width() - 90.0).max(100.0)),
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    actions.push(UiAction::SearchPools);
                }
                if ui.button("Find pools").clicked() {
                    actions.push(UiAction::SearchPools);
                }
            });
        });

    let selected_pool = selected_pool_idx.and_then(|i| pool_results.get(i).cloned());

    egui::TopBottomPanel::bottom("pools_actions_bottom")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 4)))
        .show_inside(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(selected_pool.is_some(), egui::Button::new("Show pool"))
                    .clicked()
                {
                    if let Some(p) = selected_pool.clone() {
                        actions.push(UiAction::OpenPool {
                            pool: p,
                            new_tab: false,
                        });
                    }
                }
                if ui
                    .add_enabled(selected_pool.is_some(), egui::Button::new("+ New Tab"))
                    .on_hover_text("Open pool in a new tab")
                    .clicked()
                {
                    if let Some(p) = selected_pool.clone() {
                        actions.push(UiAction::OpenPool {
                            pool: p,
                            new_tab: true,
                        });
                    }
                }
                let in_bank = selected_pool
                    .as_ref()
                    .map(|p| bank_has_pool_fn(p.id))
                    .unwrap_or(false);
                let bank_btn = egui::Button::new(
                    RichText::new(if in_bank { "In Bank" } else { "+ Bank" })
                        .strong()
                        .color(if in_bank {
                            theme::ACCENT_AMBER
                        } else {
                            theme::ACCENT_POOL
                        }),
                );
                if ui
                    .add_enabled(selected_pool.is_some(), bank_btn)
                    .on_hover_text("Save/remove selected pool in Spank Bank")
                    .clicked()
                {
                    if let Some(p) = selected_pool.clone() {
                        actions.push(UiAction::TogglePoolInBank(p));
                    }
                }
                if ui
                    .add_enabled(
                        selected_pool.is_some(),
                        egui::Button::new(RichText::new("Download pool").strong()),
                    )
                    .clicked()
                {
                    if let Some(p) = selected_pool.clone() {
                        actions.push(UiAction::DownloadPool(p));
                    }
                }
            });

            ui.label(
                RichText::new(
                    "Pools are saved to pools/<name> (id)/ with pages numbered in reading order (0001_…, 0002_…).",
                )
                .small()
                .color(theme::TEXT_MUTED),
            );
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show_inside(ui, |ui| {
            egui::Frame::group(ui.style())
                .fill(theme::BG_SURFACE)
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .id_salt("pool_results_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for (idx, p) in pool_results.iter().enumerate() {
                                let selected = *selected_pool_idx == Some(idx);
                                let label = format!("{}  [{}]", p.display_name(), p.post_count());
                                let resp = ui
                                    .selectable_label(selected, label)
                                    .on_hover_text(format!(
                                        "Pool #{} · {} · {} posts",
                                        p.id,
                                        p.category,
                                        p.post_count()
                                    ));
                                if resp.clicked() {
                                    *selected_pool_idx = Some(idx);
                                }
                                if resp.double_clicked() {
                                    *selected_pool_idx = Some(idx);
                                    actions.push(UiAction::OpenPool {
                                        pool: p.clone(),
                                        new_tab: false,
                                    });
                                }
                            }
                        });
                });
        });
}

fn show_favorites_tab(ui: &mut Ui, fav_user: &mut String, actions: &mut Vec<UiAction>) {
    ui.label("User:");
    let resp = ui.add(
        TextEdit::singleline(fav_user)
            .hint_text("username (defaults to your account)")
            .desired_width(f32::INFINITY),
    );
    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
        actions.push(UiAction::LoadFavorites);
    }
    if ui.button("Load favorites").clicked() {
        actions.push(UiAction::LoadFavorites);
    }
    ui.label(
        RichText::new(
            "Uses the rating / score / sort filters from the Search tab. Private favorites require your own API key.",
        )
        .small()
        .color(theme::TEXT_MUTED),
    );
}

#[allow(clippy::too_many_arguments)]
fn show_spank_bank_tab(
    ui: &mut Ui,
    subtab: &mut BankSubTab,
    bank_pools: &[Pool],
    bank_pools_filter: &mut String,
    selected_bank_pool_ids: &mut HashSet<u64>,
    last_bank_pool_id: &mut Option<u64>,
    bank_posts: &[Post],
    bank_posts_filter: &mut String,
    selected_bank_post_ids: &mut HashSet<u64>,
    bank_artists: &[String],
    bank_artists_filter: &mut String,
    bank_artist_add: &mut String,
    selected_bank_artists: &mut HashSet<String>,
    last_bank_artist: &mut Option<String>,
    actions: &mut Vec<UiAction>,
) {
    egui::TopBottomPanel::top("spank_bank_subtabs_top")
        .frame(egui::Frame::NONE)
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(
                    subtab,
                    BankSubTab::Pools,
                    format!("Pools ({})", bank_pools.len()),
                );
                ui.selectable_value(
                    subtab,
                    BankSubTab::Posts,
                    format!("Posts ({})", bank_posts.len()),
                );
                ui.selectable_value(
                    subtab,
                    BankSubTab::Artists,
                    format!("Artists ({})", bank_artists.len()),
                );
            });
            ui.separator();
        });

    egui::TopBottomPanel::bottom("spank_bank_footer")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 4)))
        .show_inside(ui, |ui| {
            if ui
                .add_sized(
                    [ui.available_width(), 28.0],
                    egui::Button::new(
                        RichText::new("Download Spank Bank")
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(theme::ACCENT_GREEN_BTN),
                )
                .on_hover_text("Download all saved posts and pools in the Spank Bank")
                .clicked()
            {
                actions.push(UiAction::DownloadEntireBank);
            }
            ui.add_space(2.0);
            ui.label(
                RichText::new(
                    "The Spank Bank lets you save online pools, posts and artists for later viewing or batch downloading. Play full-screen slideshows with timers in reading order or shuffle.",
                )
                .small()
                .color(theme::TEXT_MUTED),
            );
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show_inside(ui, |ui| match *subtab {
            BankSubTab::Pools => show_bank_pools_subtab(
                ui,
                bank_pools,
                bank_pools_filter,
                selected_bank_pool_ids,
                last_bank_pool_id,
                actions,
            ),
            BankSubTab::Posts => show_bank_posts_subtab(
                ui,
                bank_posts,
                bank_posts_filter,
                selected_bank_post_ids,
                actions,
            ),
            BankSubTab::Artists => show_bank_artists_subtab(
                ui,
                bank_artists,
                bank_artists_filter,
                bank_artist_add,
                selected_bank_artists,
                last_bank_artist,
                actions,
            ),
        });
}

fn show_bank_pools_subtab(
    ui: &mut Ui,
    bank_pools: &[Pool],
    filter: &mut String,
    selected_ids: &mut HashSet<u64>,
    last_id: &mut Option<u64>,
    actions: &mut Vec<UiAction>,
) {
    let cnt = bank_pools.len();
    egui::TopBottomPanel::top("bank_pools_top")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 2)))
        .show_inside(ui, |ui| {
            ui.label(
                RichText::new(format!(
                    "{cnt} saved pool{}",
                    if cnt == 1 { "" } else { "s" }
                ))
                .strong()
                .size(13.0)
                .color(theme::ACCENT_AMBER),
            );

            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(filter)
                        .hint_text("Filter saved pools by name or ID…")
                        .desired_width((ui.available_width() - 28.0).max(100.0)),
                );
                if !filter.is_empty() && ui.small_button("×").clicked() {
                    filter.clear();
                }
            });
        });

    let active_pool = last_id
        .and_then(|id| bank_pools.iter().find(|p| p.id == id))
        .or_else(|| {
            selected_ids
                .iter()
                .next()
                .and_then(|id| bank_pools.iter().find(|p| p.id == *id))
        })
        .cloned();

    egui::TopBottomPanel::bottom("bank_pools_bottom")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 4)))
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button(RichText::new("View Pool").strong()).clicked() {
                    if let Some(p) = active_pool.clone() {
                        actions.push(UiAction::OpenPool {
                            pool: p,
                            new_tab: false,
                        });
                    }
                }
                if ui.button("+ New Tab").clicked() {
                    if let Some(p) = active_pool.clone() {
                        actions.push(UiAction::OpenPool {
                            pool: p,
                            new_tab: true,
                        });
                    }
                }
            });

            ui.horizontal(|ui| {
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("Play In Order").strong().color(Color32::WHITE),
                        )
                        .fill(theme::ACCENT_BLUE_BTN),
                    )
                    .on_hover_text("Load pool posts and start full-screen slideshow in reading order")
                    .clicked()
                {
                    if let Some(p) = active_pool.clone() {
                        actions.push(UiAction::PlayBankPoolSlideshow {
                            pool: p,
                            shuffle: false,
                        });
                    }
                }
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("Play Shuffle").strong().color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(0x7c, 0x2d, 0x12)),
                    )
                    .on_hover_text("Load pool posts and start full-screen slideshow shuffled")
                    .clicked()
                {
                    if let Some(p) = active_pool.clone() {
                        actions.push(UiAction::PlayBankPoolSlideshow {
                            pool: p,
                            shuffle: true,
                        });
                    }
                }
            });

            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("Download Pool").strong().color(theme::TEXT_SUCCESS))
                    .on_hover_text("Download the selected pool(s)")
                    .clicked()
                {
                    actions.push(UiAction::DownloadSelectedBankPools);
                }
                if ui
                    .button(
                        RichText::new("Download All Pools")
                            .strong()
                            .color(Color32::from_rgb(0x60, 0xa5, 0xfa)),
                    )
                    .on_hover_text("Download every pool saved in the Spank Bank")
                    .clicked()
                {
                    actions.push(UiAction::DownloadAllBankPools);
                }
            });

            ui.horizontal(|ui| {
                if ui.button("Remove selected").clicked() {
                    actions.push(UiAction::RemoveSelectedBankPools);
                }
                if ui.button("Clear all pools").clicked() {
                    actions.push(UiAction::ClearBankPoolsPrompt);
                }
            });
        });

    let q = filter.to_lowercase().trim().to_string();
    let filtered: Vec<&Pool> = bank_pools
        .iter()
        .filter(|pool| {
            q.is_empty()
                || pool.id.to_string().contains(&q)
                || pool.name.to_lowercase().contains(&q)
                || pool.category.to_lowercase().contains(&q)
        })
        .collect();

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show_inside(ui, |ui| {
            egui::Frame::group(ui.style())
                .fill(theme::BG_SURFACE)
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .id_salt("bank_pools_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for pool in &filtered {
                                let sel = selected_ids.contains(&pool.id);
                                let label = format!(
                                    "#{} · {} ({} posts)",
                                    pool.id,
                                    pool.display_name(),
                                    pool.post_count()
                                );
                                let resp = ui.selectable_label(sel, label).on_hover_text(format!(
                                    "Double-click to open pool #{}: {}",
                                    pool.id,
                                    pool.display_name()
                                ));
                                if resp.clicked() {
                                    let multi =
                                        ui.input(|i| i.modifiers.command || i.modifiers.shift);
                                    if multi {
                                        if sel {
                                            selected_ids.remove(&pool.id);
                                        } else {
                                            selected_ids.insert(pool.id);
                                        }
                                    } else {
                                        selected_ids.clear();
                                        selected_ids.insert(pool.id);
                                    }
                                    *last_id = Some(pool.id);
                                }
                                if resp.double_clicked() {
                                    selected_ids.clear();
                                    selected_ids.insert(pool.id);
                                    *last_id = Some(pool.id);
                                    actions.push(UiAction::OpenPool {
                                        pool: (*pool).clone(),
                                        new_tab: false,
                                    });
                                }
                            }
                        });
                });
        });
}

fn show_bank_posts_subtab(
    ui: &mut Ui,
    bank_posts: &[Post],
    filter: &mut String,
    selected_ids: &mut HashSet<u64>,
    actions: &mut Vec<UiAction>,
) {
    let cnt = bank_posts.len();
    egui::TopBottomPanel::top("bank_posts_top")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 2)))
        .show_inside(ui, |ui| {
            ui.label(
                RichText::new(format!(
                    "{cnt} saved post{}",
                    if cnt == 1 { "" } else { "s" }
                ))
                .strong()
                .size(13.0)
                .color(theme::ACCENT_AMBER),
            );

            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(filter)
                        .hint_text("Filter saved posts by tag or ID…")
                        .desired_width((ui.available_width() - 28.0).max(100.0)),
                );
                if !filter.is_empty() && ui.small_button("×").clicked() {
                    filter.clear();
                }
            });
        });

    egui::TopBottomPanel::bottom("bank_posts_bottom")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 4)))
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button(RichText::new("View Bank").strong()).clicked() {
                    actions.push(UiAction::OpenSpankBank { new_tab: false });
                }
                if ui.button("+ New Tab").clicked() {
                    actions.push(UiAction::OpenSpankBank { new_tab: true });
                }
            });

            ui.horizontal(|ui| {
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("Play In Order").strong().color(Color32::WHITE),
                        )
                        .fill(theme::ACCENT_BLUE_BTN),
                    )
                    .clicked()
                {
                    actions.push(UiAction::PlayBankSlideshow { shuffle: false });
                }
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("Play Shuffle").strong().color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(0x7c, 0x2d, 0x12)),
                    )
                    .clicked()
                {
                    actions.push(UiAction::PlayBankSlideshow { shuffle: true });
                }
            });

            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("Download Selected").strong().color(theme::TEXT_SUCCESS))
                    .on_hover_text("Download the selected post(s) from the Spank Bank")
                    .clicked()
                {
                    actions.push(UiAction::DownloadSelectedBankPosts);
                }
                if ui
                    .button(
                        RichText::new("Download All Posts")
                            .strong()
                            .color(Color32::from_rgb(0x60, 0xa5, 0xfa)),
                    )
                    .on_hover_text("Download all posts saved in the Spank Bank")
                    .clicked()
                {
                    actions.push(UiAction::DownloadAllBankPosts);
                }
            });

            ui.horizontal(|ui| {
                if ui.button("Remove selected").clicked() {
                    actions.push(UiAction::RemoveSelectedBankPosts);
                }
                if ui.button("Clear all").clicked() {
                    actions.push(UiAction::ClearBankPostsPrompt);
                }
            });
        });

    let q = filter.to_lowercase().trim().to_string();
    let filtered: Vec<&Post> = bank_posts
        .iter()
        .filter(|p| {
            if q.is_empty() || p.id.to_string().contains(&q) {
                return true;
            }
            let artists = p.artists().join(", ").to_lowercase();
            if artists.contains(&q) {
                return true;
            }
            p.tags
                .values()
                .any(|list| list.iter().any(|t| t.to_lowercase().contains(&q)))
        })
        .collect();

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show_inside(ui, |ui| {
            egui::Frame::group(ui.style())
                .fill(theme::BG_SURFACE)
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .id_salt("bank_posts_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for p in &filtered {
                                let sel = selected_ids.contains(&p.id);
                                let artists_list = p.artists();
                                let artists = if artists_list.is_empty() {
                                    "unknown".to_string()
                                } else {
                                    artists_list.join(", ")
                                };
                                let display = format!(
                                    "#{} · {} · Score {} · {}",
                                    p.id,
                                    artists,
                                    p.score,
                                    p.rating.to_uppercase()
                                );
                                let resp = ui
                                    .selectable_label(sel, display)
                                    .on_hover_text(format!("Double-click to open #{}", p.id));
                                if resp.clicked() {
                                    let multi =
                                        ui.input(|i| i.modifiers.command || i.modifiers.shift);
                                    if multi {
                                        if sel {
                                            selected_ids.remove(&p.id);
                                        } else {
                                            selected_ids.insert(p.id);
                                        }
                                    } else {
                                        selected_ids.clear();
                                        selected_ids.insert(p.id);
                                    }
                                }
                                if resp.double_clicked() {
                                    actions.push(UiAction::OpenViewer {
                                        post: (*p).clone(),
                                        posts: Some(bank_posts.to_vec()),
                                        start_slideshow: false,
                                        shuffle: false,
                                    });
                                }
                            }
                        });
                });
        });
}

fn show_bank_artists_subtab(
    ui: &mut Ui,
    bank_artists: &[String],
    filter: &mut String,
    add_edit: &mut String,
    selected_artists: &mut HashSet<String>,
    last_artist: &mut Option<String>,
    actions: &mut Vec<UiAction>,
) {
    let cnt = bank_artists.len();
    egui::TopBottomPanel::top("bank_artists_top")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 2)))
        .show_inside(ui, |ui| {
            ui.label(
                RichText::new(format!(
                    "{cnt} saved artist{}",
                    if cnt == 1 { "" } else { "s" }
                ))
                .strong()
                .size(13.0)
                .color(theme::ACCENT_AMBER),
            );

            ui.horizontal(|ui| {
                let resp = ui.add(
                    TextEdit::singleline(add_edit)
                        .hint_text("Add artist by name…")
                        .desired_width((ui.available_width() - 60.0).max(100.0)),
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    actions.push(UiAction::AddArtistToBankFromEdit);
                }
                if ui.button("+ Add").clicked() {
                    actions.push(UiAction::AddArtistToBankFromEdit);
                }
            });

            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(filter)
                        .hint_text("Filter saved artists…")
                        .desired_width((ui.available_width() - 28.0).max(100.0)),
                );
                if !filter.is_empty() && ui.small_button("×").clicked() {
                    filter.clear();
                }
            });
        });

    let active_artist = last_artist
        .as_ref()
        .filter(|a| bank_artists.contains(a))
        .cloned()
        .or_else(|| selected_artists.iter().next().cloned());

    egui::TopBottomPanel::bottom("bank_artists_bottom")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 4)))
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button(RichText::new("View Artist").strong()).clicked() {
                    if let Some(a) = active_artist.clone() {
                        actions.push(UiAction::OpenArtist {
                            name: a,
                            new_tab: false,
                        });
                    }
                }
                if ui.button("+ New Tab").clicked() {
                    if let Some(a) = active_artist.clone() {
                        actions.push(UiAction::OpenArtist {
                            name: a,
                            new_tab: true,
                        });
                    }
                }
                if ui
                    .button("e621 Page")
                    .on_hover_text("Open the artist's page on e621")
                    .clicked()
                {
                    if let Some(a) = active_artist.clone() {
                        actions.push(UiAction::OpenUrl(artist_page_url(&a)));
                    }
                }
            });

            if ui
                .add_sized(
                    [ui.available_width(), 24.0],
                    egui::Button::new(
                        RichText::new("Latest From All Saved Artists")
                            .strong()
                            .color(theme::ACCENT_AMBER),
                    ),
                )
                .on_hover_text("One combined feed of the newest posts by every saved artist")
                .clicked()
            {
                actions.push(UiAction::OpenSavedArtistsFeed { new_tab: false });
            }

            ui.horizontal(|ui| {
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("Play Newest").strong().color(Color32::WHITE),
                        )
                        .fill(theme::ACCENT_BLUE_BTN),
                    )
                    .on_hover_text("Slideshow of the selected artist's newest posts")
                    .clicked()
                {
                    if let Some(a) = active_artist.clone() {
                        actions.push(UiAction::PlayArtistSlideshow {
                            name: a,
                            shuffle: false,
                        });
                    }
                }
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("Play Shuffle").strong().color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(0x7c, 0x2d, 0x12)),
                    )
                    .on_hover_text("Shuffled slideshow of the selected artist's posts")
                    .clicked()
                {
                    if let Some(a) = active_artist.clone() {
                        actions.push(UiAction::PlayArtistSlideshow {
                            name: a,
                            shuffle: true,
                        });
                    }
                }
            });

            ui.horizontal(|ui| {
                if ui.button("Remove selected").clicked() {
                    actions.push(UiAction::RemoveSelectedBankArtists);
                }
                if ui.button("Clear all artists").clicked() {
                    actions.push(UiAction::ClearBankArtistsPrompt);
                }
            });
        });

    let q = filter.to_lowercase().trim().replace(' ', "_");
    let filtered: Vec<&String> = bank_artists
        .iter()
        .filter(|name| q.is_empty() || name.contains(&q))
        .collect();

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show_inside(ui, |ui| {
            egui::Frame::group(ui.style())
                .fill(theme::BG_SURFACE)
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .id_salt("bank_artists_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for name in &filtered {
                                let sel = selected_artists.contains(*name);
                                let resp = ui
                                    .selectable_label(sel, name.as_str())
                                    .on_hover_text(format!(
                                        "Double-click to view all posts by {name}"
                                    ));
                                if resp.clicked() {
                                    let multi =
                                        ui.input(|i| i.modifiers.command || i.modifiers.shift);
                                    if multi {
                                        if sel {
                                            selected_artists.remove(*name);
                                        } else {
                                            selected_artists.insert((*name).clone());
                                        }
                                    } else {
                                        selected_artists.clear();
                                        selected_artists.insert((*name).clone());
                                    }
                                    *last_artist = Some((*name).clone());
                                }
                                if resp.double_clicked() {
                                    selected_artists.clear();
                                    selected_artists.insert((*name).clone());
                                    *last_artist = Some((*name).clone());
                                    actions.push(UiAction::OpenArtist {
                                        name: (*name).clone(),
                                        new_tab: false,
                                    });
                                }
                            }
                        });
                });
        });
}
