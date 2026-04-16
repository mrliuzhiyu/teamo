// storage/ · 本地 SQLite 存储层
//
// 职责：数据库连接管理、migration、业务读写

pub mod canonicalize;
pub mod repository;
pub mod schema;

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

/// 全局数据库句柄（Mutex 包装，Tauri 多线程安全）
pub struct AppDatabase {
    conn: Mutex<Connection>,
    data_dir: PathBuf,
}

impl AppDatabase {
    /// 初始化数据库：打开/创建 clipboard.db + 执行 migration
    pub fn init(data_dir: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(&data_dir)?;

        let db_path = data_dir.join("clipboard.db");
        let conn = Connection::open(&db_path)?;

        // WAL 模式 + 性能优化
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;",
        )?;

        // 执行 migration
        schema::run_migrations(&conn)?;

        // 创建 images 子目录
        std::fs::create_dir_all(data_dir.join("images"))?;

        tracing::info!("SQLite initialized at {}", db_path.display());

        Ok(Self {
            conn: Mutex::new(conn),
            data_dir,
        })
    }

    /// 获取数据库连接引用（加锁）
    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("database mutex poisoned")
    }

    /// 图片存储目录
    pub fn images_dir(&self) -> PathBuf {
        self.data_dir.join("images")
    }
}
