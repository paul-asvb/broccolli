use gloo_net::http::Request;
use serde::Serialize;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[derive(Serialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub on_success: Callback<()>,
}

#[function_component(LoginPage)]
pub fn login_page(props: &Props) -> Html {
    let username = use_state(String::new);
    let password = use_state(String::new);
    let error = use_state(|| None::<String>);
    let submitting = use_state(|| false);

    let on_username_input = {
        let username = username.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            username.set(input.value());
        })
    };
    let on_password_input = {
        let password = password.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            password.set(input.value());
        })
    };

    let on_submit = {
        let username = username.clone();
        let password = password.clone();
        let error = error.clone();
        let submitting = submitting.clone();
        let on_success = props.on_success.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let username = (*username).clone();
            let password = (*password).clone();
            let error = error.clone();
            let submitting = submitting.clone();
            let on_success = on_success.clone();
            submitting.set(true);
            spawn_local(async move {
                let body = LoginRequest { username, password };
                let result = match Request::post("/api/login").json(&body) {
                    Ok(req) => req.send().await,
                    Err(_) => {
                        submitting.set(false);
                        error.set(Some("failed to build login request".to_string()));
                        return;
                    }
                };
                submitting.set(false);
                match result {
                    Ok(resp) if resp.ok() => {
                        error.set(None);
                        on_success.emit(());
                    }
                    Ok(_) => error.set(Some("invalid username or password".to_string())),
                    Err(_) => error.set(Some("login request failed".to_string())),
                }
            });
        })
    };

    let input_class =
        "w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-gray-500 focus:outline-none";

    html! {
        <div class="flex min-h-screen items-center justify-center bg-gray-50">
            <div class="w-full max-w-sm rounded-lg border border-gray-200 bg-white p-6 shadow-sm">
                <h2 class="mb-4 text-lg font-semibold text-gray-900">{ "Log in" }</h2>
                <form onsubmit={on_submit} class="space-y-4">
                    <div>
                        <label for="username" class="mb-1 block text-sm font-medium text-gray-700">{ "Username" }</label>
                        <input id="username" type="text" class={input_class} value={(*username).clone()} oninput={on_username_input} />
                    </div>
                    <div>
                        <label for="password" class="mb-1 block text-sm font-medium text-gray-700">{ "Password" }</label>
                        <input id="password" type="password" class={input_class} value={(*password).clone()} oninput={on_password_input} />
                    </div>
                    { if let Some(err) = &*error {
                        html! { <p class="text-sm text-red-600">{ err }</p> }
                    } else {
                        html! {}
                    } }
                    <button
                        type="submit"
                        disabled={*submitting}
                        class="w-full rounded-md bg-gray-900 px-3 py-2 text-sm font-medium text-white hover:bg-gray-700 disabled:opacity-50"
                    >
                        { if *submitting { "logging in..." } else { "Log in" } }
                    </button>
                </form>
            </div>
        </div>
    }
}
