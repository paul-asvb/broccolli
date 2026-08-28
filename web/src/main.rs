mod header;
mod message_detail;
mod messages;
mod processing;

use header::Header;
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
        Route::Home => html! { <h1>{ "Hello from broccolli!" }</h1> },
        Route::Messages => html! { <MessagesPage /> },
        Route::Message { chat_id, message_id } => {
            html! { <MessageDetailPage chat_id={chat_id} message_id={message_id} /> }
        }
        Route::Processing => html! { <ProcessingPage /> },
        Route::NotFound => html! { <h1>{ "404" }</h1> },
    }
}

#[function_component(App)]
fn app() -> Html {
    html! {
        <BrowserRouter>
            <Header />
            <main>
                <Switch<Route> render={switch} />
            </main>
        </BrowserRouter>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
