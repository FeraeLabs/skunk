mod flows;
mod home;
mod settings;

use std::time::Duration;

use gloo_timers::future::sleep;
use leptos::{
    component,
    create_node_ref,
    html,
    view,
    DynAttrs,
    IntoView,
    Signal,
    SignalGet,
    SignalSet,
    WriteSignal,
};
use leptos_hotkeys::{
    provide_hotkeys_context,
    scopes,
    HotkeysContext,
};
use leptos_meta::{
    provide_meta_context,
    Html,
};
use leptos_router::{
    Route,
    Router,
    Routes,
};
use leptos_use::{
    use_color_mode,
    ColorMode,
    UseColorModeReturn,
};
use settings::SettingsRoutes;
use skunk_api_client::Client;
use url::Url;

use self::{
    flows::Flows,
    home::Home,
};
use crate::components::{
    command_menu::CommandMenu,
    dock::Dock,
    status_bar::StatusBar,
};

stylance::import_crate_style!(style, "src/app/app.module.scss");

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    bs_theme: Signal<&'static str>,
    theme_icon: Signal<&'static str>,
    mode: Signal<ColorMode>,
    set_mode: WriteSignal<ColorMode>,
}

impl Default for Theme {
    fn default() -> Self {
        let UseColorModeReturn { mode, set_mode, .. } = use_color_mode();
        let bs_theme = Signal::derive(move || {
            match mode.get() {
                ColorMode::Dark => "dark",
                _ => "light",
            }
        });
        let theme_icon = Signal::derive(move || {
            match mode.get() {
                ColorMode::Dark => "moon-stars-fill",
                _ => "sun-fill",
            }
        });
        Self {
            bs_theme,
            theme_icon,
            mode,
            set_mode,
        }
    }
}

impl Theme {
    pub fn toggle(&self) {
        let current = self.mode.get();
        let new = match current {
            ColorMode::Dark => ColorMode::Light,
            _ => ColorMode::Dark,
        };
        self.set_mode.set(new);
    }

    pub fn set(&self, mode: ColorMode) {
        self.set_mode.set(mode);
    }

    pub fn icon(&self) -> Signal<&'static str> {
        self.theme_icon
    }
}

fn base_url() -> Option<Url> {
    gloo_utils::document().base_uri().ok()??.parse().ok()
}

fn api_url() -> Option<Url> {
    let mut url = base_url()?;
    url.path_segments_mut().unwrap().push("api");
    Some(url)
}

#[derive(Clone, Debug)]
pub struct Context {
    pub theme: Theme,
    pub client: Client,
}

impl Context {
    pub fn provide() -> Self {
        let (client, connection) = Client::new(api_url().expect("Could not determine API url"));

        // poll the connection in a separate task
        leptos::spawn_local(async move {
            if let Err(e) = connection.await {
                tracing::error!("client connection failed: {e}");
            }
        });

        // reload page on hot-reload signal
        let mut hot_reload = client.hot_reload();
        leptos::spawn_local(async move {
            hot_reload.wait().await;
            tracing::info!("Reloading page in 2s");
            sleep(Duration::from_secs(2)).await;
            let _ = gloo_utils::window().location().reload();
        });

        let context = Self {
            theme: Theme::default(),
            client,
        };

        leptos::provide_context(context.clone());

        context
    }

    pub fn get() -> Self {
        leptos::expect_context()
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    // create app context
    let Context {
        theme: Theme { bs_theme, .. },
        ..
    } = Context::provide();

    let main_ref = create_node_ref::<html::Main>();
    let HotkeysContext { .. } = provide_hotkeys_context(main_ref, false, scopes!());

    view! {
        <Html
            attr:data-bs-theme=bs_theme
        />
        <Router>
            <div class=style::app>
                <Dock />
                <main class=style::main node_ref=main_ref>
                    <CommandMenu />
                    <Routes>
                        <Route path="/" view=Home />
                        <Route path="/flows" view=Flows />
                        <Route path="/filters" view=|| view!{ "TODO" } />
                        <SettingsRoutes />
                        <Route path="/*any" view=NotFound />
                    </Routes>
                </main>
            </div>
            <StatusBar />
        </Router>
    }
}

#[component]
fn NotFound() -> impl IntoView {
    view! {
        <div class="h-100 w-100 pt-3 px-4">
            <h1>"404 - Not found"</h1>
        </div>
    }
}