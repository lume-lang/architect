use std::any::Any;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::DerefMut;

use bitflags::bitflags;
use indexmap::IndexSet;
#[cfg(feature = "derive")]
pub use lume_architect_derive::cached_query;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryError {
    Cycle,
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cycle => write!(f, "cycle detected"),
        }
    }
}

impl std::error::Error for QueryError {}

pub type QueryResult<T> = Result<T, QueryError>;

/// Represents a unique index, referencing a [`Query`] within a [`Database`].
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct QueryId(usize);

impl QueryId {
    /// Creates a new [`QueryId`] from the given string.
    pub fn from_name(str: &str) -> Self {
        let hash = fxhash::hash(str);

        Self(hash)
    }
}

/// Represents a unique index, referencing a result within a [`Query`].
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResultKey(usize);

impl ResultKey {
    /// Creates a new [`ResultKey`] from a value, implementing [`Hash`].
    pub fn from_hashable<H: Hash>(h: &H) -> Self {
        let hash = fxhash::hash(h);

        Self(hash)
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct QueryFlags: u32 {
        /// Always re-compute the result of the query, even if a matching entry
        /// already exists within the result set.
        const ALWAYS = 1;
    }
}

#[derive(Debug)]
pub struct Query {
    name: String,
    flags: QueryFlags,
    results: HashMap<ResultKey, Box<dyn Any>>,
}

impl Query {
    /// Creates a new [`Query`] with the given name.
    pub fn new(name: String, flags: QueryFlags) -> Self {
        Self {
            name,
            flags,
            results: HashMap::new(),
        }
    }

    /// Gets the name of the query.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Gets the flags of the query.
    #[inline]
    pub fn flags(&self) -> QueryFlags {
        self.flags
    }

    /// Gets the result with the given value as the result key.
    ///
    /// The value used for the key must be the same as the key used when
    /// inserting the value.
    ///
    /// # Returns
    ///
    /// If no value could be found, or the value found is not of type [`T`],
    /// this method returns [`None`].
    pub fn get<K: Hash, T: Clone + 'static>(&self, key: &K) -> Option<&T> {
        let key = ResultKey::from_hashable(key);

        self.results.get(&key)?.downcast_ref::<T>()
    }

    /// Inserts the given result into the query, indexed by the given key.
    ///
    /// If the query already contains a result for the key [`key`], the old
    /// result is overwritten.
    pub fn insert<K: Hash, T: Clone + 'static>(&mut self, key: &K, value: T) {
        let key = ResultKey::from_hashable(key);
        let value = Box::new(value);

        self.results.insert(key, value);
    }

    /// Determines whether the query contains a result for the given key.
    ///
    /// The value used for the key must be the same as the key used when
    /// inserting the value.
    pub fn contains<K: Hash>(&self, key: &K) -> bool {
        let key = ResultKey::from_hashable(key);

        self.results.contains_key(&key)
    }

    /// Looks up the given key within the query instance.
    ///
    /// If a value is found within the query, it is returned as a reference. If
    /// the key could not be found within the instance, returns [`None`].
    /// stored, the original result is returned.
    fn value_of<K: Hash, T: Clone + 'static>(&self, key: &K) -> Option<&T> {
        let key = ResultKey::from_hashable(key);
        let value = self.results.get(&key)?;

        Some(
            value
                .downcast_ref::<T>()
                .unwrap_or_else(|| panic!("could not convert result `{}.!{}` to type of T", self.name, key.0)),
        )
    }

    /// Looks up the given key within the query instance.
    ///
    /// If a value is found within the query, it is returned as a reference. If
    /// the key could not be found within the instance, `f` is invoked and the
    /// result is cloned and inserted into the instance. After the result is
    /// stored, the original result is returned.
    pub fn get_or_insert<K: Hash, T: Clone + 'static>(&mut self, key: &K, f: impl FnOnce() -> T) -> &T {
        if self.flags.contains(QueryFlags::ALWAYS) || !self.contains(key) {
            self.insert(key, f());
        }

        self.value_of(key).unwrap()
    }

    /// Looks up the given key within the query instance.
    ///
    /// If a value is found within the query, it is returned as a reference. If
    /// the key could not be found within the instance, `f` is invoked and the
    /// result is cloned and inserted into the instance. After the result is
    /// stored, the original result is returned.
    ///
    /// # Errors
    ///
    /// If the given closure returns `Err`, this method will propagate the error
    /// to the caller.
    pub fn get_or_insert_result<K: Hash, T: Clone + 'static, E>(
        &mut self,
        key: &K,
        f: impl FnOnce() -> Result<T, E>,
    ) -> Result<&T, E> {
        if self.flags.contains(QueryFlags::ALWAYS) || !self.contains(key) {
            self.insert(key, f()?);
        }

        Ok(self.value_of(key).unwrap())
    }
}

/// Inner, non-locked version of [`Database`].
#[derive(Default)]
pub(crate) struct DatabaseInner {
    pub(crate) queries: HashMap<QueryId, Query>,
    pub(crate) active: IndexSet<(QueryId, ResultKey)>,
}

impl DatabaseInner {
    /// Clears all results from the query with the given name.
    #[inline]
    pub fn clear(&mut self, query: &str) {
        self.query_mut(query).results.clear();
    }

    /// Clears all results from all queries in the database.
    #[inline]
    pub fn clear_all(&mut self) {
        self.queries.clear();
    }

    /// Retrieves a shared read access to the [`Query`] which matches the given
    /// query name.
    ///
    /// # Panics
    ///
    /// This method panics if another thread write-locked the query before
    /// this method was invoked, without releasing the lock.
    pub fn query(&self, name: &str) -> &Query {
        let id = QueryId::from_name(name);

        self.queries.get(&id).unwrap()
    }

    /// Retrieves an exclusive-write access to the [`Query`] which matches the
    /// given query name.
    ///
    /// # Panics
    ///
    /// This method panics if another thread write-locked the query before
    /// this method was invoked, without releasing the lock.
    pub fn query_mut(&mut self, name: &str) -> &mut Query {
        let id = QueryId::from_name(name);

        self.queries.get_mut(&id).unwrap()
    }

    /// Adds a new [`Query`] to the database, with the given name and flags.
    ///
    /// # Panics
    ///
    /// This method will panic if a query with the given name already exists.
    #[inline]
    pub fn add_query(&mut self, name: &str, flags: QueryFlags) {
        let key = QueryId::from_name(name);
        let existing = self.queries.insert(key, Query::new(name.to_string(), flags));

        assert!(existing.is_none(), "duplicate query name: {name}");
    }

    /// Determines whether a query with the given name exists within the
    /// database.
    #[inline]
    pub fn query_exists(&self, name: &str) -> bool {
        let key = QueryId::from_name(name);

        self.queries.contains_key(&key)
    }
}

#[derive(Default)]
pub struct Database {
    inner: UnsafeCell<DatabaseInner>,
}

impl Database {
    /// Creates a new empty [`Database`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Retrieves a shared read access to the [`DatabaseInner`]'s inner
    /// instance.
    #[inline]
    pub(crate) fn read(&self) -> &DatabaseInner {
        unsafe { self.inner.get().as_ref() }.unwrap()
    }

    /// Retrieves an exclusive-write access to the [`DatabaseInner`]'s inner
    /// instance.
    #[inline]
    #[expect(clippy::mut_from_ref)]
    pub(crate) fn write(&self) -> &mut DatabaseInner {
        unsafe { self.inner.get().as_mut() }.unwrap()
    }

    /// Clears all results from the query with the given name.
    #[inline]
    pub fn clear(&self, query: &str) {
        self.write().clear(query);
    }

    /// Clears all results from all queries in the database.
    #[inline]
    pub fn clear_all(&self) {
        self.write().clear_all();
    }

    /// Retrieves a shared read access to the [`Query`] which matches the given
    /// query name.
    pub fn query(&self, name: &str) -> &Query {
        self.read().query(name)
    }

    /// Retrieves an exclusive-write access to the [`Query`] which matches the
    /// given query name.
    pub fn query_mut(&self, name: &str) -> &mut Query {
        self.write().query_mut(name)
    }

    /// Ensures that a [`Query`] with the given name exists. If the query does
    /// not exist, a new [`Query`] is added with the given name, using the
    /// flags returned by `flags`.
    ///
    /// # Panics
    ///
    /// This method panics if another thread write-locked the query before
    /// this method was invoked, without releasing the lock.
    pub fn ensure_query_exists(&self, name: &str, flags: impl FnOnce() -> QueryFlags) {
        if !self.read().query_exists(name) {
            self.write().add_query(name, flags());
        }
    }

    /// Looks up the given key within the query instance with the given name.
    ///
    /// If a value is found within the query, it is cloned and returned. If
    /// the key could not be found within the instance, `f` is invoked and the
    /// result is cloned and inserted into the instance. After the result is
    /// stored, the original result is returned.
    pub fn execute_query<K: Hash, T: Clone + 'static>(
        &self,
        name: &str,
        key: &K,
        f: impl FnOnce() -> T,
    ) -> QueryResult<T> {
        self.call_query(name, key, |query| query.get_or_insert(key, f).clone())
    }

    /// Looks up the given key within the query instance with the given name.
    ///
    /// If a value is found within the query, it is cloned and returned. If
    /// the key could not be found within the instance, `f` is invoked and the
    /// result is cloned and inserted into the instance. After the result is
    /// stored, the original result is returned.
    ///
    /// # Errors
    ///
    /// If the given closure returns `Err`, this method will propagate the error
    /// to the caller.
    pub fn execute_query_result<K: Hash, T: Clone + 'static, E>(
        &self,
        name: &str,
        key: &K,
        f: impl FnOnce() -> Result<T, E>,
    ) -> QueryResult<Result<T, E>> {
        self.call_query(name, key, |query| Ok(query.get_or_insert_result(key, f)?.clone()))
    }

    /// Calls the [`Query`] with the given name, passing it to the given closure
    /// `f`.
    ///
    /// # Panics
    ///
    /// The query key is added to the set of active queries, allowing the
    /// database to detect for cycles. If a cycle is found, the
    /// corresponding query handler is invoked. If none is found, the method
    /// panics.
    fn call_query<K: Hash, T>(&self, name: &str, key: &K, f: impl FnOnce(&mut Query) -> T) -> QueryResult<T> {
        let query_id = QueryId::from_name(name);
        let result_key = ResultKey::from_hashable(key);

        // If the query with the same arguments already exists within the set,
        // we're in a cycle.
        if !self.write().active.insert((query_id, result_key)) {
            return Err(QueryError::Cycle);
        }

        let result = f(self.query_mut(name).deref_mut());
        self.write().active.pop();

        Ok(result)
    }
}

/// A trait that provides access to a [`Database`] instance.
pub trait DatabaseContext {
    /// Retrieves the instance of [`Database`], which is provided by the
    /// [`DatabaseContext`] implementation.
    fn db(&self) -> &Database;
}
