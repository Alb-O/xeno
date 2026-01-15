//! OAuth constants for Anthropic Claude authentication.

/// OAuth client ID for Claude CLI applications.
pub const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

/// Claude Max authorization endpoint (for Pro/Max subscription users).
pub const AUTHORIZE_URL_MAX: &str = "https://claude.ai/oauth/authorize";

/// Console authorization endpoint (for API key creation).
pub const AUTHORIZE_URL_CONSOLE: &str = "https://console.anthropic.com/oauth/authorize";

/// Token exchange endpoint.
pub const TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";

/// OAuth redirect URI (Anthropic's hosted callback).
pub const REDIRECT_URI: &str = "https://console.anthropic.com/oauth/code/callback";

/// OAuth scopes for Claude access.
pub const SCOPE: &str = "org:create_api_key user:profile user:inference";

/// API key creation endpoint.
pub const CREATE_API_KEY_URL: &str =
	"https://api.anthropic.com/api/oauth/claude_cli/create_api_key";
