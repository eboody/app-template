use crate::web_config;
use axum::{
	async_trait,
	extract::{FromRef, FromRequestParts},
	http::{request::Parts, StatusCode},
	response::{Html, IntoResponse, Response},
	routing::get,
	Router,
};
use minijinja::{path_loader, Environment};
use minijinja_autoreload::AutoReloader;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
struct AppState {
	reloader: Arc<RwLock<AutoReloader>>,
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

async fn handler(TemplateEnv(env): TemplateEnv) -> Html<String> {
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

pub async fn styles() -> impl IntoResponse {
	let styles = std::fs::read_to_string(format!(
		"{}public/styles.css",
		&web_config().WEB_FOLDER
	))
	.unwrap();

	let response = Response::builder()
		.status(StatusCode::OK)
		.header("Content-Type", "text/css")
		.body(styles)
		.unwrap();

	response
}

pub fn template_router() -> Router {
	let reloader = AutoReloader::new(|notifier| {
		let template_path = format!("{}templates", &web_config().WEB_FOLDER);
		let mut env = Environment::new();
		if !cfg!(debug_assertions) {
			println!("Loading embedded templates...");
			minijinja_embed::load_templates!(&mut env);
		} else {
			println!("Setting up path loader and auto-reloader...");
			env.set_loader(path_loader(&template_path));
			notifier.watch_path(&template_path, true);
		}
		Ok(env)
	});

	let state = AppState {
		reloader: Arc::new(RwLock::new(reloader)),
	};
	Router::new()
		.route("/", get(handler))
		.route("/styles.css", get(styles))
		.with_state(state)
}
