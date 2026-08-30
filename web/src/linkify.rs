use yew::prelude::*;

/// Renders free-form text, turning any `http://`/`https://` URL into a clickable link while
/// leaving the rest as plain text.
pub fn linkify(text: &str) -> Html {
    let mut nodes: Vec<Html> = Vec::new();

    for token in text.split_inclusive(char::is_whitespace) {
        let trailing_len: usize = token
            .chars()
            .rev()
            .take_while(|c| c.is_whitespace())
            .map(char::len_utf8)
            .sum();
        let (word, trailing) = token.split_at(token.len() - trailing_len);

        if word.starts_with("http://") || word.starts_with("https://") {
            nodes.push(html! {
                <a
                    href={word.to_string()}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="text-blue-600 underline break-all"
                >
                    { word }
                </a>
            });
            if !trailing.is_empty() {
                nodes.push(trailing.into());
            }
        } else {
            nodes.push(token.into());
        }
    }

    html! { <>{ for nodes }</> }
}
