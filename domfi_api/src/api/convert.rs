use actix_web::{Responder, HttpResponse};
use actix_web::body::Body;
use log::{error};
use crate::repo::RepositoryError;
use crate::api::models::ErrorResponse;
use crate::historical;
use crate::historical::ClientFindByIdHistoryError;
use crate::api::routes::QueryFlagError;

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

impl ToResponse for ClientFindByIdHistoryError {
    type Output = HttpResponse<Body>;
    fn to_response(&self) -> Self::Output {
        let reason = format!("{}", self);
        match self {
            ClientFindByIdHistoryError::CoinUnknownOrNotAllowed => {
                HttpResponse::BadRequest().json(ErrorResponse::new(reason))
            }
            ClientFindByIdHistoryError::DbError => {
                HttpResponse::InternalServerError().json(ErrorResponse::new(reason))
            }
            ClientFindByIdHistoryError::FailedToLocateService => {
                HttpResponse::InternalServerError().json(ErrorResponse::new(reason))
            }
        }
    }
}

impl ToResponse for QueryFlagError {
    type Output = HttpResponse<Body>;
    fn to_response(&self) -> Self::Output {
        HttpResponse::BadRequest().json(
            ErrorResponse::new(format!("{}", self)))
    }
}

impl ToResponse for tokio::sync::mpsc::error::SendError<historical::HistoryFetchRequest> {
    type Output = HttpResponse<Body>;
    fn to_response(&self) -> Self::Output {
        HttpResponse::InternalServerError().json(
            ErrorResponse::new("Failed to communicate with interval service".into()))
    }
}

impl ToResponse for tokio::sync::mpsc::error::RecvError {
    type Output = HttpResponse<Body>;
    fn to_response(&self) -> Self::Output {
        HttpResponse::InternalServerError().json(
            ErrorResponse::new("Failed to receive response from interval service".into()))
    }
}

impl ToResponse for tokio::sync::oneshot::error::RecvError {
    type Output = HttpResponse<Body>;
    fn to_response(&self) -> Self::Output {
        HttpResponse::InternalServerError().json(
            ErrorResponse::new("Failed to receive response from interval service".into()))
    }
}
