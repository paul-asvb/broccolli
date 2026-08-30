use gloo_net::http::Request;
use js_sys::Date;
use serde::Deserialize;
use wasm_bindgen::JsValue;
use yew::prelude::*;
use yew_router::prelude::*;

#[derive(Deserialize, PartialEq, Clone, Default)]
struct QueryParams {
    page: Option<i64>,
    per_page: Option<i64>,
}

#[derive(Deserialize, Clone, PartialEq)]
struct Message {
    chat_id: i64,
    message_id: i64,
    date_unixtime: i64,
    text: Option<String>,
}

fn format_date(unixtime: i64) -> String {
    let date = Date::new(&JsValue::from_f64((unixtime * 1000) as f64));
    String::from(date.to_locale_string("default", &JsValue::UNDEFINED))
}

#[derive(Deserialize, Clone, PartialEq)]
struct MessagesResponse {
    messages: Vec<Message>,
    page: i64,
    per_page: i64,
    total: i64,
}

#[function_component(MessagesPage)]
pub fn messages_page() -> Html {
    let location = use_location().unwrap();
    let query = location.query::<QueryParams>().unwrap_or_default();
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).max(1);

    let data = use_state(|| None::<MessagesResponse>);

    {
        let data = data.clone();
        use_effect_with((page, per_page), move |(page, per_page)| {
            let data = data.clone();
            let page = *page;
            let per_page = *per_page;
            wasm_bindgen_futures::spawn_local(async move {
                let url = format!("/api/messages?page={page}&per_page={per_page}");
                if let Ok(resp) = Request::get(&url).send().await {
                    if let Ok(parsed) = resp.json::<MessagesResponse>().await {
                        data.set(Some(parsed));
                    }
                }
            });
            || ()
        });
    }

    let Some(data) = (*data).clone() else {
        return html! { <p>{ "loading..." }</p> };
    };

    let total_pages = ((data.total + data.per_page - 1) / data.per_page.max(1)).max(1);
    let prev_href = format!(
        "/messages?page={}&per_page={}",
        (data.page - 1).max(1),
        data.per_page
    );
    let next_href = format!("/messages?page={}&per_page={}", data.page + 1, data.per_page);
    let page_href = |p: i64| format!("/messages?page={}&per_page={}", p, data.per_page);

    html! {
        <div>
            <h2>{ format!("Messages — page {} of {} ({} total)", data.page, total_pages, data.total) }</h2>
            <ul>
                { for data.messages.iter().map(|m| html! {
                    <li>
                        <a href={format!("/messages/{}/{}", m.chat_id, m.message_id)}>
                            <strong>{ format_date(m.date_unixtime) }</strong>
                        </a>
                        { ": " }
                        { m.text.clone().unwrap_or_default() }
                    </li>
                }) }
            </ul>
            <p>
                <a href={prev_href}>{ "« prev" }</a>
                { " " }
                { for (1..=total_pages).map(|p| html! {
                    <>
                        { if p == data.page {
                            html! { <strong>{ p }</strong> }
                        } else {
                            html! { <a href={page_href(p)}>{ p }</a> }
                        } }
                        { " " }
                    </>
                }) }
                <a href={next_href}>{ "next »" }</a>
            </p>
        </div>
    }
}
