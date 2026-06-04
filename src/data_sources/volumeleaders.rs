//! Session-backed client manager for the VolumeLeaders API.
//!
//! Wraps [`rusty_volumeleaders::Client`] with cache-first startup and
//! automatic re-authentication on auth failures. The manager refreshes
//! cached XSRF tokens, falls back to credential login, and retries login
//! exactly once before surfacing the error.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use rusty_volumeleaders::{
    Client, ClientConfig, ClientError, DataTablesResponse, Session, Trade, TradeCluster,
    TradeClusterBomb, TradeClusterBombsRequest, TradeClustersRequest, TradeLevel,
    TradeLevelsRequest, TradesRequest,
};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Type-erased async function that performs login and returns a fresh [`Client`].
///
/// Production code calls [`rusty_volumeleaders::login()`] followed by
/// [`Client::with_config`]. Tests inject a closure that returns a mock client
/// and tracks call counts.
type LoginFn = Arc<
    dyn Fn(String, String) -> Pin<Box<dyn Future<Output = Result<Client, ClientError>> + Send>>
        + Send
        + Sync,
>;

/// Manages an authenticated VolumeLeaders API session.
///
/// Stores credentials alongside the HTTP client so it can transparently
/// re-authenticate when the server reports an expired or rejected session.
pub struct VolumeLeadersManager {
    username: String,
    password: String,
    client: Arc<RwLock<Client>>,
    login_fn: LoginFn,
}

/// Call a dashboard method on the inner client, retrying login exactly once on
/// session expiry or HTTP 401/403. Avoids lifetime issues with async closures
/// by expanding the retry logic inline.
macro_rules! call_with_retry {
    ($self:expr, $method:ident, $request:expr) => {{
        let result = {
            let client = $self.client.read().await;
            client.$method($request).await
        };
        match result {
            Ok(value) => Ok(value),
            Err(e) if should_relogin(&e) => {
                $self.relogin().await?;
                let client = $self.client.read().await;
                client
                    .$method($request)
                    .await
                    .map_err(|e| crate::Error::VolumeLeaders(e.to_string()))
            }
            Err(e) => Err(crate::Error::VolumeLeaders(e.to_string())),
        }
    }};
}

impl VolumeLeadersManager {
    /// Create a new manager from the cached session or given credentials.
    ///
    /// Refreshes the cached session's XSRF token when cache material exists,
    /// otherwise logs in and stores the credentials for future
    /// re-authentication attempts.
    pub async fn new(username: String, password: String) -> crate::Result<Self> {
        let config = ClientConfig::default();
        let client = match client_from_cached_session(config.clone()).await {
            Some(client) => client,
            None => login_client(&username, &password, config.clone())
                .await
                .map_err(|e| crate::Error::VolumeLeaders(e.to_string()))?,
        };

        let login_fn: LoginFn = Arc::new(|u, p| {
            Box::pin(async move { login_client(&u, &p, ClientConfig::default()).await })
        });

        Ok(Self {
            username,
            password,
            client: Arc::new(RwLock::new(client)),
            login_fn,
        })
    }

    /// Re-authenticate and replace the stored client.
    async fn relogin(&self) -> crate::Result<()> {
        let new_client = (self.login_fn)(self.username.clone(), self.password.clone())
            .await
            .map_err(|e| crate::Error::VolumeLeaders(e.to_string()))?;
        *self.client.write().await = new_client;
        Ok(())
    }

    /// Fetch the trades dashboard.
    pub async fn get_trades(
        &self,
        request: &TradesRequest,
    ) -> crate::Result<DataTablesResponse<Trade>> {
        call_with_retry!(self, get_trades, request)
    }

    /// Fetch trade clusters.
    pub async fn get_trade_clusters(
        &self,
        request: &TradeClustersRequest,
    ) -> crate::Result<DataTablesResponse<TradeCluster>> {
        call_with_retry!(self, get_trade_clusters, request)
    }

    /// Fetch chart-0 trade levels.
    pub async fn get_chart0_trade_levels(
        &self,
        request: &TradeLevelsRequest,
    ) -> crate::Result<DataTablesResponse<TradeLevel>> {
        call_with_retry!(self, get_chart0_trade_levels, request)
    }

    /// Fetch trade cluster bombs.
    pub async fn get_trade_cluster_bombs(
        &self,
        request: &TradeClusterBombsRequest,
    ) -> crate::Result<DataTablesResponse<TradeClusterBomb>> {
        call_with_retry!(self, get_trade_cluster_bombs, request)
    }

    /// Create a manager with a pre-built client and custom login function.
    ///
    /// Used in tests to inject mock servers and track re-authentication.
    #[cfg(test)]
    fn with_client_and_login(
        client: Client,
        username: String,
        password: String,
        login_fn: LoginFn,
    ) -> Self {
        Self {
            username,
            password,
            client: Arc::new(RwLock::new(client)),
            login_fn,
        }
    }
}

/// Return whether a failed request should trigger one fresh login attempt.
fn should_relogin(error: &ClientError) -> bool {
    error.is_session_expired()
        || matches!(
            error,
            ClientError::Status {
                code: 401 | 403,
                ..
            }
        )
}

/// Build a client from the shared session cache, refreshing its XSRF token first.
async fn client_from_cached_session(config: ClientConfig) -> Option<Client> {
    let session = rusty_volumeleaders::load_cached_session()?;
    debug!("using cached VolumeLeaders session");

    match build_client_from_session(session, config).await {
        Ok(client) => Some(client),
        Err(err) => {
            if should_clear_cached_session(&err) {
                warn!(error = %err, "cached VolumeLeaders session invalid, clearing cache");
                rusty_volumeleaders::clear_cache();
            } else {
                warn!(error = %err, "cached VolumeLeaders session unusable, falling back to login");
            }
            None
        }
    }
}

/// Log in with credentials, cache the new session, and build an authenticated client.
async fn login_client(
    username: &str,
    password: &str,
    config: ClientConfig,
) -> Result<Client, ClientError> {
    let session = rusty_volumeleaders::login(username, password).await?;

    if let Err(err) = rusty_volumeleaders::save_session(&session) {
        warn!(error = %err, "failed to cache VolumeLeaders session");
    }

    build_client_from_session(session, config).await
}

/// Build a client from session cookies and refresh the XSRF token from VolumeLeaders.
async fn build_client_from_session(
    session: Session,
    config: ClientConfig,
) -> Result<Client, ClientError> {
    let cookies = session.cookies().to_vec();
    let bootstrap_client = Client::with_config(session, config.clone())?;
    let xsrf_token = rusty_volumeleaders::extract_xsrf_token(&bootstrap_client).await?;
    let refreshed_session = Session::new(cookies, xsrf_token);
    Client::with_config(refreshed_session, config)
}

/// Return whether a cached session should be deleted after a failed refresh.
fn should_clear_cached_session(error: &ClientError) -> bool {
    matches!(
        error,
        ClientError::SessionExpired { .. }
            | ClientError::SessionValidation { .. }
            | ClientError::LoginFailed { .. }
    )
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rusty_volumeleaders::test_support::{datatables_body, test_session};
    use rusty_volumeleaders::{
        Client, ClientConfig, Trade, TradeCluster, TradeClusterBomb, TradeClusterBombsRequest,
        TradeClustersRequest, TradeLevel, TradeLevelsRequest, TradesRequest,
    };

    use super::*;

    /// Build a test client pointed at a mockito server.
    fn test_client_for(server: &mockito::Server) -> Client {
        Client::with_config(
            test_session(),
            ClientConfig {
                base_url: server.url(),
                ..ClientConfig::default()
            },
        )
        .unwrap()
    }

    /// A no-op login function for tests that should not trigger re-auth.
    fn noop_login_fn() -> LoginFn {
        Arc::new(|_u, _p| {
            Box::pin(async {
                panic!("login_fn should not be called in this test");
            })
        })
    }

    #[test]
    fn should_relogin_for_session_expiry_and_forbidden_statuses() {
        assert!(should_relogin(&ClientError::SessionExpired {
            url: "https://www.volumeleaders.com/Login".to_string(),
        }));
        assert!(should_relogin(&ClientError::Status {
            code: 401,
            url: "https://www.volumeleaders.com/Trades/GetTrades".to_string(),
            body: String::new(),
        }));
        assert!(should_relogin(&ClientError::Status {
            code: 403,
            url: "https://www.volumeleaders.com/Trades/GetTrades".to_string(),
            body: String::new(),
        }));
        assert!(!should_relogin(&ClientError::Status {
            code: 500,
            url: "https://www.volumeleaders.com/Trades/GetTrades".to_string(),
            body: String::new(),
        }));
    }

    #[tokio::test]
    async fn volumeleaders_dashboard_calls_succeed() {
        let mut server = mockito::Server::new_async().await;

        let _trades_mock = server
            .mock("POST", "/Trades/GetTrades")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(datatables_body::<Trade>(vec![]))
            .create_async()
            .await;

        let _clusters_mock = server
            .mock("POST", "/TradeClusters/GetTradeClusters")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(datatables_body::<TradeCluster>(vec![]))
            .create_async()
            .await;

        let _levels_mock = server
            .mock("POST", "/Chart0/GetTradeLevels")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(datatables_body::<TradeLevel>(vec![]))
            .create_async()
            .await;

        let _bombs_mock = server
            .mock("POST", "/TradeClusterBombs/GetTradeClusterBombs")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(datatables_body::<TradeClusterBomb>(vec![]))
            .create_async()
            .await;

        let manager = VolumeLeadersManager::with_client_and_login(
            test_client_for(&server),
            "user".into(),
            "pass".into(),
            noop_login_fn(),
        );

        let trades = manager.get_trades(&TradesRequest::default()).await;
        assert!(trades.is_ok(), "get_trades failed: {trades:?}");
        assert!(trades.unwrap().data.is_empty());

        let clusters = manager
            .get_trade_clusters(&TradeClustersRequest::default())
            .await;
        assert!(clusters.is_ok(), "get_trade_clusters failed: {clusters:?}");
        assert!(clusters.unwrap().data.is_empty());

        let levels = manager
            .get_chart0_trade_levels(&TradeLevelsRequest::default())
            .await;
        assert!(levels.is_ok(), "get_chart0_trade_levels failed: {levels:?}");
        assert!(levels.unwrap().data.is_empty());

        let bombs = manager
            .get_trade_cluster_bombs(&TradeClusterBombsRequest::default())
            .await;
        assert!(bombs.is_ok(), "get_trade_cluster_bombs failed: {bombs:?}");
        assert!(bombs.unwrap().data.is_empty());
    }

    #[tokio::test]
    async fn volumeleaders_reauth_retries_once() {
        // Initial client talks to expired_server, which always returns session-expired.
        let mut expired_server = mockito::Server::new_async().await;
        let _expired_mock = expired_server
            .mock("POST", "/Trades/GetTrades")
            .with_status(200)
            .with_body(r#"<html><form><input type="password" /></form></html>"#)
            .create_async()
            .await;

        // After re-auth, the new client talks to success_server.
        let mut success_server = mockito::Server::new_async().await;
        let _success_mock = success_server
            .mock("POST", "/Trades/GetTrades")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(datatables_body::<Trade>(vec![]))
            .create_async()
            .await;

        let login_count = Arc::new(AtomicUsize::new(0));
        let counter = login_count.clone();
        let success_url = success_server.url();
        let login_fn: LoginFn = Arc::new(move |_u, _p| {
            counter.fetch_add(1, Ordering::SeqCst);
            let url = success_url.clone();
            Box::pin(async move {
                Client::with_config(
                    test_session(),
                    ClientConfig {
                        base_url: url,
                        ..ClientConfig::default()
                    },
                )
            })
        });

        let manager = VolumeLeadersManager::with_client_and_login(
            test_client_for(&expired_server),
            "user".into(),
            "pass".into(),
            login_fn,
        );

        let result = manager.get_trades(&TradesRequest::default()).await;
        assert!(result.is_ok(), "retry should succeed: {result:?}");
        assert_eq!(
            login_count.load(Ordering::SeqCst),
            1,
            "login should be called exactly once for re-auth"
        );
    }

    #[tokio::test]
    async fn volumeleaders_forbidden_status_reauth_retries_once() {
        let mut forbidden_server = mockito::Server::new_async().await;
        let _forbidden_mock = forbidden_server
            .mock("POST", "/Trades/GetTrades")
            .with_status(403)
            .with_body("forbidden")
            .create_async()
            .await;

        let mut success_server = mockito::Server::new_async().await;
        let _success_mock = success_server
            .mock("POST", "/Trades/GetTrades")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(datatables_body::<Trade>(vec![]))
            .create_async()
            .await;

        let login_count = Arc::new(AtomicUsize::new(0));
        let counter = login_count.clone();
        let success_url = success_server.url();
        let login_fn: LoginFn = Arc::new(move |_u, _p| {
            counter.fetch_add(1, Ordering::SeqCst);
            let url = success_url.clone();
            Box::pin(async move {
                Client::with_config(
                    test_session(),
                    ClientConfig {
                        base_url: url,
                        ..ClientConfig::default()
                    },
                )
            })
        });

        let manager = VolumeLeadersManager::with_client_and_login(
            test_client_for(&forbidden_server),
            "user".into(),
            "pass".into(),
            login_fn,
        );

        let result = manager.get_trades(&TradesRequest::default()).await;
        assert!(result.is_ok(), "403 retry should succeed: {result:?}");
        assert_eq!(
            login_count.load(Ordering::SeqCst),
            1,
            "login should be called exactly once for 403 re-auth"
        );
    }

    #[tokio::test]
    async fn volumeleaders_reauth_fails_after_retry() {
        // Both initial and post-reauth clients hit the same server that always
        // returns session-expired HTML, so the retry also fails.
        let mut server = mockito::Server::new_async().await;
        let _expired_mock = server
            .mock("POST", "/Trades/GetTrades")
            .with_status(200)
            .with_body(r#"<html><form><input type="password" /></form></html>"#)
            .create_async()
            .await;

        let server_url = server.url();
        let login_fn: LoginFn = Arc::new(move |_u, _p| {
            let url = server_url.clone();
            Box::pin(async move {
                Client::with_config(
                    test_session(),
                    ClientConfig {
                        base_url: url,
                        ..ClientConfig::default()
                    },
                )
            })
        });

        let manager = VolumeLeadersManager::with_client_and_login(
            test_client_for(&server),
            "user".into(),
            "pass".into(),
            login_fn,
        );

        let result = manager.get_trades(&TradesRequest::default()).await;
        assert!(result.is_err(), "should fail after retry exhausted");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("session"),
            "error should mention session: {err_msg}"
        );
    }
}
