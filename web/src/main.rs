mod header;
mod login;
mod message_detail;
mod messages;
mod processing;

use gloo_net::http::Request;
use header::Header;
use login::LoginPage;
use message_detail::MessageDetailPage;
use messages::MessagesPage;
use processing::ProcessingPage;
use yew::prelude::*;
use yew_router::prelude::*;

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/messages")]
    Messages,
    #[at("/messages/:chat_id/:message_id")]
    Message { chat_id: i64, message_id: i64 },
    #[at("/processing")]
    Processing,
    #[not_found]
    #[at("/404")]
    NotFound,
}

fn switch(route: Route) -> Html {
    match route {
        Route::Home => html! { <Redirect<Route> to={Route::Messages} /> },
        Route::Messages => html! { <MessagesPage /> },
        Route::Message { chat_id, message_id } => {
            html! { <MessageDetailPage chat_id={chat_id} message_id={message_id} /> }
        }
        Route::Processing => html! { <ProcessingPage /> },
        Route::NotFound => html! { <h1>{ "404" }</h1> },
    }
}

#[derive(Clone, PartialEq)]
enum AuthState {
    Checking,
    Authenticated,
    Unauthenticated,
}

#[function_component(App)]
fn app() -> Html {
    let auth = use_state(|| AuthState::Checking);

    {
        let auth = auth.clone();
        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                let authenticated = Request::get("/api/session")
                    .send()
                    .await
                    .map(|resp| resp.ok())
                    .unwrap_or(false);
                auth.set(if authenticated {
                    AuthState::Authenticated
                } else {
                    AuthState::Unauthenticated
                });
            });
            || ()
        });
    }

    match *auth {
        AuthState::Checking => html! { <p>{ "loading..." }</p> },
        AuthState::Unauthenticated => {
            let auth = auth.clone();
            let on_success = Callback::from(move |_| auth.set(AuthState::Authenticated));
            html! { <LoginPage on_success={on_success} /> }
        }
        AuthState::Authenticated => html! {
            <BrowserRouter>
                <Header />
                <main>
                    <Switch<Route> render={switch} />
                </main>
            </BrowserRouter>
        },
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
