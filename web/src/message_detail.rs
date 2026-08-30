use crate::linkify::linkify;
use gloo_net::http::Request;
use js_sys::Date;
use serde::Deserialize;
use wasm_bindgen::JsValue;
use yew::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub chat_id: i64,
    pub message_id: i64,
}

#[derive(Deserialize, Clone, PartialEq)]
struct Message {
    date_unixtime: i64,
    from_name: Option<String>,
    text: Option<String>,
}

#[derive(Deserialize, Clone, PartialEq)]
struct ProcessingState {
    status: String,
    attempts: i64,
    error: Option<String>,
    updated_at: i64,
}

#[derive(Deserialize, Clone, PartialEq)]
struct Analysis {
    category: String,
    needs_followup: bool,
    reasoning: Option<String>,
    model: String,
    created_at: i64,
}

#[derive(Deserialize, Clone, PartialEq)]
struct TiktokAnalysis {
    summary: String,
    on_screen_text: String,
    topics: Vec<String>,
    model: String,
    created_at: i64,
}

#[derive(Deserialize, Clone, PartialEq)]
struct WebArticleAnalysis {
    url: String,
    summary: String,
    model: String,
    created_at: i64,
}

#[derive(Deserialize, Clone, PartialEq, Default)]
struct MessageDetail {
    message: Option<Message>,
    processing: Option<ProcessingState>,
    analysis: Option<Analysis>,
    tiktok_analysis: Option<TiktokAnalysis>,
    web_article_analysis: Option<WebArticleAnalysis>,
}

fn format_date(unixtime: i64) -> String {
    let date = Date::new(&JsValue::from_f64((unixtime * 1000) as f64));
    String::from(date.to_locale_string("default", &JsValue::UNDEFINED))
}

async fn fetch_detail(chat_id: i64, message_id: i64) -> Option<MessageDetail> {
    let url = format!("/api/messages/{chat_id}/{message_id}");
    Request::get(&url).send().await.ok()?.json().await.ok()
}

#[function_component(MessageDetailPage)]
pub fn message_detail_page(props: &Props) -> Html {
    let chat_id = props.chat_id;
    let message_id = props.message_id;

    let data = use_state(|| None::<MessageDetail>);
    let enqueueing = use_state(|| false);

    {
        let data = data.clone();
        use_effect_with((chat_id, message_id), move |(chat_id, message_id)| {
            let data = data.clone();
            let chat_id = *chat_id;
            let message_id = *message_id;
            wasm_bindgen_futures::spawn_local(async move {
                data.set(fetch_detail(chat_id, message_id).await);
            });
            || ()
        });
    }

    let start_processing = {
        let data = data.clone();
        let enqueueing = enqueueing.clone();
        Callback::from(move |_| {
            let data = data.clone();
            let enqueueing = enqueueing.clone();
            enqueueing.set(true);
            wasm_bindgen_futures::spawn_local(async move {
                let url = format!("/api/messages/{chat_id}/{message_id}/process");
                let _ = Request::post(&url).send().await;
                data.set(fetch_detail(chat_id, message_id).await);
                enqueueing.set(false);
            });
        })
    };

    let Some(data) = (*data).clone() else {
        return html! { <p class="text-gray-500">{ "loading..." }</p> };
    };

    let Some(message) = data.message else {
        return html! { <p class="text-gray-500">{ "message not found" }</p> };
    };

    let section_class = "rounded-lg border border-gray-200 bg-white p-4";
    let heading_class = "mb-3 text-sm font-semibold uppercase tracking-wide text-gray-500";
    let list_class = "space-y-1 text-sm text-gray-700";

    html! {
        <div class="space-y-6">
            <div>
                <h2 class="text-lg font-semibold text-gray-900">{ "Message" }</h2>
                <p class="mt-1 text-sm text-gray-500">
                    <span class="font-medium text-gray-900">{ format_date(message.date_unixtime) }</span>
                    { " — " }
                    { message.from_name.unwrap_or_else(|| "?".to_string()) }
                </p>
                <p class="mt-2 whitespace-pre-wrap text-gray-800">{ linkify(message.text.as_deref().unwrap_or_default()) }</p>
            </div>

            <button
                onclick={start_processing}
                disabled={*enqueueing}
                class="rounded-md bg-gray-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-gray-700 disabled:opacity-50"
            >
                { if *enqueueing { "starting..." } else { "Start processing" } }
            </button>

            <div class={section_class}>
                <h3 class={heading_class}>{ "Processing status" }</h3>
                { match data.processing {
                    Some(p) => html! {
                        <ul class={list_class}>
                            <li>{ format!("status: {}", p.status) }</li>
                            <li>{ format!("attempts: {}", p.attempts) }</li>
                            <li>{ format!("updated: {}", format_date(p.updated_at)) }</li>
                            { if let Some(error) = p.error {
                                html! { <li class="text-red-600">{ format!("error: {error}") }</li> }
                            } else {
                                html! {}
                            } }
                        </ul>
                    },
                    None => html! { <p class="text-sm text-gray-500">{ "not queued yet" }</p> },
                } }
            </div>

            <div class={section_class}>
                <h3 class={heading_class}>{ "Analysis" }</h3>
                { match data.analysis {
                    Some(a) => html! {
                        <ul class={list_class}>
                            <li>{ format!("category: {}", a.category) }</li>
                            <li>{ format!("needs follow-up: {}", a.needs_followup) }</li>
                            <li>{ format!("model: {}", a.model) }</li>
                            <li>{ format!("analyzed: {}", format_date(a.created_at)) }</li>
                            { if let Some(reasoning) = a.reasoning {
                                html! { <li>{ format!("reasoning: {reasoning}") }</li> }
                            } else {
                                html! {}
                            } }
                        </ul>
                    },
                    None => html! { <p class="text-sm text-gray-500">{ "no analysis yet" }</p> },
                } }
            </div>

            <div class={section_class}>
                <h3 class={heading_class}>{ "TikTok summary" }</h3>
                { match data.tiktok_analysis {
                    Some(t) => html! {
                        <div>
                            <p class="text-gray-800">{ t.summary }</p>
                            <ul class={classes!(list_class, "mt-2")}>
                                <li>{ format!("on-screen text: {}", t.on_screen_text) }</li>
                                <li>{ format!("topics: {}", t.topics.join(", ")) }</li>
                                <li>{ format!("model: {}", t.model) }</li>
                                <li>{ format!("analyzed: {}", format_date(t.created_at)) }</li>
                            </ul>
                        </div>
                    },
                    None => html! { <p class="text-sm text-gray-500">{ "no tiktok summary yet" }</p> },
                } }
            </div>

            <div class={section_class}>
                <h3 class={heading_class}>{ "Web article summary" }</h3>
                { match data.web_article_analysis {
                    Some(w) => html! {
                        <div>
                            <p class="text-gray-800">{ w.summary }</p>
                            <ul class={classes!(list_class, "mt-2")}>
                                <li>
                                    <a href={w.url.clone()} target="_blank" rel="noopener noreferrer" class="text-gray-900 hover:underline">
                                        { w.url }
                                    </a>
                                </li>
                                <li>{ format!("model: {}", w.model) }</li>
                                <li>{ format!("analyzed: {}", format_date(w.created_at)) }</li>
                            </ul>
                        </div>
                    },
                    None => html! { <p class="text-sm text-gray-500">{ "no web article summary yet" }</p> },
                } }
            </div>
        </div>
    }
}
