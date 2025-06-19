// Authentication middleware for Whisper API
//
// This module provides authentication middleware for the Whisper API.
// It verifies that incoming requests have a valid Authorization header when enabled.
// OPTIONS requests are always allowed to support CORS pre-flight requests.

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    error::ErrorUnauthorized,
    http::header,
    Error,
};
use futures::future::{ok, LocalBoxFuture, Ready};
use log::{debug, info, warn};
use std::env;

/// Default setting for authorization requirement
const DEFAULT_ENABLE_AUTHORIZATION: bool = true;

/// Helper function to check if authorization is enabled
fn is_authorization_enabled() -> bool {
    env::var("ENABLE_AUTHORIZATION")
        .ok()
        .and_then(|val| val.parse::<bool>().ok())
        .unwrap_or(DEFAULT_ENABLE_AUTHORIZATION)
}

/// Middleware factory for authentication
pub struct Authentication;

impl<S, B> Transform<S, ServiceRequest> for Authentication
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthenticationMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        // Log authentication status at startup
        let auth_enabled = is_authorization_enabled();
        if !auth_enabled {
            info!("Authentication requirement is disabled via configuration");
        }
        ok(AuthenticationMiddleware { service })
    }
}

/// Authentication middleware implementation
pub struct AuthenticationMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for AuthenticationMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Skip authentication for OPTIONS requests
        if req.method() == actix_web::http::Method::OPTIONS {
            debug!("OPTIONS request - bypassing authentication check");
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res)
            });
        }

        let authenticate_result = authenticate(&req);

        if authenticate_result.is_err() {
            let error = authenticate_result.err().unwrap();
            return Box::pin(async move { Err(error) });
        }

        let fut = self.service.call(req);
        Box::pin(async move {
            let res = fut.await?;
            Ok(res)
        })
    }
}

/// Authenticate a request by checking the Authorization header
fn authenticate(req: &ServiceRequest) -> Result<(), Error> {
    // Check if authorization is enabled via configuration
    if !is_authorization_enabled() {
        debug!("Authorization is disabled, allowing request without authentication");
        return Ok(());
    }

    // Check if Authorization header exists
    if let Some(auth_header) = req.headers().get(header::AUTHORIZATION) {
        // Check if header can be converted to string
        if let Ok(auth_str) = auth_header.to_str() {
            // Check if it's a Bearer token
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..]; // Skip "Bearer " prefix
                debug!("Request received with token: {}", token);

                // Validate the token
                return validate_token(token);
            } else {
                warn!("Invalid Authorization header format, missing 'Bearer' prefix");
                return Err(ErrorUnauthorized(
                    "Invalid Authorization header format. Must be 'Bearer <token>'",
                ));
            }
        } else {
            warn!("Authorization header contains invalid characters");
            return Err(ErrorUnauthorized("Invalid Authorization header"));
        }
    } else {
        warn!("Missing Authorization header");
        return Err(ErrorUnauthorized("Authorization header is required"));
    }
}
/// Validates a token for authentication
///
/// Currently just returns Ok for any token, but can be extended
/// to implement real token validation in the future.
fn validate_token(_token: &str) -> Result<(), Error> {
    // TODO: Implement proper token validation
    Ok(())
}
