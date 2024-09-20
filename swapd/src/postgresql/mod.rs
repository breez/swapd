mod chain_filter_repository;
mod chain_repository;
mod redeem_repository;
mod swap_repository;

pub use chain_filter_repository::ChainFilterRepository;
pub use chain_repository::ChainRepository;
pub use redeem_repository::RedeemRepository;
use sqlx::{Pool, Postgres};
pub use swap_repository::SwapRepository;

pub async fn migrate(pool: &Pool<Postgres>) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("src/postgresql/migrations")
        .run(&*pool)
        .await
}
