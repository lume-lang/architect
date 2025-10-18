mod cached_query;

use proc_macro::TokenStream;

/// Defines a memoized query method, which stores
/// results of the method in a database cache store. Cached results are
/// keyed from the method name and arguments.
///
/// # Attributes
/// - `db`: (optional, expr) specify the value which should be used to get the
///   database instance. Defaults to `self`.
///
///   NOTE: the resulting expression **must** implement
///   [`lume_architect::DatabaseContext`].
///
///   Example:
///   ```rs
///   #[cached_query(db = self.db())]
///   ```
///
/// - `key`: (optional, expr) specify the value(s) which should be used to
///   create the cache key.
///
///   NOTE: the resulting expression **must** implement [`std::hash::Hash`].
///
///   Example:
///   ```rs
///   #[cached_query(key = self.id)]
///   ```
///
/// - `result`: (optional, boolean) specifies that the return type of the method
///   is a [`Result`], which should only be cached if the method returned
///   successfully.
///
///   Example:
///   ```rs
///   #[cached_query(result)]
///   ```
#[proc_macro_attribute]
pub fn cached_query(args: TokenStream, input: TokenStream) -> TokenStream {
    cached_query::cached_query(args, input)
}
