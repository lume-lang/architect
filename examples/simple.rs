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
    ctx.db().ensure_query_exists("get_name", QueryFlags::empty);

    let _ = ctx
        .db()
        .execute_query("get_name", &"user_name", || String::from("Admin"))
        .unwrap();

    let result = ctx
        .db()
        .execute_query("get_name", &"user_name", || String::from("Username"))
        .unwrap();

    assert_eq!(result, String::from("Admin"));
}
