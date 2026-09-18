//! Scoped key sequences. Callers supply focus ancestry and a monotonic clock.
use crate::{Id, Key, Modifiers};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stroke {
    pub key: Key,
    pub modifiers: Modifiers,
}
impl From<Key> for Stroke {
    fn from(key: Key) -> Self {
        Self {
            key,
            modifiers: Modifiers::default(),
        }
    }
}

/// A complete command, an incomplete sequence, or an unbound key.
#[derive(Debug, PartialEq, Eq)]
pub enum Match<C> {
    Command(C),
    Pending,
    Unbound,
}
struct Binding<C> {
    scope: Option<Id>,
    keys: Vec<Stroke>,
    command: C,
}

/// Focused scopes take precedence over ancestors and global bindings. Within a
/// scope, newer bindings win. Changing focus clears an incomplete sequence.
pub struct Keymap<C> {
    bindings: Vec<Binding<C>>,
    pending: Vec<Stroke>,
    scopes: Vec<Id>,
    deadline: Option<Duration>,
    deferred: Option<C>,
    timeout: Duration,
}
impl<C: Clone> Keymap<C> {
    pub fn new(timeout: Duration) -> Self {
        Self {
            bindings: Vec::new(),
            pending: Vec::new(),
            scopes: Vec::new(),
            deadline: None,
            deferred: None,
            timeout,
        }
    }
    /// Empty sequences are rejected. A missing scope creates a global binding.
    pub fn bind(
        &mut self,
        scope: Option<Id>,
        keys: impl IntoIterator<Item = Stroke>,
        command: C,
    ) -> bool {
        let keys: Vec<_> = keys.into_iter().collect();
        if keys.is_empty() {
            return false;
        }
        self.bindings.push(Binding {
            scope,
            keys,
            command,
        });
        self.clear();
        true
    }
    /// Remove bindings when their owning element is destroyed.
    pub fn remove(&mut self, scope: Id) {
        self.bindings.retain(|b| b.scope != Some(scope));
        self.clear();
    }
    pub fn clear(&mut self) {
        self.pending.clear();
        self.deadline = None;
        self.deferred = None;
    }
    pub fn deadline(&self) -> Option<Duration> {
        self.deadline
    }
    /// Resolve an exact binding that was waiting for a longer sequence.
    pub fn expire(&mut self, now: Duration) -> Option<C> {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            let result = self.deferred.take();
            self.clear();
            result
        } else {
            None
        }
    }
    /// Feed a stroke after checking `expire`. Pass scopes from focused element to
    /// root; call `clear` when focus changes without a subsequent key event.
    pub fn feed(&mut self, stroke: Stroke, scopes: &[Id], now: Duration) -> Match<C> {
        if self.scopes != scopes {
            self.clear();
            self.scopes = scopes.to_vec();
        }
        self.pending.push(stroke);
        let result = self.resolve(scopes, now);
        if result.is_none() && self.pending.len() > 1 {
            self.clear();
            self.pending.push(stroke);
            if let Some(result) = self.resolve(scopes, now) {
                return result;
            }
        } else if let Some(result) = result {
            return result;
        }
        self.clear();
        Match::Unbound
    }
    fn resolve(&mut self, scopes: &[Id], now: Duration) -> Option<Match<C>> {
        for scope in scopes
            .iter()
            .copied()
            .map(Some)
            .chain(std::iter::once(None))
        {
            let candidates: Vec<_> = self
                .bindings
                .iter()
                .rev()
                .filter(|b| b.scope == scope && b.keys.starts_with(&self.pending))
                .collect();
            if candidates.is_empty() {
                continue;
            }
            let exact = candidates
                .iter()
                .find(|b| b.keys.len() == self.pending.len())
                .map(|b| b.command.clone());
            if candidates.iter().any(|b| b.keys.len() > self.pending.len()) {
                self.deferred = exact;
                self.deadline = Some(now.saturating_add(self.timeout));
                return Some(Match::Pending);
            }
            if let Some(command) = exact {
                self.clear();
                return Some(Match::Command(command));
            }
        }
        None
    }
}
