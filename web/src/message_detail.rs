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

#[derive(Deserialize, Clone, PartialEq, Default)]
struct MessageDetail {
    message: Option<Message>,
    processing: Option<ProcessingState>,
    analysis: Option<Analysis>,
}

#[derive(Deserialize, Clone, PartialEq)]
struct TiktokDownloadResult {
    path: String,
}

#[derive(Clone, PartialEq)]
enum TiktokDownloadState {
    Idle,
    Downloading,
    Done(String),
    Failed(String),
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
    let tiktok_download = use_state(|| TiktokDownloadState::Idle);

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

    let download_tiktok_video = {
        let tiktok_download = tiktok_download.clone();
        Callback::from(move |_| {
            let tiktok_download = tiktok_download.clone();
            tiktok_download.set(TiktokDownloadState::Downloading);
            wasm_bindgen_futures::spawn_local(async move {
                let url = format!("/api/messages/{chat_id}/{message_id}/download-tiktok");
                let result = match Request::post(&url).send().await {
                    Ok(resp) if resp.ok() => resp
                        .json::<TiktokDownloadResult>()
                        .await
                        .map(|r| r.path)
                        .map_err(|e| e.to_string()),
                    Ok(resp) => Err(resp.text().await.unwrap_or_else(|_| resp.status_text())),
                    Err(e) => Err(e.to_string()),
                };
                tiktok_download.set(match result {
                    Ok(path) => TiktokDownloadState::Done(path),
                    Err(err) => TiktokDownloadState::Failed(err),
                });
            });
        })
    };

    let Some(data) = (*data).clone() else {
        return html! { <p>{ "loading..." }</p> };
    };

    let Some(message) = data.message else {
        return html! { <p>{ "message not found" }</p> };
    };

    html! {
        <div>
            <h2>{ "Message" }</h2>
            <p>
                <strong>{ format_date(message.date_unixtime) }</strong>
                { " — " }
                { message.from_name.unwrap_or_else(|| "?".to_string()) }
            </p>
            <p>{ message.text.unwrap_or_default() }</p>

            <button onclick={start_processing} disabled={*enqueueing}>
                { if *enqueueing { "starting..." } else { "Start processing" } }
            </button>

            <h3>{ "Processing status" }</h3>
            { match data.processing {
                Some(p) => html! {
                    <ul>
                        <li>{ format!("status: {}", p.status) }</li>
                        <li>{ format!("attempts: {}", p.attempts) }</li>
                        <li>{ format!("updated: {}", format_date(p.updated_at)) }</li>
                        { if let Some(error) = p.error {
                            html! { <li>{ format!("error: {error}") }</li> }
                        } else {
                            html! {}
                        } }
                    </ul>
                },
                None => html! { <p>{ "not queued yet" }</p> },
            } }

            <h3>{ "Analysis" }</h3>
            { match data.analysis {
                Some(a) => html! {
                    <>
                        <ul>
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
                        { if a.category == "tiktok_video" {
                            html! {
                                <div>
                                    <button
                                        onclick={download_tiktok_video}
                                        disabled={*tiktok_download == TiktokDownloadState::Downloading}
                                    >
                                        { "Download video" }
                                    </button>
                                    { match &*tiktok_download {
                                        TiktokDownloadState::Idle => html! {},
                                        TiktokDownloadState::Downloading => html! { <p>{ "downloading..." }</p> },
                                        TiktokDownloadState::Done(path) => html! { <p>{ format!("saved to {path}") }</p> },
                                        TiktokDownloadState::Failed(err) => html! { <p>{ format!("failed: {err}") }</p> },
                                    } }
                                </div>
                            }
                        } else {
                            html! {}
                        } }
                    </>
                },
                None => html! { <p>{ "no analysis yet" }</p> },
            } }
        </div>
    }
}
