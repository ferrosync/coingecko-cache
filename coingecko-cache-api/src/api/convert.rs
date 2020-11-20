use actix_web::{Responder, HttpResponse};
use actix_web::body::Body;
use crate::repo::RepositoryError;
use crate::api::models::ErrorResponse;

pub trait ToResponse {
    type Output : Responder;
    fn to_response(&self) -> Self::Output;
}

impl ToResponse for RepositoryError {
    type Output = HttpResponse<Body>;
    fn to_response(&self) -> Self::Output {
        match self {
            RepositoryError::SqlError { source: sqlx::Error::RowNotFound } => {
                HttpResponse::NotFound().json(
                    ErrorResponse::new("Unable to find data origin requested".into()))
            },
            RepositoryError::SqlError { source } => {
                error!("Database error: {}", source);
                HttpResponse::InternalServerError().json(
                    ErrorResponse::new("Invalid database connection error".into()))
            },
        }
    }
}
