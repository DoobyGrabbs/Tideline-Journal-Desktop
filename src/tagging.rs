//! Tags (`#work`) in the diary: counting them for the sidebar, the search filter and the suggestions while typing.
//! The rules for what a tag is, and which paragraph it marks, are in `tideline_core::tags`.

use super::*;
use tideline_core::tags;

/// The tags in `entries` with how many entries use each (most used first, then by name). A tag in a paragraph with no
/// text to mark doesn't count. Private entries are left out when `hide_private` is on.
pub(crate) fn count_tags(entries: &[Entry], hide_private: bool) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for e in entries {
        if hide_private && e.sensitive {
            continue;
        }
        let mut seen: Vec<String> = Vec::new();
        for section in tags::tag_sections(&e.text) {
            for t in section.tags {
                if !seen.contains(&t) {
                    seen.push(t);
                }
            }
        }
        for t in seen {
            *counts.entry(t).or_insert(0) += 1;
        }
    }
    let mut list: Vec<(String, usize)> = counts.into_iter().collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    list
}

impl DiaryApp {
    /// The tags in the diary (see `count_tags`), worked out again only when the diary or the setting changes.
    pub(crate) fn tag_counts(&mut self) -> Vec<(String, usize)> {
        let hide = self.data.settings.hide_private_tags;
        if self.tag_cache.as_ref().is_none_or(|(rev, h, _)| *rev != self.rev || *h != hide) {
            self.tag_cache = Some((self.rev, hide, count_tags(&self.data.entries, hide)));
        }
        self.tag_cache.as_ref().map(|c| c.2.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u64, text: &str, private: bool) -> Entry {
        Entry { id, date: "2026-10-09".into(), added_at: format!("2026-10-09 0{id}:00:00"), category: "Personal".into(), text: text.into(), sensitive: private, ..Default::default() }
    }

    #[test]
    fn tags_are_counted_per_entry_and_private_ones_can_be_left_out() {
        let entries = vec![
            entry(1, "#work Good day\n\nUnrelated.", false),
            entry(2, "Home #family and #work, and again #work", false),
            entry(3, "#secret hidden", true),
            entry(4, "#lonely", false),
        ];
        assert_eq!(count_tags(&entries, true), [("work".to_string(), 2), ("family".to_string(), 1)]);
        let all = count_tags(&entries, false);
        assert_eq!(all.iter().map(|t| t.0.as_str()).collect::<Vec<_>>(), ["work", "family", "secret"]);
    }
}
