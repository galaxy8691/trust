//! **ORM / Diesel 与 Trust 的分工**：下面这种链式 API（`filter().load()?`、关联类型、`?`）**只能写在 Rust 里**，
//! 再通过**窄接口**（本 crate 的固有方法）暴露给 TypeScript。
//!
//! ```ignore
//! // 典型 Diesel（不能整段映射到 `.ts`）：
//! let users = users::table
//!     .filter(name.eq("Tom"))
//!     .load::<User>(&mut conn)?; // `?`、生命周期、`Queryable` 关联类型都在此层消化
//! ```
//!
//! Trust 清单里只绑定 `OrmFacade::new` 与 `users_named_tom_count` 这类**签名简单**的方法。

use std::sync::Mutex;

use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

diesel::table! {
    users (id) {
        id -> Integer,
        name -> Text,
    }
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = users)]
#[allow(dead_code)]
struct User {
    id: i32,
    name: String,
}

/// TS 侧 `import { OrmFacade } from "trust_orm_facade"` 对应的类型；内部持有内存库连接。
pub struct OrmFacade {
    conn: Mutex<SqliteConnection>,
}

impl OrmFacade {
    /// Trust 的 `new TypeName(x: string)` 会生成 `TypeName::new(&(x))`；此处忽略占位参数。
    pub fn new(_unused: &String) -> Self {
        let mut conn = SqliteConnection::establish(":memory:").expect("sqlite :memory:");
        conn.batch_execute(
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL
            );
            INSERT INTO users (name) VALUES ('Tom'), ('Jerry'), ('Tom');
            "#,
        )
        .expect("schema + seed");

        Self {
            conn: Mutex::new(conn),
        }
    }

    /// Diesel 查询链封装在此方法内；返回 **行数**（`number`）便于 Trust 绑定。
    pub fn users_named_tom_count(&self) -> f64 {
        let mut conn = self.conn.lock().expect("orm conn");
        let rows: Vec<User> = users::table
            .filter(users::name.eq("Tom"))
            .select(User::as_select())
            .load(&mut *conn)
            .expect("diesel load");
        rows.len() as f64
    }
}
