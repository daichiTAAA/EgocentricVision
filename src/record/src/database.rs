use crate::error::RecordError;
use crate::models::{Recording, RecordingStatus};
use chrono::{DateTime, Utc};
use sqlx::{migrate::MigrateDatabase, PgPool, Postgres};
use uuid::Uuid;

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, RecordError> {
        // Create database if it doesn't exist
        if !Postgres::database_exists(database_url)
            .await
            .unwrap_or(false)
        {
            Postgres::create_database(database_url).await?;
        }

        let pool = PgPool::connect(database_url).await?;

        Ok(Database { pool })
    }

    pub async fn migrate(&self) -> Result<(), RecordError> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    pub async fn is_connected(&self) -> bool {
        self.pool.acquire().await.is_ok()
    }

    pub async fn create_recording(
        &self,
        id: Uuid,
        file_name: String,
        file_path: String,
        start_time: DateTime<Utc>,
    ) -> Result<Recording, RecordError> {
        let status = RecordingStatus::Recording;
        let recording = sqlx::query_as!(
            Recording,
            r#"
            INSERT INTO recordings (id, file_name, file_path, start_time, status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
            RETURNING id, file_name, file_path, start_time, end_time, duration_seconds, 
                      file_size_bytes, status AS "status: _", created_at, updated_at
            "#,
            id,
            file_name,
            file_path,
            start_time,
            status as _,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(recording)
    }

    pub async fn update_recording_completed(
        &self,
        id: Uuid,
        end_time: DateTime<Utc>,
        duration_seconds: i64,
        file_size_bytes: i64,
    ) -> Result<Recording, RecordError> {
        let status = RecordingStatus::Completed;
        let recording = sqlx::query_as!(
            Recording,
            r#"
            UPDATE recordings 
            SET end_time = $2, duration_seconds = $3, file_size_bytes = $4, 
                status = $5, updated_at = NOW()
            WHERE id = $1
            RETURNING id, file_name, file_path, start_time, end_time, duration_seconds, 
                      file_size_bytes, status AS "status: _", created_at, updated_at
            "#,
            id,
            end_time,
            duration_seconds,
            file_size_bytes,
            status as _,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(recording)
    }

    #[allow(dead_code)]
    pub async fn update_recording_failed(&self, id: Uuid) -> Result<Recording, RecordError> {
        let status = RecordingStatus::Failed;
        let recording = sqlx::query_as!(
            Recording,
            r#"
            UPDATE recordings 
            SET status = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING id, file_name, file_path, start_time, end_time, duration_seconds, 
                      file_size_bytes, status AS "status: _", created_at, updated_at
            "#,
            id,
            status as _,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(recording)
    }

    pub async fn get_recording(&self, id: Uuid) -> Result<Recording, RecordError> {
        let recording = sqlx::query_as!(
            Recording,
            r#"
            SELECT id, file_name, file_path, start_time, end_time, duration_seconds, 
                   file_size_bytes, status AS "status: _", created_at, updated_at
            FROM recordings 
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| RecordError::RecordingNotFound(id.to_string()))?;

        Ok(recording)
    }

    pub async fn list_recordings(&self) -> Result<Vec<Recording>, RecordError> {
        let recordings = sqlx::query_as!(
            Recording,
            r#"
            SELECT id, file_name, file_path, start_time, end_time, duration_seconds, 
                   file_size_bytes, status as "status: _", created_at, updated_at
            FROM recordings 
            ORDER BY start_time DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(recordings)
    }

    pub async fn delete_recording(&self, id: Uuid) -> Result<(), RecordError> {
        let result = sqlx::query("DELETE FROM recordings WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(RecordError::RecordingNotFound(id.to_string()));
        }

        Ok(())
    }
}
