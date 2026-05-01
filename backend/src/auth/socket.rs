use axum_extra::extract::cookie::CookieJar;
use serde::Serialize;
use socketioxide::extract::SocketRef;
use tracing::info;
use uuid::Uuid;

use crate::auth::jwt::decode_access_token;

#[derive(Debug, Clone, Serialize)]
pub struct SocketAuthUser {
    pub user_id: Uuid,
    pub username: String,
}

pub fn authenticate_socket(socket: &SocketRef, jwt_secret: &[u8]) -> Option<SocketAuthUser> {
    let jar = CookieJar::from_headers(&socket.req_parts().headers);
    let token = jar.get("access_token")?.value().to_string();
    let claims = decode_access_token(&token, jwt_secret).ok()?;
    let user_id = claims.sub.parse::<Uuid>().ok()?;
    Some(SocketAuthUser {
        user_id,
        username: claims.username,
    })
}

pub fn store_auth(socket: &SocketRef, user: Option<SocketAuthUser>) {
    if let Some(u) = user {
        socket.extensions.insert(u);
    }
}

pub fn get_auth(socket: &SocketRef) -> Option<SocketAuthUser> {
    socket.extensions.get::<SocketAuthUser>()
}

pub fn require_auth(socket: &SocketRef) -> Option<SocketAuthUser> {
    let user = get_auth(socket);
    if user.is_none() {
        let _ = socket.emit(
            "auth_error",
            &serde_json::json!({"error": "Authentication required"}),
        );
    }
    user
}

pub fn check_token_expiry(socket: &SocketRef, jwt_secret: &[u8]) {
    let jar = CookieJar::from_headers(&socket.req_parts().headers);
    let is_valid = jar
        .get("access_token")
        .map(|c| decode_access_token(c.value(), jwt_secret).is_ok())
        .unwrap_or(false);

    if !is_valid && get_auth(socket).is_some() {
        let _ = socket.emit(
            "auth_expired",
            &serde_json::json!({"message": "Access token expired, please refresh"}),
        );
    }
}

pub fn on_connect(socket: SocketRef, jwt_secret: Vec<u8>) {
    let user = authenticate_socket(&socket, &jwt_secret);

    match &user {
        Some(u) => info!(
            "client connected: {} (user: {}, id: {})",
            socket.id, u.username, u.user_id
        ),
        None => info!("client connected: {} (anonymous)", socket.id),
    }

    store_auth(&socket, user);

    let secret = jwt_secret.clone();
    socket.on("reauthenticate", move |socket: SocketRef| {
        let new_user = authenticate_socket(&socket, &secret);
        match &new_user {
            Some(u) => {
                info!(
                    "socket {} reauthenticated as {} ({})",
                    socket.id, u.username, u.user_id
                );
                store_auth(&socket, Some(u.clone()));
                let _ = socket.emit(
                    "auth_refreshed",
                    &serde_json::json!({"user_id": u.user_id.to_string(), "username": &u.username}),
                );
            }
            None => {
                let _ = socket.emit(
                    "auth_error",
                    &serde_json::json!({"error": "Reauthentication failed — invalid or missing token"}),
                );
            }
        }
    });

    socket.on_disconnect(|socket: SocketRef| {
        let user = get_auth(&socket);
        match user {
            Some(u) => info!(
                "client disconnected: {} (user: {}, id: {})",
                socket.id, u.username, u.user_id
            ),
            None => info!("client disconnected: {} (anonymous)", socket.id),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_auth_user_is_clone_and_debug() {
        let user = SocketAuthUser {
            user_id: Uuid::new_v4(),
            username: "testuser".into(),
        };
        let cloned = user.clone();
        assert_eq!(user.user_id, cloned.user_id);
        assert_eq!(user.username, cloned.username);
        let debug = format!("{:?}", user);
        assert!(debug.contains("testuser"));
    }

    #[test]
    fn socket_auth_user_serializes() {
        let user = SocketAuthUser {
            user_id: Uuid::new_v4(),
            username: "alice".into(),
        };
        let json = serde_json::to_value(&user).unwrap();
        assert_eq!(json["username"], "alice");
        assert!(json["user_id"].is_string());
    }
}
