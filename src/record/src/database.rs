use sqlx::{PgPool, migrate::MigrateDatabase, Postgres, Row};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::error::RecordError;
use crate::models::{Recording, RecordingStatus};

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, RecordError> {
        // Create database if it doesn't exist
        if !Postgres::database_exists(database_url).await.unwrap_or(false) {
            Postgres::create_database(database_url).await?;
        }

        let pool = PgPool::connect(database_url).await?;
        
        Ok(Database { pool })
    }

    pub async fn migrate(&self) -> Result<(), RecordError> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    pub async fn create_recording(
        &self,
        id: Uuid,
        file_name: String,
        file_path: String,
        start_time: DateTime<Utc>,
    ) -> Result<Recording, RecordError> {
        let status = "RECORDING";
        let row = sqlx::query(
            r#"
            INSERT INTO recordings (id, file_name, file_path, start_time, status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
            RETURNING id, file_name, file_path, start_time, end_time, duration_seconds, 
                      file_size_bytes, status, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(&file_name)
        .bind(&file_path)
        .bind(start_time)
        .bind(status)
        .fetch_one(&self.pool)
        .await?;

        let recording = Recording {
            id: row.get("id"),
            file_name: row.get("file_name"),
            file_path: row.get("file_path"),
            start_time: row.get("start_time"),
            end_time: row.get("end_time"),
            duration_seconds: row.get("duration_seconds"),
            file_size_bytes: row.get("file_size_bytes"),
            status: match row.get::<String, _>("status").as_str() {
                "RECORDING" => RecordingStatus::Recording,
                "COMPLETED" => RecordingStatus::Completed,
                "FAILED" => RecordingStatus::Failed,
                _ => RecordingStatus::Failed,
            },
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        };

        Ok(recording)
    }

    pub async fn update_recording_completed(
        &self,
        id: Uuid,
        end_time: DateTime<Utc>,
        duration_seconds: i64,
        file_size_bytes: i64,
    ) -> Result<Recording, RecordError> {
        let status = "COMPLETED";
        let row = sqlx::query(
            r#"
            UPDATE recordings 
            SET end_time = $2, duration_seconds = $3, file_size_bytes = $4, 
                status = $5, updated_at = NOW()
            WHERE id = $1
            RETURNING id, file_name, file_path, start_time, end_time, duration_seconds, 
                      file_size_bytes, status, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(end_time)
        .bind(duration_seconds)
        .bind(file_size_bytes)
        .bind(status)
        .fetch_one(&self.pool)
        .await?;

        let recording = Recording {
            id: row.get("id"),
            file_name: row.get("file_name"),
            file_path: row.get("file_path"),
            start_time: row.get("start_time"),
            end_time: row.get("end_time"),
            duration_seconds: row.get("duration_seconds"),
            file_size_bytes: row.get("file_size_bytes"),
            status: match row.get::<String, _>("status").as_str() {
                "RECORDING" => RecordingStatus::Recording,
                "COMPLETED" => RecordingStatus::Completed,
                "FAILED" => RecordingStatus::Failed,
                _ => RecordingStatus::Failed,
            },
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        };

        Ok(recording)
    }

    /*
    pub async fn update_recording_failed(&self, id: Uuid) -> Result<Recording, RecordError> {
        let status = "FAILED";
        let row = sqlx::query(
            r#"
            UPDATE recordings 
            SET status = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING id, file_name, file_path, start_time, end_time, duration_seconds, 
                      file_size_bytes, status, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(status)
        .fetch_one(&self.pool)
        .await?;

        let recording = Recording {
            id: row.get("id"),
            file_name: row.get("file_name"),
            file_path: row.get("file_path"),
            start_time: row.get("start_time"),
            end_time: row.get("end_time"),
            duration_seconds: row.get("duration_seconds"),
            file_size_bytes: row.get("file_size_bytes"),
            status: match row.get::<String, _>("status").as_str() {
                "RECORDING" => RecordingStatus::Recording,
                "COMPLETED" => RecordingStatus::Completed,
                "FAILED" => RecordingStatus::Failed,
                _ => RecordingStatus::Failed,
            },
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        };

        Ok(recording)
    }
    */

    pub async fn get_recording(&self, id: Uuid) -> Result<Recording, RecordError> {
        let row = sqlx::query(
            r#"
            SELECT id, file_name, file_path, start_time, end_time, duration_seconds, 
                   file_size_bytes, status, created_at, updated_at
            FROM recordings 
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| RecordError::RecordingNotFound(id.to_string()))?;

        let recording = Recording {
            id: row.get("id"),
            file_name: row.get("file_name"),
            file_path: row.get("file_path"),
            start_time: row.get("start_time"),
            end_time: row.get("end_time"),
            duration_seconds: row.get("duration_seconds"),
            file_size_bytes: row.get("file_size_bytes"),
            status: match row.get::<String, _>("status").as_str() {
                "RECORDING" => RecordingStatus::Recording,
                "COMPLETED" => RecordingStatus::Completed,
                "FAILED" => RecordingStatus::Failed,
                _ => RecordingStatus::Failed,
            },
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        };

        Ok(recording)
    }

    pub async fn list_recordings(&self) -> Result<Vec<Recording>, RecordError> {
        let rows = sqlx::query(
            r#"
            SELECT id, file_name, file_path, start_time, end_time, duration_seconds, 
                   file_size_bytes, status, created_at, updated_at
            FROM recordings 
            ORDER BY start_time DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let recordings = rows.into_iter().map(|row| Recording {
            id: row.get("id"),
            file_name: row.get("file_name"),
            file_path: row.get("file_path"),
            start_time: row.get("start_time"),
            end_time: row.get("end_time"),
            duration_seconds: row.get("duration_seconds"),
            file_size_bytes: row.get("file_size_bytes"),
            status: match row.get::<String, _>("status").as_str() {
                "RECORDING" => RecordingStatus::Recording,
                "COMPLETED" => RecordingStatus::Completed,
                "FAILED" => RecordingStatus::Failed,
                _ => RecordingStatus::Failed,
            },
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }).collect();

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