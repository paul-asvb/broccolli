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

    let link_class = "text-sm font-medium text-gray-600 hover:text-gray-900";

    html! {
        <header class="border-b border-gray-200 bg-white">
            <nav class="mx-auto flex max-w-5xl items-center gap-6 px-4 py-3">
                <span class="text-lg font-semibold text-gray-900">{ "broccolli" }</span>
                <Link<Route> to={Route::Messages} classes={link_class}>{ "Messages" }</Link<Route>>
                <Link<Route> to={Route::Processing} classes={link_class}>{ "Processing" }</Link<Route>>
                <button
                    onclick={logout}
                    class="ml-auto rounded-md border border-gray-300 px-3 py-1.5 text-sm font-medium text-gray-700 hover:bg-gray-100"
                >
                    { "Log out" }
                </button>
            </nav>
        </header>
    }
}
