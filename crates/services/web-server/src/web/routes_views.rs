use axum::{
	async_trait,
	body::Body,
	extract::{FromRef, FromRequestParts, Request, State},
	http::{header, request::Parts, HeaderValue},
	middleware::{self, Next},
	response::{Html, IntoResponse, Redirect},
	routing::{get, post},
	Form, Json, Router,
};
use axum_htmx::HxRequest;
use lib_core::model::ModelManager;
use minijinja::{path_loader, Environment};
use minijinja_autoreload::AutoReloader;
use serde::Deserialize;
use std::sync::{Arc, RwLock};
use tower_cookies::Cookies;
use tracing::debug;

use crate::web::{mw_auth::CtxExtError, AUTH_TOKEN};

use super::{
	mw_auth::ctx_resolve,
	routes_login::{self, api_login_handler},
};

#[derive(Clone)]
struct AppState {
	reloader: Arc<RwLock<AutoReloader>>,
	mm: ModelManagerW,
}

struct TemplateEnv(Environment<'static>);

impl TemplateEnv {
	fn new(env: Environment<'static>) -> Self {
		TemplateEnv(env)
	}
}

#[async_trait]
impl<S> FromRequestParts<S> for TemplateEnv
where
	S: Send + Sync,
	AppState: FromRef<S>,
{
	type Rejection = &'static str;

	async fn from_request_parts(
		_: &mut Parts,
		state: &S,
	) -> Result<Self, Self::Rejection> {
		let state = AppState::from_ref(state);
		let reloader = state.reloader.read().map_err(|_| "Lock Poisoned")?;
		let env = reloader
			.acquire_env()
			.map_err(|_| "Failed to acquire environment")?;
		Ok(TemplateEnv::new(env.clone()))
	}
}

#[derive(Clone)]
pub struct ModelManagerW(pub ModelManager);

#[async_trait]
impl<S> FromRequestParts<S> for ModelManagerW
where
	S: Send + Sync,
	AppState: FromRef<S>,
{
	type Rejection = &'static str;

	async fn from_request_parts(
		_: &mut Parts,
		state: &S,
	) -> Result<Self, Self::Rejection> {
		let state = AppState::from_ref(state);
		let mm = state.mm;
		Ok(mm)
	}
}

pub async fn mw_protected_page(
	cookies: Cookies,
	req: Request<Body>,
	next: Next,
) -> crate::web::Result<axum::response::Response> {
	debug!("{:<12} - mw_protected_page - {req:?}", "MIDDLEWARE");

	//If user navigated to a protected page without a valid token then redirect them to /
	let token = cookies
		.get(AUTH_TOKEN)
		.map(|c| c.value().to_string())
		.ok_or(CtxExtError::TokenNotInCookie);

	let fetch_mode = req.headers().get("sec-fetch-mode");

	if matches!(token, Err(CtxExtError::TokenNotInCookie)) {
		println!("{:#?}", req.headers());
		if let Some(fetch_mode) = fetch_mode {
			if fetch_mode == "navigate" {
				return Ok(Redirect::to("/").into_response());
			}
		}
	}
	Ok(next.run(req).await)
}

async fn home(TemplateEnv(env): TemplateEnv) -> Html<String> {
	let tmpl = env.get_template("index.html").unwrap();

	let rendered = tmpl
		.render(minijinja::context! {
			title => "My Page",
			heading => "Hello, world!",
			content => "This is a paragraph."
		})
		.unwrap();

	Html(rendered)
}

fn render_login_form(env: Environment<'static>) -> Html<String> {
	let tmpl = env.get_template("pages/login.html").unwrap();

	let rendered = tmpl
		.render(minijinja::context! {
			error => "Invalid username or password".to_owned()
		})
		.unwrap();

	Html(rendered)
}

#[derive(Deserialize)]
struct LoginPayload {
	username: Option<String>,
	pwd: Option<String>,
}

// region:    --- HTMX Login
async fn htmx_login_handler(
	TemplateEnv(env): TemplateEnv,
	ModelManagerW(mm): ModelManagerW,
	cookies: Cookies,
	HxRequest(_): HxRequest, // Use HxRequest extractor to ensure this is an HTMX request
	Form(payload): Form<LoginPayload>,
) -> impl IntoResponse {
	let redirect = || {
		let mut response = ().into_response();

		response.headers_mut().insert(
			header::HeaderName::from_static("hx-redirect"),
			HeaderValue::from_static("/dashboard"),
		);

		response
	};

	let ctx_resolve = ctx_resolve(mm.clone(), &cookies).await;

	if ctx_resolve.is_ok() {
		return redirect();
	}

	if payload.username.is_some() && payload.pwd.is_some() {
		let login_response = api_login_handler(
			State(mm),
			cookies,
			Json(routes_login::LoginPayload {
				username: payload.username.unwrap(),
				pwd: payload.pwd.unwrap(),
			}),
		)
		.await;

		if login_response.is_ok() {
			return redirect();
		} else {
			render_login_form(env).into_response()
		}
	} else {
		render_login_form(env).into_response()
	}
}

async fn dashboard(TemplateEnv(env): TemplateEnv) -> impl IntoResponse {
	let tmpl = env.get_template("pages/dashboard.html").unwrap();

	let rendered = tmpl.render(minijinja::context! {}).unwrap();

	Html(rendered)
}

pub fn routes(mm: ModelManager) -> Router {
	let reloader = AutoReloader::new(|notifier| {
		let template_path = format!(
			"{}/src/templates",
			std::env::var("CARGO_MANIFEST_DIR").unwrap()
		);

		let mut env = Environment::new();

		if cfg!(debug_assertions) {
			println!("Setting up path loader and auto-reloader...");
			env.set_loader(path_loader(&template_path));
			notifier.watch_path(&template_path, true);
		} else {
			println!("Loading embedded templates...");
			minijinja_embed::load_templates!(&mut env);
		}

		Ok(env)
	});

	let state = AppState {
		reloader: Arc::new(RwLock::new(reloader)),
		mm: ModelManagerW(mm),
	};

	let protected_routes = Router::new()
		.route("/dashboard", get(dashboard))
		.route_layer(middleware::from_fn(mw_protected_page));

	Router::new()
		.route("/", get(home))
		.route("/login", post(htmx_login_handler))
		.merge(protected_routes)
		.with_state(state)
}
