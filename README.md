# Lume Architect

[![CI](https://img.shields.io/github/actions/workflow/status/lume-lang/architect/build?style=for-the-badge)](https://github.com/lume-lang/architect/actions)
[![crates.io](https://img.shields.io/crates/v/lume_architect?style=for-the-badge&label=crates.io)](https://crates.io/crates/lume_architect)
[![docs.rs](https://img.shields.io/docsrs/lume_architect?style=for-the-badge&label=docs.rs)](https://docs.rs/lume_architect)

**A simplistic query system, allowing for on-demand, memoized computation.**

`architect` is a way of defining *queries*. A query is some function which takes a set of arguments and computes some result. By default, results from queries are memoized to prevent recomputation. This does assume that the result of the query is idempotent, given the input arguments.

## Getting started

To make a function into a query, add the crate:
```sh
cargo add lume_architect --features derive
```

On the type which the query operates on, implement the `DatabaseContext` trait:
```rs
use lume_architect::{Database, DatabaseContext};

struct Provider {
    // ... other fields

    db: Database,
}

impl DatabaseContext for Provider {
    fn db(&self) -> &Database {
        &self.db
    }
}
```

To declare a method as a query, add the `cached_query` attribute:
```rs
impl Provider {
    /// Computes some result, which takes a reeeeally long time.
    #[cached_query]
    pub fn compute(&self) -> f32 {
        // ...
    }
}
```

## Inspiration

This implementation is heavily based on [Rust's query system](https://rustc-dev-guide.rust-lang.org/query.html), based on [salsa](https://github.com/salsa-rs/salsa). Massive credit to the countless of amazing developers who helped create them.
