use aes_gcm::Aes256Gcm;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub cipher: Aes256Gcm,
}

impl AppState {
    pub fn new(db: PgPool, cipher: Aes256Gcm) -> Self {
        Self {
            db,
            cipher,
        }
    }
}