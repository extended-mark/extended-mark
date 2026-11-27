//! What a compile shares with the core it is made on: cloning a compiled core copies none of it.

use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::{Index, IndexMut};
use std::rc::Rc;

use crate::syntax::span::Span;

use crate::check::Found;

/// A list whose items are shared until one is changed.
#[derive(Debug)]
pub struct Shared<T>(Vec<Rc<T>>);

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}

impl<T> Default for Shared<T> {
    fn default() -> Self {
        Shared(Vec::new())
    }
}

impl<T> Shared<T> {
    pub fn push(&mut self, x: T) {
        self.0.push(Rc::new(x));
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator + Clone {
        self.0.iter().map(|x| &**x)
    }
}

impl<T> Index<usize> for Shared<T> {
    type Output = T;
    fn index(&self, k: usize) -> &T {
        &self.0[k]
    }
}

impl<T: Clone> IndexMut<usize> for Shared<T> {
    fn index_mut(&mut self, k: usize) -> &mut T {
        Rc::make_mut(&mut self.0[k])
    }
}

/// A map over the core's, which stays as it is: what this compile puts in is found first.
#[derive(Debug)]
pub struct Over<K, V> {
    under: Rc<HashMap<K, V>>,
    own: HashMap<K, V>,
}

impl<K, V: Clone> Clone for Over<K, V>
where
    K: Clone,
{
    fn clone(&self) -> Self {
        Over { under: self.under.clone(), own: self.own.clone() }
    }
}

impl<K, V> Default for Over<K, V> {
    fn default() -> Self {
        Over { under: Rc::new(HashMap::new()), own: HashMap::new() }
    }
}

impl<K: Eq + Hash + Clone, V: Clone> Over<K, V> {
    pub fn get<Q: Eq + Hash + ?Sized>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.own.get(k).or_else(|| self.under.get(k))
    }
    pub fn insert(&mut self, k: K, v: V) {
        self.own.insert(k, v);
    }
    /// What is there to change, the core's copied first.
    pub fn entry_or_default(&mut self, k: K) -> &mut V
    where
        V: Default,
    {
        let under = &self.under;
        self.own.entry(k).or_insert_with_key(|k| under.get(k).cloned().unwrap_or_default())
    }
    /// What this compile put in.
    pub fn own(&self) -> &HashMap<K, V> {
        &self.own
    }
    pub fn own_mut(&mut self) -> &mut HashMap<K, V> {
        &mut self.own
    }
    /// Everything in it becomes what the next compile is made over.
    pub fn settle(&mut self) {
        if self.own.is_empty() {
            return;
        }
        let own = std::mem::take(&mut self.own);
        if self.under.is_empty() {
            self.under = Rc::new(own);
        } else {
            Rc::make_mut(&mut self.under).extend(own);
        }
    }
}

/// The diagnostics found, each said once, found again by where they are.
#[derive(Clone, Default)]
pub struct Diags {
    list: Vec<Found>,
    at: HashMap<(&'static str, Span), Vec<usize>>,
}

impl Diags {
    /// Adds one, unless it has been said.
    pub fn once(&mut self, f: Found) {
        let d = &f.diagnostic;
        let there = self.at.get(&(d.code, d.span));
        if !there.map_or(false, |ks| ks.iter().any(|k| self.list[*k].file == f.file && self.list[*k].diagnostic.message == d.message)) {
            self.push(f);
        }
    }
    pub fn push(&mut self, f: Found) {
        self.at.entry((f.diagnostic.code, f.diagnostic.span)).or_default().push(self.list.len());
        self.list.push(f);
    }
    /// Forgets those after the first `n`.
    pub fn truncate(&mut self, n: usize) {
        for f in self.list.drain(n.min(self.list.len())..) {
            if let Some(ks) = self.at.get_mut(&(f.diagnostic.code, f.diagnostic.span)) {
                ks.pop();
            }
        }
    }
    pub fn take(&mut self) -> Vec<Found> {
        self.at.clear();
        std::mem::take(&mut self.list)
    }
}

impl std::ops::Deref for Diags {
    type Target = [Found];
    fn deref(&self) -> &[Found] {
        &self.list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::diag::Diagnostic;

    fn found(file: &str, message: &str) -> Found {
        Found { file: file.into(), diagnostic: Diagnostic::error("bad", Span { start: 1, end: 2 }, message) }
    }

    #[test]
    fn a_diagnostic_is_said_once_and_again_after_it_is_forgotten() {
        let mut d = Diags::default();
        for f in [found("a", "x"), found("a", "x"), found("b", "x"), found("a", "y")] {
            d.once(f);
        }
        assert_eq!(d.len(), 3);
        d.truncate(1);
        d.once(found("a", "x"));
        d.once(found("a", "y"));
        assert_eq!(d.iter().map(|f| f.diagnostic.message.as_str()).collect::<Vec<_>>(), ["x", "y"]);
        assert_eq!(d.take().len(), 2);
        d.once(found("a", "x"));
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn a_list_and_a_map_over_anothers_leave_it_as_it_was() {
        let mut base: Shared<String> = Shared::default();
        base.push("a".into());
        let mut mine = base.clone();
        mine[0].push('!');
        mine.push("b".into());
        assert_eq!((base[0].as_str(), base.len(), mine[0].as_str(), mine.len()), ("a", 1, "a!", 2));

        let mut base: Over<u32, Vec<u32>> = Over::default();
        base.insert(1, vec![1]);
        base.settle();
        let mut mine = base.clone();
        mine.entry_or_default(1).push(2);
        mine.insert(3, vec![]);
        assert_eq!((base.get(&1), mine.get(&1), mine.own().len()), (Some(&vec![1]), Some(&vec![1, 2]), 2));
    }
}
