use sqlx::mysql::{MySqlConnectOptions, MySqlPool, MySqlPoolOptions};
use std::{str::FromStr, time::Duration};

use crate::errors::AppError;

pub fn create_pool(url: Option<&str>) -> Result<Option<MySqlPool>, AppError> {
    let Some(url) = url.filter(|url| !url.trim().is_empty()) else {
        return Ok(None);
    };

    let options = MySqlConnectOptions::from_str(url)
        .map_err(|error| AppError::BadRequest(format!("invalid MYSQL_URL: {error}")))?;
    Ok(Some(
        MySqlPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
            .connect_lazy_with(options),
    ))
}
