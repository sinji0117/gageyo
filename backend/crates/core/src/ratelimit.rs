//! 요청 횟수 제한 (MariaDB `rate_limits` 표).
//!
//! bandiuri 는 Redis 로 처리하지만 가게요는 Redis 없이 배포하므로 DB 한 줄로 셉니다.
//! 고정 창(fixed window): 창이 지나면 1부터 다시 셉니다. 동시에 여러 요청이 와도 한두 번 차이는 허용합니다.

use sqlx::MySqlPool;

use crate::AppResult;

/// `bucket` 을 1 늘리고, 창(`window_secs`) 안에서 `max` 이하인지 돌려줍니다.
pub async fn hit(db: &MySqlPool, bucket: &str, window_secs: i64, max: i64) -> AppResult<bool> {
    let bucket = trim_bucket(bucket);
    // MariaDB 의 ON DUPLICATE KEY UPDATE 는 왼쪽부터 차례로 계산되므로
    // hits 를 먼저(예전 window_start 기준으로) 계산하고, 그다음 window_start 를 바꿉니다.
    sqlx::query(
        "INSERT INTO rate_limits (bucket, hits, window_start) VALUES (?, 1, NOW()) \
         ON DUPLICATE KEY UPDATE \
           hits = IF(window_start < NOW() - INTERVAL ? SECOND, 1, hits + 1), \
           window_start = IF(window_start < NOW() - INTERVAL ? SECOND, NOW(), window_start)",
    )
    .bind(&bucket)
    .bind(window_secs)
    .bind(window_secs)
    .execute(db)
    .await?;
    let hits: i32 = sqlx::query_scalar("SELECT hits FROM rate_limits WHERE bucket = ?").bind(&bucket).fetch_one(db).await?;
    Ok(i64::from(hits) <= max)
}

/// 지금 창 안의 횟수 (늘리지 않음)
pub async fn count(db: &MySqlPool, bucket: &str, window_secs: i64) -> AppResult<i64> {
    let n: Option<i32> = sqlx::query_scalar(
        "SELECT hits FROM rate_limits WHERE bucket = ? AND window_start >= NOW() - INTERVAL ? SECOND",
    )
    .bind(trim_bucket(bucket))
    .bind(window_secs)
    .fetch_optional(db)
    .await?;
    Ok(n.map(i64::from).unwrap_or(0))
}

pub async fn clear(db: &MySqlPool, bucket: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM rate_limits WHERE bucket = ?").bind(trim_bucket(bucket)).execute(db).await?;
    Ok(())
}

/// 오래된 줄 정리 (하루 지난 것)
pub async fn cleanup(db: &MySqlPool) -> AppResult<u64> {
    let r = sqlx::query("DELETE FROM rate_limits WHERE window_start < NOW() - INTERVAL 1 DAY").execute(db).await?;
    Ok(r.rows_affected())
}

fn trim_bucket(b: &str) -> String {
    b.chars().take(191).collect()
}

#[cfg(test)]
mod tests {
    use super::trim_bucket;

    #[test]
    fn bucket_is_trimmed_to_key_length() {
        assert_eq!(trim_bucket(&"가".repeat(300)).chars().count(), 191);
        assert_eq!(trim_bucket("login:c:a@b.kr"), "login:c:a@b.kr");
    }
}
