use crate::Route;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

#[function_component(Header)]
pub fn header() -> Html {
    let logout = Callback::from(move |_| {
        spawn_local(async move {
            let _ = Request::post("/api/logout").send().await;
            if let Some(window) = web_sys::window() {
                let _ = window.location().reload();
            }
        });
    });

    html! {
        <header>
            <nav>
                <Link<Route> to={Route::Home}>{ "Home" }</Link<Route>>
                { " · " }
                <Link<Route> to={Route::Messages}>{ "Messages" }</Link<Route>>
                { " · " }
                <Link<Route> to={Route::Processing}>{ "Processing" }</Link<Route>>
                { " · " }
                <button onclick={logout}>{ "Log out" }</button>
            </nav>
        </header>
    }
}
