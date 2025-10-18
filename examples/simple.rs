use lume_architect::*;

struct Context {
    db: Database,
}

impl DatabaseContext for Context {
    fn db(&self) -> &Database {
        &self.db
    }
}

fn main() {
    let ctx = Context { db: Database::new() };
    let mut query = ctx.db().get_or_add_query("get_name", QueryFlags::empty);

    let _ = query.get_or_insert(&"user_name", || String::from("Admin"));
    let result = query.get_or_insert(&"user_name", || String::from("Admin"));

    println!("Result: {result:?}");
}
