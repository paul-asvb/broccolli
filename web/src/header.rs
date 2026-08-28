use crate::Route;
use yew::prelude::*;
use yew_router::prelude::*;

#[function_component(Header)]
pub fn header() -> Html {
    html! {
        <header>
            <nav>
                <Link<Route> to={Route::Home}>{ "Home" }</Link<Route>>
                { " · " }
                <Link<Route> to={Route::Messages}>{ "Messages" }</Link<Route>>
            </nav>
        </header>
    }
}
