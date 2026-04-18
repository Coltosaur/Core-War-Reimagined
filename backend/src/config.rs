use std::env;

#[derive(Debug)]
pub struct Config {
    pub database_url: String,
    pub frontend_url: String,
    pub port: u16,
    pub jwt_secret: Vec<u8>,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|key| env::var(key).ok())
    }

    fn from_lookup<F>(get: F) -> Result<Self, ConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let database_url = get("DATABASE_URL").ok_or(ConfigError::Missing("DATABASE_URL"))?;

        let frontend_url = get("FRONTEND_URL")
            .unwrap_or_else(|| "http://localhost:5173".into());

        let port: u16 = get("PORT")
            .and_then(|p| p.parse().ok())
            .unwrap_or(3001);

        let jwt_secret_str = get("JWT_SECRET").ok_or(ConfigError::Missing("JWT_SECRET"))?;
        let jwt_secret = jwt_secret_str.into_bytes();
        if jwt_secret.len() < 32 {
            return Err(ConfigError::WeakJwtSecret);
        }

        Ok(Self {
            database_url,
            frontend_url,
            port,
            jwt_secret,
        })
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Missing(&'static str),
    WeakJwtSecret,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(var) => write!(f, "required environment variable {var} is not set"),
            Self::WeakJwtSecret => {
                write!(f, "JWT_SECRET must be at least 32 bytes")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_lookup(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |key: &str| map.get(key).cloned()
    }

    fn valid_env() -> Vec<(&'static str, &'static str)> {
        vec![
            ("DATABASE_URL", "postgresql://localhost/test"),
            ("JWT_SECRET", "this-is-a-secret-that-is-at-least-32-bytes!"),
            ("FRONTEND_URL", "http://localhost:5173"),
            ("PORT", "4000"),
        ]
    }

    #[test]
    fn happy_path_all_vars_set() {
        let lookup = make_lookup(&valid_env());
        let config = Config::from_lookup(lookup).unwrap();

        assert_eq!(config.database_url, "postgresql://localhost/test");
        assert_eq!(config.frontend_url, "http://localhost:5173");
        assert_eq!(config.port, 4000);
        assert_eq!(
            config.jwt_secret,
            b"this-is-a-secret-that-is-at-least-32-bytes!"
        );
    }

    #[test]
    fn defaults_for_optional_vars() {
        let lookup = make_lookup(&[
            ("DATABASE_URL", "postgresql://localhost/test"),
            ("JWT_SECRET", "this-is-a-secret-that-is-at-least-32-bytes!"),
        ]);
        let config = Config::from_lookup(lookup).unwrap();

        assert_eq!(config.frontend_url, "http://localhost:5173");
        assert_eq!(config.port, 3001);
    }

    #[test]
    fn invalid_port_falls_back_to_default() {
        let lookup = make_lookup(&[
            ("DATABASE_URL", "postgresql://localhost/test"),
            ("JWT_SECRET", "this-is-a-secret-that-is-at-least-32-bytes!"),
            ("PORT", "not-a-number"),
        ]);
        let config = Config::from_lookup(lookup).unwrap();
        assert_eq!(config.port, 3001);
    }

    #[test]
    fn missing_database_url() {
        let lookup = make_lookup(&[
            ("JWT_SECRET", "this-is-a-secret-that-is-at-least-32-bytes!"),
        ]);
        let err = Config::from_lookup(lookup).unwrap_err();
        assert!(matches!(err, ConfigError::Missing("DATABASE_URL")));
    }

    #[test]
    fn missing_jwt_secret() {
        let lookup = make_lookup(&[
            ("DATABASE_URL", "postgresql://localhost/test"),
        ]);
        let err = Config::from_lookup(lookup).unwrap_err();
        assert!(matches!(err, ConfigError::Missing("JWT_SECRET")));
    }

    #[test]
    fn jwt_secret_too_short() {
        let lookup = make_lookup(&[
            ("DATABASE_URL", "postgresql://localhost/test"),
            ("JWT_SECRET", "too-short"),
        ]);
        let err = Config::from_lookup(lookup).unwrap_err();
        assert!(matches!(err, ConfigError::WeakJwtSecret));
    }

    #[test]
    fn jwt_secret_exactly_32_bytes() {
        let lookup = make_lookup(&[
            ("DATABASE_URL", "postgresql://localhost/test"),
            ("JWT_SECRET", "abcdefghijklmnopqrstuvwxyz012345"),
        ]);
        let config = Config::from_lookup(lookup).unwrap();
        assert_eq!(config.jwt_secret.len(), 32);
    }

    #[test]
    fn config_error_display() {
        let missing = ConfigError::Missing("DATABASE_URL");
        assert_eq!(
            missing.to_string(),
            "required environment variable DATABASE_URL is not set"
        );

        let weak = ConfigError::WeakJwtSecret;
        assert_eq!(weak.to_string(), "JWT_SECRET must be at least 32 bytes");
    }
}
