use super::*;
use std::cell::{Cell, RefCell};

pub(super) struct ChatScroll {
    pub state: ListState,
    identity: RefCell<(String, Vec<(String, usize)>)>,
    bottom_requested: Cell<bool>,
}

impl ChatScroll {
    pub fn new() -> Self {
        Self {
            state: ListState::new(0, ListAlignment::Top, px(400.)),
            identity: RefCell::new(Default::default()),
            bottom_requested: Cell::new(false),
        }
    }
    pub fn offset(&self) -> Point<Pixels> {
        self.state.scroll_px_offset_for_scrollbar()
    }
    pub fn max_offset(&self) -> Point<Pixels> {
        self.state.max_offset_for_scrollbar()
    }
    #[cfg(feature = "visual-test")]
    pub fn set_offset(&self, offset: Point<Pixels>) {
        self.bottom_requested.set(false);
        self.state.set_offset_from_scrollbar(offset);
    }
    pub fn scroll_to_bottom(&self) {
        self.bottom_requested.set(true);
        self.state.set_follow_mode(FollowMode::Tail);
        self.state.scroll_to_end();
    }
    pub fn sync(&self, conversation: &Conversation, expanded: &HashSet<String>) {
        let mut previous = self.identity.borrow_mut();
        let rows: Vec<_> = conversation
            .messages
            .iter()
            .map(|m| {
                let tool_size: usize = m
                    .tools
                    .iter()
                    .map(|t| {
                        t.arguments.len()
                            + t.result.as_ref().map_or(0, String::len)
                            + usize::from(expanded.contains(&format!("tool-{}-{}", m.id, t.id)))
                    })
                    .sum();
                (
                    m.id.as_str(),
                    m.content.len()
                        + m.reasoning.len()
                        + m.context_note.len()
                        + m.error.as_ref().map_or(0, String::len)
                        + m.attachments.len()
                        + tool_size
                        + usize::from(expanded.contains(&m.id)),
                )
            })
            .collect();
        let common = if previous.0 == conversation.id {
            previous
                .1
                .iter()
                .zip(&rows)
                .take_while(|(a, b)| a.0 == b.0)
                .count()
        } else {
            0
        };
        if common < previous.1.len() || common < rows.len() {
            self.state
                .splice(common..previous.1.len(), rows.len() - common);
            if common > 0 {
                self.state.remeasure_items(common - 1..common);
            }
        }
        for (i, (a, b)) in previous.1.iter().zip(&rows).take(common).enumerate() {
            if a.1 != b.1 {
                self.state.remeasure_items(i..i + 1);
            }
        }
        for (old, new) in previous.1.iter_mut().zip(&rows).take(common) {
            old.1 = new.1;
        }
        previous.1.truncate(common);
        previous.1.extend(
            rows[common..]
                .iter()
                .map(|(id, size)| ((*id).to_owned(), *size)),
        );
        if previous.0 != conversation.id {
            previous.0.clone_from(&conversation.id);
        }
        if self.bottom_requested.replace(false) {
            self.state.set_follow_mode(FollowMode::Tail);
            self.state.scroll_to_end();
        }
    }
}

#[derive(Default)]
pub(super) struct TextCache {
    entries: std::collections::HashMap<String, (SharedString, SharedString)>,
    bytes: usize,
}
impl TextCache {
    pub fn get(
        &mut self,
        key: String,
        source: &str,
        prepare: impl FnOnce(&str) -> String,
    ) -> (SharedString, SharedString) {
        if let Some((original, prepared)) = self.entries.get(&key)
            && original.as_ref() == source
        {
            return (original.clone(), prepared.clone());
        }
        let original: SharedString = source.to_owned().into();
        let prepared = prepare(source);
        let prepared: SharedString = if prepared == source {
            original.clone()
        } else {
            prepared.into()
        };
        let bytes = original.len() + prepared.len();
        if self.bytes + bytes > 16 * 1024 * 1024 || self.entries.len() >= 256 {
            self.entries.clear();
            self.bytes = 0;
        }
        if let Some((a, b)) = self
            .entries
            .insert(key, (original.clone(), prepared.clone()))
        {
            self.bytes -= a.len() + b.len();
        }
        self.bytes += bytes;
        (original, prepared)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test]
    fn cached_text_keeps_shared_storage_and_invalidates_equal_length_edits() {
        let mut cache = TextCache::default();
        let original = "你好\n世界".repeat(1000);
        let (_, first) = cache.get("message".into(), &original, crate::math::normalize);
        let (_, second) = cache.get("message".into(), &original, |_| {
            panic!("unchanged text must not be normalized again")
        });
        assert_eq!(first.as_ptr(), second.as_ptr());
        let replacement = "你好\n朋友".repeat(1000);
        let (_, edited) = cache.get("message".into(), &replacement, crate::math::normalize);
        assert_eq!(edited.as_ref(), replacement);
    }
}
