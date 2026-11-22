//! Pre-seeded e621 tags, category colors, and cursor-aware tag autocomplete state.

use std::collections::HashMap;
use std::time::Instant;

use egui::Color32;

use crate::api::{tag_category_id, Post, Tag};

pub fn category_color(category: u8) -> Color32 {
    match category {
        0 => Color32::from_rgb(0x5b, 0x9b, 0xd5), // general
        1 => Color32::from_rgb(0xe6, 0xa0, 0x00), // artist
        3 => Color32::from_rgb(0xc2, 0x3b, 0xc2), // copyright
        4 => Color32::from_rgb(0x2e, 0x9e, 0x2e), // character
        5 => Color32::from_rgb(0xe0, 0x60, 0x1f), // species
        6 => Color32::from_rgb(0xdd, 0x33, 0x33), // invalid
        7 => Color32::from_rgb(0x88, 0x88, 0x88), // meta
        8 => Color32::from_rgb(0x2a, 0x7a, 0x2a), // lore
        _ => Color32::from_rgb(0x5b, 0x9b, 0xd5),
    }
}

pub fn rating_color(rating: &str) -> Color32 {
    match rating {
        "s" => Color32::from_rgb(0x3a, 0x9e, 0x3a),
        "q" => Color32::from_rgb(0xd9, 0xa4, 0x00),
        "e" => Color32::from_rgb(0xd2, 0x3b, 0x3b),
        _ => Color32::from_rgb(0x66, 0x66, 0x66),
    }
}

pub fn seed_tags() -> Vec<Tag> {
    vec![
        // General (0)
        Tag::new("solo", 0, 4_200_000),
        Tag::new("male", 0, 3_100_000),
        Tag::new("female", 0, 2_900_000),
        Tag::new("anthro", 0, 3_500_000),
        Tag::new("feral", 0, 1_100_000),
        Tag::new("duo", 0, 1_500_000),
        Tag::new("fur", 0, 2_400_000),
        Tag::new("tail", 0, 2_100_000),
        Tag::new("claws", 0, 800_000),
        Tag::new("fluffy", 0, 450_000),
        Tag::new("fluffy_tail", 0, 300_000),
        Tag::new("digital_media_(artwork)", 0, 3_200_000),
        Tag::new("hi_res", 0, 2_800_000),
        Tag::new("simple_background", 0, 1_800_000),
        Tag::new("detailed_background", 0, 750_000),
        Tag::new("looking_at_viewer", 0, 1_600_000),
        Tag::new("smile", 0, 1_200_000),
        Tag::new("open_mouth", 0, 1_000_000),
        Tag::new("clothing", 0, 1_400_000),
        Tag::new("nude", 0, 1_300_000),
        Tag::new("erection", 0, 950_000),
        Tag::new("penis", 0, 1_600_000),
        Tag::new("breasts", 0, 1_300_000),
        Tag::new("balls", 0, 1_100_000),
        Tag::new("butt", 0, 1_200_000),
        Tag::new("pussy", 0, 750_000),
        Tag::new("anus", 0, 850_000),
        Tag::new("canine_penis", 0, 210_000),
        Tag::new("knot", 0, 450_000),
        Tag::new("muscular", 0, 520_000),
        Tag::new("spread_legs", 0, 430_000),
        Tag::new("cum", 0, 980_000),
        Tag::new("order:score", 0, 0),
        Tag::new("order:favcount", 0, 0),
        Tag::new("order:id_desc", 0, 0),
        Tag::new("comic", 0, 400_000),
        // Species (5)
        Tag::new("canine", 5, 1_700_000),
        Tag::new("wolf", 5, 430_000),
        Tag::new("fox", 5, 540_000),
        Tag::new("dog", 5, 380_000),
        Tag::new("feline", 5, 1_200_000),
        Tag::new("cat", 5, 360_000),
        Tag::new("dragon", 5, 480_000),
        Tag::new("mammal", 5, 2_900_000),
        Tag::new("hybrid", 5, 220_000),
        Tag::new("pokemon_(species)", 5, 800_000),
        Tag::new("rabbit", 5, 170_000),
        Tag::new("hare", 5, 60_000),
        Tag::new("equine", 5, 290_000),
        Tag::new("horse", 5, 190_000),
        Tag::new("deer", 5, 120_000),
        Tag::new("bovine", 5, 150_000),
        Tag::new("otter", 5, 110_000),
        Tag::new("mustelid", 5, 180_000),
        Tag::new("bear", 5, 110_000),
        Tag::new("ursine", 5, 130_000),
        Tag::new("raccoon", 5, 75_000),
        Tag::new("procyonid", 5, 85_000),
        Tag::new("shark", 5, 85_000),
        Tag::new("reptile", 5, 450_000),
        Tag::new("scalie", 5, 350_000),
        Tag::new("avian", 5, 140_000),
        Tag::new("bird", 5, 110_000),
        Tag::new("hyena", 5, 100_000),
        Tag::new("rodent", 5, 160_000),
        Tag::new("mouse", 5, 80_000),
        Tag::new("tiger", 5, 120_000),
        Tag::new("lion", 5, 130_000),
        Tag::new("leopard", 5, 70_000),
        Tag::new("cheetah", 5, 55_000),
        Tag::new("big_cat", 5, 160_000),
        // Copyright (3)
        Tag::new("pokemon", 3, 900_000),
        Tag::new("nintendo", 3, 1_100_000),
        Tag::new("digimon", 3, 180_000),
        Tag::new("disney", 3, 150_000),
        Tag::new("my_little_pony", 3, 160_000),
        Tag::new("hasbro", 3, 170_000),
        Tag::new("sega", 3, 140_000),
        Tag::new("sonic_the_hedgehog", 3, 130_000),
        Tag::new("capcom", 3, 80_000),
        Tag::new("zootopia", 3, 75_000),
        Tag::new("star_fox", 3, 70_000),
        // Character (4)
        Tag::new("lucario", 4, 95_000),
        Tag::new("renamon", 4, 45_000),
        Tag::new("judy_hopps", 4, 30_000),
        Tag::new("nick_wilde", 4, 35_000),
        Tag::new("krystal_(star_fox)", 4, 25_000),
        Tag::new("wolf_o'donnell", 4, 8_000),
        Tag::new("tails_(sonic)", 4, 25_000),
        Tag::new("sonic_the_hedgehog_(character)", 4, 30_000),
        // Artist (1)
        Tag::new("conditional_dnp", 1, 150_000),
        Tag::new("unknown_artist", 1, 120_000),
    ]
}

#[derive(Debug, Clone)]
pub struct TagAutocompleteState {
    pub known_tags: HashMap<String, Tag>,
    pub popup_open: bool,
    pub selected_row: usize,
    pub suggestions: Vec<Tag>,
    pub debounce_token: Option<String>,
    pub debounce_since: Option<Instant>,
    pub last_requested_token: Option<String>,
    pub last_cursor_char: Option<usize>,
}

impl Default for TagAutocompleteState {
    fn default() -> Self {
        Self::new()
    }
}

impl TagAutocompleteState {
    pub fn new() -> Self {
        let known_tags = seed_tags()
            .into_iter()
            .map(|t| (t.name.clone(), t))
            .collect();
        Self {
            known_tags,
            popup_open: false,
            selected_row: 0,
            suggestions: Vec::new(),
            debounce_token: None,
            debounce_since: None,
            last_requested_token: None,
            last_cursor_char: None,
        }
    }

    pub fn learn_tags(&mut self, tags: &[Tag]) {
        for t in tags {
            self.known_tags.insert(t.name.clone(), t.clone());
        }
    }

    pub fn learn_post_tags(&mut self, post: &Post) {
        for (cat_name, names) in &post.tags {
            let cat_id = tag_category_id(cat_name);
            for name in names {
                self.known_tags
                    .entry(name.clone())
                    .or_insert_with(|| Tag::new(name.clone(), cat_id, 0));
            }
        }
    }

    pub fn current_token(text: &str, cursor_char: usize) -> String {
        let byte_idx = char_to_byte_index(text, cursor_char);
        let before = &text[..byte_idx];
        let tok = before.rsplit(' ').next().unwrap_or("");
        tok.trim_start_matches(['-', '~']).to_lowercase()
    }

    pub fn local_matches(&self, prefix: &str) -> Vec<Tag> {
        let mut starts: Vec<Tag> = self
            .known_tags
            .values()
            .filter(|t| t.name.starts_with(prefix))
            .cloned()
            .collect();
        starts.sort_by(|a, b| {
            b.post_count
                .cmp(&a.post_count)
                .then_with(|| a.name.cmp(&b.name))
        });
        starts.truncate(15);
        starts
    }

    pub fn on_text_edited(&mut self, text: &str, cursor_char: usize, now: Instant) {
        self.last_cursor_char = Some(cursor_char);
        let tok = Self::current_token(text, cursor_char);
        if tok.chars().count() >= 2 && !tok.contains(':') {
            let matches = self.local_matches(&tok);
            self.suggestions = matches;
            self.popup_open = !self.suggestions.is_empty();
            self.selected_row = 0;
            self.debounce_token = Some(tok);
            self.debounce_since = Some(now);
        } else {
            self.popup_open = false;
            self.suggestions.clear();
            self.debounce_token = None;
            self.debounce_since = None;
        }
    }

    /// Returns a token if the 150ms debounce elapsed and an API autocomplete call should run.
    pub fn take_due_remote_query(&mut self, text: &str, cursor_char: usize, now: Instant) -> Option<String> {
        let since = self.debounce_since?;
        if now.duration_since(since).as_millis() < 150 {
            return None;
        }
        let tok = Self::current_token(text, cursor_char);
        self.debounce_since = None;
        self.debounce_token = None;
        if tok.chars().count() >= 2 && !tok.contains(':') {
            if self.last_requested_token.as_deref() == Some(tok.as_str()) {
                return None;
            }
            self.last_requested_token = Some(tok.clone());
            Some(tok)
        } else {
            None
        }
    }

    pub fn set_suggestions(&mut self, tags: &[Tag], text: &str, cursor_char: usize) {
        self.learn_tags(tags);
        let cur = Self::current_token(text, cursor_char);
        if cur.chars().count() < 2 || cur.contains(':') {
            self.popup_open = false;
            self.suggestions.clear();
            return;
        }
        let matches = self.local_matches(&cur);
        if matches.is_empty() {
            self.popup_open = false;
            self.suggestions.clear();
        } else {
            self.suggestions = matches;
            if self.selected_row >= self.suggestions.len() {
                self.selected_row = 0;
            }
            self.popup_open = true;
        }
    }

    pub fn insert_tag(text: &str, cursor_char: usize, name: &str, add_space: bool) -> (String, usize) {
        let byte_pos = char_to_byte_index(text, cursor_char);
        let before = &text[..byte_pos];
        let after = &text[byte_pos..];

        let start = before.rfind(' ').map(|i| i + 1).unwrap_or(0);
        let tok = &before[start..];
        let prefix = if tok.starts_with('-') {
            "-"
        } else if tok.starts_with('~') {
            "~"
        } else {
            ""
        };

        let end_len = after
            .char_indices()
            .find(|(_, c)| c.is_whitespace())
            .map(|(i, _)| i)
            .unwrap_or(after.len());
        let suffix = if add_space { " " } else { "" };
        let new_before = format!("{}{}{}{}", &before[..start], prefix, name, suffix);
        let new_after = after[end_len..].trim_start();
        let new_cursor = new_before.chars().count();
        (format!("{new_before}{new_after}"), new_cursor)
    }
}

pub fn char_to_byte_index(s: &str, char_idx: usize) -> usize {
    if char_idx == 0 {
        return 0;
    }
    s.char_indices()
        .nth(char_idx)
        .map(|(byte_i, _)| byte_i)
        .unwrap_or(s.len())
}

pub fn format_post_count(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (idx, ch) in s.chars().rev().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}
