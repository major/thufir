//! Session-backed client manager for the VolumeLeaders API.
//!
//! Wraps [`rusty_volumeleaders::Client`] with automatic re-authentication
//! on session expiry. The manager detects expired sessions and retries
//! login exactly once before surfacing the error.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use rusty_volumeleaders::{
    Client, ClientConfig, ClientError, DataTablesResponse, Trade, TradeCluster, TradeClusterBomb,
    TradeClusterBombsRequest, TradeClustersRequest, TradeLevel, TradeLevelsRequest, TradesRequest,
};
use tokio::sync::RwLock;

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
/// re-authenticate when the server reports session expiry.
pub struct VolumeLeadersManager {
    username: String,
    password: String,
    client: Arc<RwLock<Client>>,
    login_fn: LoginFn,
}

/// Call a dashboard method on the inner client, retrying login exactly once
/// on session expiry. Avoids lifetime issues with async closures by expanding
/// the retry logic inline.
macro_rules! call_with_retry {
    ($self:expr, $method:ident, $request:expr) => {{
        let result = {
            let client = $self.client.read().await;
            client.$method($request).await
        };
        match result {
            Ok(value) => Ok(value),
            Err(e) if e.is_session_expired() => {
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
    /// Create a new manager by logging in with the given credentials.
    ///
    /// Performs an initial login to establish the session, then stores
    /// the credentials for future re-authentication attempts.
    pub async fn new(username: String, password: String) -> crate::Result<Self> {
        let config = ClientConfig::default();
        let session = rusty_volumeleaders::login(&username, &password)
            .await
            .map_err(|e| crate::Error::VolumeLeaders(e.to_string()))?;
        let client = Client::with_config(session, config)
            .map_err(|e| crate::Error::VolumeLeaders(e.to_string()))?;

        let login_fn: LoginFn = Arc::new(|u, p| {
            Box::pin(async move {
                let session = rusty_volumeleaders::login(&u, &p).await?;
                Client::with_config(session, ClientConfig::default())
            })
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
