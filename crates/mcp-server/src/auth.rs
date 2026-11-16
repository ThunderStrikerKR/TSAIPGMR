use std::{env, sync::Arc};

use axum::{
    extract::{Request, State},
    http::{StatusCode, header, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use rmcp::{
    ErrorData,
    handler::server::common::{AsRequestContext, FromContextPart},
};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Collector,
    Reader,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Collector => "collector",
            Self::Reader => "reader",
        }
    }
}

#[derive(Clone)]
pub struct Scope {
    pub project: Arc<str>,
    pub role: Role,
}

impl<C: AsRequestContext> FromContextPart<C> for Scope {
    fn from_context_part(context: &mut C) -> Result<Self, ErrorData> {
        context
            .as_request_context()
            .extensions
            .get::<Parts>()
            .and_then(|parts| parts.extensions.get::<Self>())
            .cloned()
            .ok_or_else(|| ErrorData::internal_error("Missing authenticated scope", None))
    }
}

#[derive(Clone)]
pub struct Auth {
    project: Arc<str>,
    collector: [u8; 32],
    reader: [u8; 32],
}

impl Auth {
    pub fn from_env(project: String) -> Result<Self, String> {
        let collector = load_digest("MCP_COLLECTOR_TOKEN")?;
        let reader = load_digest("MCP_READER_TOKEN")?;
        if bool::from(collector.ct_eq(&reader)) {
            return Err("Collector and reader tokens must differ".into());
        }

        Ok(Self {
            project: project.into(),
            collector,
            reader,
        })
    }

    fn scope(&self, token: &str) -> Option<Scope> {
        let digest = Sha256::digest(token.as_bytes());
        let collector = bool::from(self.collector.ct_eq(digest.as_slice()));
        let reader = bool::from(self.reader.ct_eq(digest.as_slice()));
        let role = match (collector, reader) {
            (true, false) => Role::Collector,
            (false, true) => Role::Reader,
            _ => return None,
        };
        Some(Scope {
            project: self.project.clone(),
            role,
        })
    }
}

fn load_digest(name: &str) -> Result<[u8; 32], String> {
    let token = env::var(name).map_err(|_| format!("{name} is required"))?;
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "{name} must contain exactly 64 hexadecimal characters"
        ));
    }
    Ok(Sha256::digest(token.as_bytes()).into())
}

pub async fn authorize(State(auth): State<Auth>, mut request: Request, next: Next) -> Response {
    let scope = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .and_then(|(_, token)| auth.scope(token));
    let Some(scope) = scope else {
        return (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
        )
            .into_response();
    };
    request.extensions_mut().insert(scope);
    next.run(request).await
}
