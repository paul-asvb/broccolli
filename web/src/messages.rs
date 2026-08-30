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
    short_summary: Option<String>,
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

    let delete_message = {
        let data = data.clone();
        Callback::from(move |(chat_id, message_id): (i64, i64)| {
            let data = data.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let url = format!("/api/messages/{chat_id}/{message_id}");
                if let Ok(resp) = Request::delete(&url).send().await
                    && resp.ok()
                    && let Some(mut current) = (*data).clone()
                {
                    current
                        .messages
                        .retain(|m| !(m.chat_id == chat_id && m.message_id == message_id));
                    current.total = (current.total - 1).max(0);
                    data.set(Some(current));
                }
            });
        })
    };

    let Some(data) = (*data).clone() else {
        return html! { <p class="text-gray-500">{ "loading..." }</p> };
    };

    let total_pages = ((data.total + data.per_page - 1) / data.per_page.max(1)).max(1);
    let prev_href = format!(
        "/messages?page={}&per_page={}",
        (data.page - 1).max(1),
        data.per_page
    );
    let next_href = format!("/messages?page={}&per_page={}", data.page + 1, data.per_page);
    let page_href = |p: i64| format!("/messages?page={}&per_page={}", p, data.per_page);
    let page_link_class = |active: bool| {
        if active {
            "rounded-md bg-gray-900 px-2.5 py-1 text-sm font-medium text-white"
        } else {
            "rounded-md px-2.5 py-1 text-sm font-medium text-gray-600 hover:bg-gray-100"
        }
    };

    html! {
        <div>
            <h2 class="mb-4 text-lg font-semibold text-gray-900">
                { format!("Messages — page {} of {} ({} total)", data.page, total_pages, data.total) }
            </h2>
            <div class="overflow-x-auto rounded-lg border border-gray-200 bg-white">
                <table class="w-full text-left text-sm">
                    <thead class="border-b border-gray-200 bg-gray-50 text-xs uppercase tracking-wide text-gray-500">
                        <tr>
                            <th class="px-4 py-2 font-medium">{ "Date" }</th>
                            <th class="px-4 py-2 font-medium">{ "Text" }</th>
                            <th class="px-4 py-2 font-medium">{ "Summary" }</th>
                            <th class="px-4 py-2"></th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-gray-100">
                        { for data.messages.iter().map(|m| {
                            let chat_id = m.chat_id;
                            let message_id = m.message_id;
                            let onclick = {
                                let delete_message = delete_message.clone();
                                Callback::from(move |_| delete_message.emit((chat_id, message_id)))
                            };
                            html! {
                                <tr class="hover:bg-gray-50">
                                    <td class="whitespace-nowrap px-4 py-2 align-top">
                                        <a
                                            href={format!("/messages/{}/{}", m.chat_id, m.message_id)}
                                            class="font-medium text-gray-900 hover:underline"
                                        >
                                            { format_date(m.date_unixtime) }
                                        </a>
                                    </td>
                                    <td class="max-w-md px-4 py-2 align-top text-gray-700">{ m.text.clone().unwrap_or_default() }</td>
                                    <td class="max-w-xs px-4 py-2 align-top text-gray-500">{ m.short_summary.clone().unwrap_or_default() }</td>
                                    <td class="px-4 py-2 align-top text-right">
                                        <button
                                            onclick={onclick}
                                            class="rounded-md border border-gray-300 px-2.5 py-1 text-xs font-medium text-gray-600 hover:bg-red-50 hover:text-red-600"
                                        >
                                            { "Delete" }
                                        </button>
                                    </td>
                                </tr>
                            }
                        }) }
                    </tbody>
                </table>
            </div>
            <div class="mt-4 flex items-center gap-1">
                <a href={prev_href} class={page_link_class(false)}>{ "« prev" }</a>
                { for (1..=total_pages).map(|p| html! {
                    <a href={page_href(p)} class={page_link_class(p == data.page)}>{ p }</a>
                }) }
                <a href={next_href} class={page_link_class(false)}>{ "next »" }</a>
            </div>
        </div>
    }
}
