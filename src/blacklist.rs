//! Client-side blacklist using e621's syntax.
//!
//! One rule per line. A post is hidden when a line matches:
//! * every plain tag on the line is present
//! * no `-tag` on the line is present
//! * at least one `~tag` is present (if the line has any)
//!
//! Supported metatags: `rating:s|q|e` (or safe/questionable/explicit),
//! `type:<ext>`, `id:<n>`. Lines starting with `#` are comments.

use std::collections::HashSet;

use crate::api::Post;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub text: String,
    pub required: Vec<String>,
    pub excluded: Vec<String>,
    pub any_of: Vec<String>,
}

fn has_tag(tag: &str, tags: &HashSet<String>, post: &Post) -> bool {
    if let Some(rest) = tag.strip_prefix("rating:") {
        let mut r_chars = rest.chars();
        let mut p_chars = post.rating.chars();
        return match (r_chars.next(), p_chars.next()) {
            (Some(rc), Some(pc)) => rc.eq_ignore_ascii_case(&pc),
            _ => false,
        };
    }
    if let Some(ext) = tag.strip_prefix("type:") {
        return post.ext == ext;
    }
    if let Some(id_str) = tag.strip_prefix("id:") {
        return post.id.to_string() == id_str;
    }
    tags.contains(tag)
}

#[derive(Debug, Clone, Default)]
pub struct Blacklist {
    pub rules: Vec<Rule>,
}

impl Blacklist {
    pub fn new(text: &str) -> Self {
        let mut rules = Vec::new();
        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let lower = line.to_lowercase();
            let mut required = Vec::new();
            let mut excluded = Vec::new();
            let mut any_of = Vec::new();

            for tok in lower.split_whitespace() {
                if let Some(stripped) = tok.strip_prefix('-') {
                    if !stripped.is_empty() {
                        excluded.push(stripped.to_string());
                    }
                } else if let Some(stripped) = tok.strip_prefix('~') {
                    if !stripped.is_empty() {
                        any_of.push(stripped.to_string());
                    }
                } else {
                    required.push(tok.to_string());
                }
            }

            if required.is_empty() && any_of.is_empty() {
                continue;
            }
            rules.push(Rule {
                text: line.to_string(),
                required,
                excluded,
                any_of,
            });
        }
        Self { rules }
    }

    pub fn match_post(&self, post: &Post) -> Option<&Rule> {
        let tags = post.all_tags();
        for r in &self.rules {
            let req_ok = r.required.iter().all(|t| has_tag(t, &tags, post));
            let exc_ok = !r.excluded.iter().any(|t| has_tag(t, &tags, post));
            let any_ok = r.any_of.is_empty() || r.any_of.iter().any(|t| has_tag(t, &tags, post));
            if req_ok && exc_ok && any_ok {
                return Some(r);
            }
        }
        None
    }

    pub fn is_blocked(&self, post: &Post) -> bool {
        self.match_post(post).is_some()
    }

    pub fn filter(&self, posts: &[Post]) -> (Vec<Post>, usize) {
        let mut kept = Vec::with_capacity(posts.len());
        let mut hidden = 0;
        for p in posts {
            if self.is_blocked(p) {
                hidden += 1;
            } else {
                kept.push(p.clone());
            }
        }
        (kept, hidden)
    }
}
