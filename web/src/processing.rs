use gloo_net::http::Request;
use js_sys::Date;
use serde::Deserialize;
use wasm_bindgen::JsValue;
use yew::prelude::*;

#[derive(Deserialize, Clone, PartialEq)]
struct StatusCount {
    status: String,
    count: i64,
}

#[derive(Deserialize, Clone, PartialEq)]
struct ErrorEntry {
    chat_id: i64,
    message_id: i64,
    attempts: i64,
    error: Option<String>,
    updated_at: i64,
}

#[derive(Deserialize, Clone, PartialEq)]
struct RecentAnalysis {
    chat_id: i64,
    message_id: i64,
    category: String,
    needs_followup: bool,
    created_at: i64,
}

#[derive(Deserialize, Clone, PartialEq)]
struct ProcessingSummary {
    counts: Vec<StatusCount>,
    recent_errors: Vec<ErrorEntry>,
    recent_processed: Vec<RecentAnalysis>,
}

fn format_date(unixtime: i64) -> String {
    let date = Date::new(&JsValue::from_f64((unixtime * 1000) as f64));
    String::from(date.to_locale_string("default", &JsValue::UNDEFINED))
}

async fn fetch_summary() -> Option<ProcessingSummary> {
    Request::get("/api/processing").send().await.ok()?.json().await.ok()
}

#[function_component(ProcessingPage)]
pub fn processing_page() -> Html {
    let data = use_state(|| None::<ProcessingSummary>);

    {
        let data = data.clone();
        use_effect_with((), move |_| {
            let data = data.clone();
            wasm_bindgen_futures::spawn_local(async move {
                data.set(fetch_summary().await);
            });
            || ()
        });
    }

    let refresh = {
        let data = data.clone();
        Callback::from(move |_| {
            let data = data.clone();
            wasm_bindgen_futures::spawn_local(async move {
                data.set(fetch_summary().await);
            });
        })
    };

    let Some(data) = (*data).clone() else {
        return html! { <p>{ "loading..." }</p> };
    };

    html! {
        <div>
            <h2>{ "Processing status" }</h2>
            <button onclick={refresh}>{ "Refresh" }</button>

            <h3>{ "By status" }</h3>
            <ul>
                { for data.counts.iter().map(|c| html! {
                    <li>{ format!("{}: {}", c.status, c.count) }</li>
                }) }
            </ul>

            <h3>{ "Recently processed" }</h3>
            { if data.recent_processed.is_empty() {
                html! { <p>{ "nothing processed yet" }</p> }
            } else {
                html! {
                    <ul>
                        { for data.recent_processed.iter().map(|p| html! {
                            <li>
                                <a href={format!("/messages/{}/{}", p.chat_id, p.message_id)}>
                                    { format!("chat {} message {}", p.chat_id, p.message_id) }
                                </a>
                                { format!(
                                    " — category: {}, needs_followup: {}, processed: {}",
                                    p.category, p.needs_followup, format_date(p.created_at)
                                ) }
                            </li>
                        }) }
                    </ul>
                }
            } }

            <h3>{ "Recent errors" }</h3>
            { if data.recent_errors.is_empty() {
                html! { <p>{ "no errors" }</p> }
            } else {
                html! {
                    <ul>
                        { for data.recent_errors.iter().map(|e| html! {
                            <li>
                                <a href={format!("/messages/{}/{}", e.chat_id, e.message_id)}>
                                    { format!("chat {} message {}", e.chat_id, e.message_id) }
                                </a>
                                { format!(" — attempts: {}, updated: {}", e.attempts, format_date(e.updated_at)) }
                                { if let Some(error) = &e.error {
                                    html! { <div>{ format!("error: {error}") }</div> }
                                } else {
                                    html! {}
                                } }
                            </li>
                        }) }
                    </ul>
                }
            } }
        </div>
    }
}
