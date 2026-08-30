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

    html! {
        <div class="login">
            <h2>{ "Log in" }</h2>
            <form onsubmit={on_submit}>
                <p>
                    <label for="username">{ "Username" }</label><br/>
                    <input id="username" type="text" value={(*username).clone()} oninput={on_username_input} />
                </p>
                <p>
                    <label for="password">{ "Password" }</label><br/>
                    <input id="password" type="password" value={(*password).clone()} oninput={on_password_input} />
                </p>
                { if let Some(err) = &*error {
                    html! { <p class="error">{ err }</p> }
                } else {
                    html! {}
                } }
                <button type="submit" disabled={*submitting}>
                    { if *submitting { "logging in..." } else { "Log in" } }
                </button>
            </form>
        </div>
    }
}
