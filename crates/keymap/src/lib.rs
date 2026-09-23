//! Scoped key sequences. Callers supply focus ancestry and a monotonic clock.
#![forbid(unsafe_code)]
use std::time::Duration;
use wove::{Id, Key, Modifiers};

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
///
/// `Flushed` reports commands that became final with this stroke, because a
/// waiting sequence expired or the stroke did not continue it. Execute them in
/// order, then handle the stroke's own result, which is never another `Flushed`.
#[derive(Debug, PartialEq, Eq)]
pub enum Match<C> {
    Command(C),
    Pending,
    Unbound,
    Flushed(Vec<C>, Box<Match<C>>),
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
    /// The longest exact binding within `pending` and its length in strokes.
    deferred: Option<(C, usize)>,
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
    /// Resolve a sequence whose deadline has passed and return the commands
    /// that became final, in order. The longest exact binding in the sequence
    /// runs and the strokes after it are replayed, which may leave a new
    /// pending sequence. Call it when the deadline arrives without a new key.
    pub fn expire(&mut self, now: Duration) -> Vec<C> {
        let mut flushed = Vec::new();
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            match self.deferred.take() {
                Some((command, len)) => {
                    flushed.push(command);
                    let scopes = self.scopes.clone();
                    if let Match::Command(command) = self.replay(len, &scopes, now, &mut flushed) {
                        flushed.push(command);
                    }
                }
                None => self.clear(),
            }
        }
        flushed
    }
    /// Feed a stroke, clearing a sequence from another focus before resolving
    /// an expired one. Pass scopes from the focused element to root; call
    /// `clear` when focus changes without a subsequent key event. When a stroke
    /// does not continue the sequence, its longest exact binding runs and the
    /// later strokes are replayed; without one, the stroke is retried alone.
    pub fn feed(&mut self, stroke: Stroke, scopes: &[Id], now: Duration) -> Match<C> {
        if self.scopes != scopes {
            self.clear();
            self.scopes = scopes.to_vec();
        }
        let mut flushed = self.expire(now);
        let result = self.step(stroke, scopes, now, &mut flushed);
        if flushed.is_empty() {
            result
        } else {
            Match::Flushed(flushed, Box::new(result))
        }
    }
    /// Add a stroke to the sequence, collecting commands made final by a dead
    /// end into `flushed`, and return the stroke's own result.
    fn step(
        &mut self,
        stroke: Stroke,
        scopes: &[Id],
        now: Duration,
        flushed: &mut Vec<C>,
    ) -> Match<C> {
        self.pending.push(stroke);
        if let Some(result) = self.resolve(scopes, now) {
            return result;
        }
        if self.pending.len() == 1 {
            self.clear();
            return Match::Unbound;
        }
        let keep = match self.deferred.take() {
            Some((command, len)) => {
                flushed.push(command);
                len
            }
            None => self.pending.len() - 1,
        };
        self.replay(keep, scopes, now, flushed)
    }
    /// Restart the sequence from the strokes after the first `keep` and return
    /// the last stroke's result; earlier completed commands go into `flushed`.
    fn replay(
        &mut self,
        keep: usize,
        scopes: &[Id],
        now: Duration,
        flushed: &mut Vec<C>,
    ) -> Match<C> {
        let rest = self.pending.split_off(keep);
        self.clear();
        let mut result = Match::Unbound;
        for stroke in rest {
            if let Match::Command(command) = result {
                flushed.push(command);
            }
            result = self.step(stroke, scopes, now, flushed);
        }
        result
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
                if let Some(command) = exact {
                    self.deferred = Some((command, self.pending.len()));
                }
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
