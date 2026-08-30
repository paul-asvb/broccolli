use gloo_net::http::Request;
use gloo_timers::callback::Interval;
use js_sys::Date;
use serde::Deserialize;
use wasm_bindgen::JsValue;
use yew::prelude::*;

const POLL_INTERVAL_MS: u32 = 5_000;

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
struct ProcessingEntry {
    chat_id: i64,
    message_id: i64,
    updated_at: i64,
}

#[derive(Deserialize, Clone, PartialEq)]
struct ProcessingSummary {
    counts: Vec<StatusCount>,
    currently_processing: Vec<ProcessingEntry>,
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
            let fetch_now = {
                let data = data.clone();
                move || {
                    let data = data.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        data.set(fetch_summary().await);
                    });
                }
            };

            fetch_now();
            let interval = Interval::new(POLL_INTERVAL_MS, fetch_now);

            move || drop(interval)
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
        return html! { <p class="text-gray-500">{ "loading..." }</p> };
    };

    let section_class = "rounded-lg border border-gray-200 bg-white p-4";
    let heading_class = "mb-3 text-sm font-semibold uppercase tracking-wide text-gray-500";
    let list_item_class = "border-b border-gray-100 py-2 text-sm last:border-0";
    let link_class = "font-medium text-gray-900 hover:underline";

    html! {
        <div class="space-y-6">
            <div class="flex items-center justify-between">
                <h2 class="text-lg font-semibold text-gray-900">{ "Processing status" }</h2>
                <button
                    onclick={refresh}
                    class="rounded-md border border-gray-300 px-3 py-1.5 text-sm font-medium text-gray-700 hover:bg-gray-100"
                >
                    { "Refresh" }
                </button>
            </div>

            <div class={section_class}>
                <h3 class={heading_class}>{ "By status" }</h3>
                <div class="flex flex-wrap gap-2">
                    { for data.counts.iter().map(|c| html! {
                        <span class="rounded-full bg-gray-100 px-3 py-1 text-sm text-gray-700">
                            { format!("{}: {}", c.status, c.count) }
                        </span>
                    }) }
                </div>
            </div>

            <div class={section_class}>
                <h3 class={heading_class}>{ "Currently processing" }</h3>
                { if data.currently_processing.is_empty() {
                    html! { <p class="text-sm text-gray-500">{ "nothing processing right now" }</p> }
                } else {
                    html! {
                        <ul>
                            { for data.currently_processing.iter().map(|p| html! {
                                <li class={list_item_class}>
                                    <a href={format!("/messages/{}/{}", p.chat_id, p.message_id)} class={link_class}>
                                        { format!("chat {} message {}", p.chat_id, p.message_id) }
                                    </a>
                                    <span class="text-gray-500">
                                        { format!(" — started: {}", format_date(p.updated_at)) }
                                    </span>
                                </li>
                            }) }
                        </ul>
                    }
                } }
            </div>

            <div class={section_class}>
                <h3 class={heading_class}>{ "Recently processed" }</h3>
                { if data.recent_processed.is_empty() {
                    html! { <p class="text-sm text-gray-500">{ "nothing processed yet" }</p> }
                } else {
                    html! {
                        <ul>
                            { for data.recent_processed.iter().map(|p| html! {
                                <li class={list_item_class}>
                                    <a href={format!("/messages/{}/{}", p.chat_id, p.message_id)} class={link_class}>
                                        { format!("chat {} message {}", p.chat_id, p.message_id) }
                                    </a>
                                    <span class="text-gray-500">
                                        { format!(
                                            " — category: {}, needs_followup: {}, processed: {}",
                                            p.category, p.needs_followup, format_date(p.created_at)
                                        ) }
                                    </span>
                                </li>
                            }) }
                        </ul>
                    }
                } }
            </div>

            <div class={section_class}>
                <h3 class={heading_class}>{ "Recent errors" }</h3>
                { if data.recent_errors.is_empty() {
                    html! { <p class="text-sm text-gray-500">{ "no errors" }</p> }
                } else {
                    html! {
                        <ul>
                            { for data.recent_errors.iter().map(|e| html! {
                                <li class={list_item_class}>
                                    <a href={format!("/messages/{}/{}", e.chat_id, e.message_id)} class={link_class}>
                                        { format!("chat {} message {}", e.chat_id, e.message_id) }
                                    </a>
                                    <span class="text-gray-500">
                                        { format!(" — attempts: {}, updated: {}", e.attempts, format_date(e.updated_at)) }
                                    </span>
                                    { if let Some(error) = &e.error {
                                        html! { <div class="mt-1 text-sm text-red-600">{ format!("error: {error}") }</div> }
                                    } else {
                                        html! {}
                                    } }
                                </li>
                            }) }
                        </ul>
                    }
                } }
            </div>
        </div>
    }
}
