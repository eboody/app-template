use axum::{
	async_trait,
	extract::{FromRef, FromRequestParts},
	http::request::Parts,
	response::Html,
	routing::get,
	Router,
};
use minijinja::{context, path_loader, Environment};
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

async fn some_name(TemplateEnv(env): TemplateEnv) -> Html<String> {
	let tmpl = env.get_template("components/code-block.html").unwrap();

	let rendered = tmpl.render(context! {}).unwrap();

	Html(rendered)
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

async fn test_component(TemplateEnv(env): TemplateEnv) -> Html<String> {
	let tmpl = env.get_template("components/test-component.html").unwrap();

	let rendered = tmpl.render(context! {}).unwrap();

	Html(rendered)
}

async fn loop_component(TemplateEnv(env): TemplateEnv) -> Html<String> {
	let template = env
		.template_from_str(
			r#"
<ul>
{% for item in items %}
  <li>
    Index: {{ loop.index }}<br>
    Zero-based index: {{ loop.index0 }}<br>
    From end: {{ loop.revindex }}<br>
    From end zero-based: {{ loop.revindex0 }}<br>
    First item: {{ loop.first }}<br>
    Last item: {{ loop.last }}<br>
    Length of items: {{ loop.length }}
  </li>
{% endfor %}
</ul>
"#,
		)
		.unwrap();

	let items = vec!["Item 1", "Item 2", "Item 3"];
	let ctx = context! {
		items => items
	};

	let rendered = template.render(ctx).unwrap();

	Html(rendered)
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

pub fn routes() -> Router {
	let reloader = AutoReloader::new(|notifier| {
		let template_path = format!(
			"{}/src/templates",
			std::env::var("CARGO_MANIFEST_DIR").unwrap()
		);
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
		.route("/", get(home))
		.route("/test_component", get(test_component))
		.route("/loop_component", get(loop_component))
		.with_state(state)
}
