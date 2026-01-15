//! OAuth constants for OpenAI Codex authentication.

/// OAuth client ID for Codex CLI applications.
pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

/// OpenAI OAuth token endpoint.
pub const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";

/// Default issuer base URL.
pub const ISSUER: &str = "https://auth.openai.com";

/// Default local callback port for OAuth redirect.
pub const DEFAULT_PORT: u16 = 1455;

/// OAuth scopes required for Codex access.
pub const SCOPE: &str = "openid profile email offline_access";

/// JWT claim path containing OpenAI auth metadata.
pub const JWT_AUTH_CLAIM: &str = "https://api.openai.com/auth";

/// Codex API base URL (ChatGPT backend).
pub const CODEX_API_BASE: &str = "https://chatgpt.com/backend-api";

/// Codex responses endpoint path.
pub const CODEX_RESPONSES_PATH: &str = "/codex/responses";

/// Originator identifier for API requests.
pub const ORIGINATOR: &str = "xeno";
