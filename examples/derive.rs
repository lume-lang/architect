use lume_architect::*;

struct Context {
    db: Database,
}

impl DatabaseContext for Context {
    fn db(&self) -> &Database {
        &self.db
    }
}

impl Context {
    // this method is reeeally slow, so don't run it too often!
    #[cached_query(always)]
    pub fn slow_method(&self, count: usize) -> String {
        println!("running slow_method");

        "A".repeat(count)
    }
}

fn main() {
    let ctx = Context { db: Database::new() };

    let r1 = ctx.slow_method(10);
    let r2 = ctx.slow_method(10);

    assert_eq!(r1, r2);
}
