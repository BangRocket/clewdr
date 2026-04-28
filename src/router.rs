use axum::{
    Router,
    body::Body,
    extract::{DefaultBodyLimit, Request, State},
    http::Method,
    middleware::{from_extractor, map_response},
    response::Response,
    routing::{delete, get, post},
};
use tower::{ServiceBuilder, ServiceExt};
use tower_http::{compression::CompressionLayer, cors::CorsLayer};

use crate::{
    api::*,
    config::{CLEWDR_CONFIG, OaiBackend},
    middleware::{
        RequireAdminAuth, RequireBearerAuth, RequireFlexibleAuth,
        claude::{add_usage_info, apply_stop_sequences, check_overloaded, to_oai},
    },
    providers::claude::ClaudeProviders,
    services::cookie_actor::CookieActorHandle,
};

/// RouterBuilder for the application
pub struct RouterBuilder {
    claude_providers: ClaudeProviders,
    cookie_actor_handle: CookieActorHandle,
    codex_auth_handle: crate::services::codex_auth_actor::CodexAuthActorHandle,
    codex_provider: std::sync::Arc<crate::providers::codex::CodexProvider>,
    inner: Router,
}

impl RouterBuilder {
    /// Creates a blank RouterBuilder instance
    /// Initializes the router with the provided application state
    ///
    /// # Arguments
    /// * `state` - The application state containing client information
    pub async fn new() -> Self {
        let cookie_handle = CookieActorHandle::start()
            .await
            .expect("Failed to start CookieActor");
        let claude_providers = crate::providers::claude::build_providers(cookie_handle.clone());
        let codex_auth_handle = crate::services::codex_auth_actor::CodexAuthActorHandle::start()
            .await
            .expect("Failed to start CodexAuthActor");
        let codex_provider =
            crate::providers::codex::build_codex_provider(codex_auth_handle.clone());
        RouterBuilder {
            claude_providers,
            cookie_actor_handle: cookie_handle,
            codex_auth_handle,
            codex_provider,
            inner: Router::new(),
        }
    }

    /// Creates a new RouterBuilder instance
    /// Sets up routes for API endpoints and static file serving
    pub fn with_default_setup(self) -> Self {
        self.route_claude_code_endpoints()
            .route_claude_web_endpoints()
            .route_admin_endpoints()
            .route_claude_web_oai_endpoints()
            .route_claude_code_oai_endpoints()
            .route_codex_oai_endpoints()
            .route_codex_admin_endpoints()
            .route_oai_dispatcher_endpoint()
            .setup_static_serving()
            .with_tower_trace()
            .with_cors()
    }

    /// Sets up routes for v1 endpoints
    fn route_claude_web_endpoints(mut self) -> Self {
        let router = Router::new()
            .route("/v1/messages", post(api_claude_web))
            .layer(
                ServiceBuilder::new()
                    .layer(from_extractor::<RequireFlexibleAuth>())
                    .layer(CompressionLayer::new())
                    .layer(map_response(add_usage_info))
                    .layer(map_response(apply_stop_sequences))
                    .layer(map_response(check_overloaded)),
            )
            .with_state(self.claude_providers.web());
        self.inner = self.inner.merge(router);
        self
    }

    /// Sets up routes for v1 endpoints
    fn route_claude_code_endpoints(mut self) -> Self {
        let router = Router::new()
            .route("/code/v1/messages", post(api_claude_code))
            .route(
                "/code/v1/messages/count_tokens",
                post(api_claude_code_count_tokens),
            )
            .layer(
                ServiceBuilder::new()
                    .layer(from_extractor::<RequireFlexibleAuth>())
                    .layer(CompressionLayer::new()),
            )
            .with_state(self.claude_providers.code());
        self.inner = self.inner.merge(router);
        self
    }

    /// Sets up routes for API endpoints
    fn route_admin_endpoints(mut self) -> Self {
        let cookie_router = Router::new()
            .route("/cookies", get(api_get_cookies))
            .route(
                "/cookie",
                delete(api_delete_cookie)
                    .post(api_post_cookie)
                    .put(api_put_cookie),
            )
            .with_state(self.cookie_actor_handle.to_owned());
        let admin_router = Router::new()
            .route("/auth", get(api_auth))
            .route("/config", get(api_get_config).post(api_post_config))
            .route("/usage/summary", get(usage::summary))
            .route(
                "/usage/cookie/{history_id}/events",
                get(usage::events),
            )
            .route(
                "/usage/cookie/{history_id}/timeseries",
                get(usage::timeseries),
            )
            .route(
                "/usage/cookie/{history_id}/snapshots",
                get(usage::snapshots),
            )
            .route("/usage/dead", get(usage::dead))
            .route("/usage/pricing", get(usage::pricing_meta))
            .route("/usage/prune", post(usage::prune_now));
        let router = Router::new()
            .nest(
                "/api",
                cookie_router
                    .merge(admin_router)
                    .layer(from_extractor::<RequireAdminAuth>()),
            )
            .route("/api/version", get(api_version));
        self.inner = self.inner.merge(router);
        self
    }

    /// Sets up routes for OpenAI compatible endpoints (Claude web).
    ///
    /// Note: `/v1/chat/completions` is intentionally NOT registered here. It is
    /// served by [`Self::route_oai_dispatcher_endpoint`], which forwards to either
    /// the Claude OAI pipeline or the Codex pipeline based on
    /// [`crate::config::ClewdrConfig::default_oai_backend`].
    fn route_claude_web_oai_endpoints(mut self) -> Self {
        let router = Router::new()
            .route("/v1/models", get(api_get_models))
            .layer(
                ServiceBuilder::new()
                    .layer(from_extractor::<RequireBearerAuth>())
                    .layer(CompressionLayer::new())
                    .layer(map_response(to_oai))
                    .layer(map_response(apply_stop_sequences))
                    .layer(map_response(check_overloaded)),
            )
            .with_state(self.claude_providers.web());
        self.inner = self.inner.merge(router);
        self
    }

    /// Builds the dispatcher route that serves the bare `/v1/chat/completions`
    /// endpoint. At request time, it consults
    /// [`crate::config::ClewdrConfig::default_oai_backend`] and forwards the
    /// request to either the Claude OAI sub-router or the Codex sub-router.
    ///
    /// Both sub-routers are pre-built here (mounted at `/v1/chat/completions`)
    /// with the same layer chains used by the dedicated path-based routes, so
    /// behavior matches `/v1/messages` (via OAI) and `/codex/v1/chat/completions`
    /// respectively. Bearer auth is enforced once on the dispatcher itself.
    fn route_oai_dispatcher_endpoint(mut self) -> Self {
        // Claude OAI sub-router: same layer chain as `route_claude_web_oai_endpoints`
        // minus the bearer auth (applied by the outer dispatcher).
        let claude_inner: Router = Router::new()
            .route("/v1/chat/completions", post(api_claude_web))
            .layer(
                ServiceBuilder::new()
                    .layer(CompressionLayer::new())
                    .layer(map_response(to_oai))
                    .layer(map_response(apply_stop_sequences))
                    .layer(map_response(check_overloaded)),
            )
            .with_state(self.claude_providers.web());

        // Codex sub-router: same layer chain as `route_codex_oai_endpoints` minus
        // the bearer auth.
        let codex_inner: Router = Router::new()
            .route("/v1/chat/completions", post(api_codex_chat))
            .layer(ServiceBuilder::new().layer(CompressionLayer::new()))
            .with_state(self.codex_provider.clone());

        let dispatch_state = OaiDispatchState {
            claude_inner,
            codex_inner,
        };

        let router = Router::new()
            .route("/v1/chat/completions", post(oai_dispatch_handler))
            .layer(from_extractor::<RequireBearerAuth>())
            .with_state(dispatch_state);

        self.inner = self.inner.merge(router);
        self
    }

    /// Sets up routes for OpenAI compatible endpoints
    fn route_claude_code_oai_endpoints(mut self) -> Self {
        let router = Router::new()
            .route("/code/v1/chat/completions", post(api_claude_code))
            .route("/code/v1/models", get(api_get_models))
            .layer(
                ServiceBuilder::new()
                    .layer(from_extractor::<RequireBearerAuth>())
                    .layer(CompressionLayer::new())
                    .layer(map_response(to_oai)),
            )
            .with_state(self.claude_providers.code());
        self.inner = self.inner.merge(router);
        self
    }

    /// Sets up routes for Codex OpenAI compatible endpoints
    fn route_codex_oai_endpoints(mut self) -> Self {
        let router = Router::new()
            .route("/codex/v1/chat/completions", post(api_codex_chat))
            .route("/codex/v1/models", get(api_codex_models))
            .layer(
                ServiceBuilder::new()
                    .layer(from_extractor::<RequireBearerAuth>())
                    .layer(CompressionLayer::new()),
            )
            .with_state(self.codex_provider.clone());
        self.inner = self.inner.merge(router);
        self
    }

    /// Sets up admin routes for managing Codex auth credentials
    fn route_codex_admin_endpoints(mut self) -> Self {
        let router = Router::new()
            .route("/codex/auth", get(api_codex_list).post(api_codex_add))
            .route("/codex/auth/{id}", delete(api_codex_delete))
            .with_state(self.codex_auth_handle.clone());
        let admin = Router::new()
            .nest("/api", router.layer(from_extractor::<RequireAdminAuth>()));
        self.inner = self.inner.merge(admin);
        self
    }

    /// Sets up static file serving
    fn setup_static_serving(mut self) -> Self {
        #[cfg(feature = "embed-resource")]
        {
            use include_dir::{Dir, include_dir};
            const INCLUDE_STATIC: Dir = include_dir!("$CARGO_MANIFEST_DIR/static");
            self.inner = self
                .inner
                .fallback_service(tower_serve_static::ServeDir::new(&INCLUDE_STATIC));
        }
        #[cfg(feature = "external-resource")]
        {
            use const_format::formatc;
            use tower_http::services::ServeDir;
            self.inner = self.inner.fallback_service(ServeDir::new(formatc!(
                "{}/static",
                env!("CARGO_MANIFEST_DIR")
            )));
        }
        self
    }

    /// Adds CORS support to the router
    fn with_cors(mut self) -> Self {
        use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
        use http::header::HeaderName;

        let cors = CorsLayer::new()
            .allow_origin(tower_http::cors::Any)
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers([
                AUTHORIZATION,
                CONTENT_TYPE,
                HeaderName::from_static("x-api-key"),
            ]);

        self.inner = self.inner.layer(cors);
        self
    }

    fn with_tower_trace(mut self) -> Self {
        use tower_http::trace::TraceLayer;

        let layer = TraceLayer::new_for_http();

        self.inner = self.inner.layer(layer);
        self
    }

    /// Returns the configured router
    /// Finalizes the router configuration for use with axum
    pub fn build(self) -> Router {
        self.inner.layer(DefaultBodyLimit::max(32 * 1024 * 1024))
    }
}

/// State for [`oai_dispatch_handler`] containing both pre-built sub-routers.
#[derive(Clone)]
struct OaiDispatchState {
    claude_inner: Router,
    codex_inner: Router,
}

/// Handler for the dispatched `/v1/chat/completions` route.
///
/// Reads `default_oai_backend` from the live config and forwards the request
/// (unchanged) to whichever inner Router was selected.
async fn oai_dispatch_handler(
    State(state): State<OaiDispatchState>,
    request: Request<Body>,
) -> Response {
    let backend = CLEWDR_CONFIG.load().default_oai_backend;
    let inner = match backend {
        OaiBackend::Claude => state.claude_inner,
        OaiBackend::Codex => state.codex_inner,
    };
    match inner.oneshot(request).await {
        Ok(resp) => resp,
        Err(infallible) => match infallible {},
    }
}
