use log;
use serde_json::Value;
use async_openai::{
    types::{
        // ChatCompletionFunctionsArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs,
        // ChatCompletionTool, ChatCompletionToolArgs, ChatCompletionToolType,
        CreateChatCompletionRequestArgs,
        // FinishReason,
    },
    Client as OpenAIClient,
    config::Config,
};
use std::env;
use reqwest::header::HeaderMap;
use secrecy::Secret;
use std::collections::HashMap;

pub fn squeeze_fit_remove_quoted(inp_str: &str, max_len: u16, split: f32) -> String {
    let mut body = String::new();
    let mut inside_quote = false;

    for line in inp_str.lines() {
        if line.contains("```") || line.contains("\"\"\"") {
            inside_quote = !inside_quote;
            continue;
        }

        if !inside_quote {
            let cleaned_line = line
                .split_whitespace()
                .filter(|word| word.len() < 150)
                .collect::<Vec<&str>>()
                .join(" ");
            body.push_str(&cleaned_line);
            body.push('\n');
        }
    }

    let body_words: Vec<&str> = body.split_whitespace().collect();
    let body_len = body_words.len();
    let n_take_from_beginning = ((body_len as f32) * split) as usize;
    let n_keep_till_end = body_len - n_take_from_beginning;

    // Range check for drain operation
    let drain_start = if n_take_from_beginning < body_len {
        n_take_from_beginning
    } else {
        body_len
    };

    let drain_end = if n_keep_till_end <= body_len { body_len - n_keep_till_end } else { 0 };

    let final_text = if body_len > (max_len as usize) {
        let mut body_text_vec = body_words.to_vec();
        body_text_vec.drain(drain_start..drain_end);
        body_text_vec.join(" ")
    } else {
        body
    };

    final_text
}

pub fn squeeze_fit_post_texts(inp_str: &str, max_len: u16, split: f32) -> String {
    let bpe = tiktoken_rs::cl100k_base().unwrap();

    let input_token_vec = bpe.encode_ordinary(inp_str);
    let input_len = input_token_vec.len();
    if input_len < (max_len as usize) {
        return inp_str.to_string();
    }
    let n_take_from_beginning = ((input_len as f32) * split).ceil() as usize;
    let n_take_from_end = (max_len as usize) - n_take_from_beginning;

    let mut concatenated_tokens = Vec::with_capacity(max_len as usize);
    concatenated_tokens.extend_from_slice(&input_token_vec[..n_take_from_beginning]);
    concatenated_tokens.extend_from_slice(&input_token_vec[input_len - n_take_from_end..]);

    bpe.decode(concatenated_tokens)
        .ok()
        .map_or("failed to decode tokens".to_string(), |s| s.to_string())
}

pub async fn chain_of_chat(
    sys_prompt_1: &str,
    usr_prompt_1: &str,
    chat_id: &str,
    gen_len_1: u16,
    usr_prompt_2: &str,
    gen_len_2: u16,
    error_tag: &str
) -> anyhow::Result<String> {
    use reqwest::header::{ HeaderValue, CONTENT_TYPE, USER_AGENT };
    let token = env::var("DEEP_API_KEY").expect("DEEP_API_KEY must be set");

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(USER_AGENT, HeaderValue::from_static("MyClient/1.0.0"));
    let config = LocalServiceProviderConfig {
        // api_base: String::from("http://52.37.228.1:8080/v1"),
        api_base: String::from("http://52.37.228.1:8080/v1"),
        headers: headers,
        api_key: Secret::new(token),
        query: HashMap::new(),
    };

    let model = "mistralai/Mistral-7B-Instruct-v0.1";
    let client = OpenAIClient::with_config(config);

    let mut messages = vec![
        ChatCompletionRequestSystemMessageArgs::default()
            .content(sys_prompt_1)
            .build()
            .expect("Failed to build system message")
            .into(),
        ChatCompletionRequestUserMessageArgs::default().content(usr_prompt_1).build()?.into()
    ];
    let request = CreateChatCompletionRequestArgs::default()
        .max_tokens(gen_len_1)
        .model(model)
        .messages(messages.clone())
        .build()?;

    let chat = client.chat().create(request).await?;

    match chat.choices[0].message.clone().content {
        Some(res) => {
            log::info!("step 1 Points: {:?}", res);
        }
        None => {
            return Err(anyhow::anyhow!(error_tag.to_string()));
        }
    }

    messages.push(
        ChatCompletionRequestUserMessageArgs::default().content(usr_prompt_2).build()?.into()
    );

    let request = CreateChatCompletionRequestArgs::default()
        .max_tokens(gen_len_2)
        .model(model)
        .messages(messages)
        .build()?;

    let chat = client.chat().create(request).await?;

    match chat.choices[0].message.clone().content {
        Some(res) => {
            log::info!("step 2 Raw: {:?}", res);
            Ok(res)
        }
        None => {
            return Err(anyhow::anyhow!(error_tag.to_string()));
        }
    }
}

pub async fn chat_inner_async(
    system_prompt: &str,
    user_input: &str,
    max_token: u16,
    model: &str
) -> anyhow::Result<String> {
    use reqwest::header::{ HeaderValue, CONTENT_TYPE, USER_AGENT };
    let token = env::var("DEEP_API_KEY").expect("DEEP_API_KEY must be set");
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(USER_AGENT, HeaderValue::from_static("MyClient/1.0.0"));
    let config = LocalServiceProviderConfig {
        // api_base: String::from("http://127.0.0.1:8080/v1"),
        api_base: String::from("http://52.37.228.1:8080/v1"),
        headers: headers,
        api_key: Secret::new(token),
        query: HashMap::new(),
    };

    let client = OpenAIClient::with_config(config);
    let messages = vec![
        ChatCompletionRequestSystemMessageArgs::default()
            .content(system_prompt)
            .build()
            .expect("Failed to build system message")
            .into(),
        ChatCompletionRequestUserMessageArgs::default().content(user_input).build()?.into()
    ];
    let request = CreateChatCompletionRequestArgs::default()
        .max_tokens(max_token)
        .model(model)
        .messages(messages)
        .build()?;

    match client.chat().create(request).await {
        Ok(chat) =>
            match chat.choices[0].message.clone().content {
                Some(res) => {
                    // log::info!("{:?}", chat.choices[0].message.clone());
                    Ok(res)
                }
                None => Err(anyhow::anyhow!("Failed to get reply from OpenAI")),
            }
        Err(_e) => {
            log::error!("Error getting response from OpenAI: {:?}", _e);
            Err(anyhow::anyhow!(_e))
        }
    }
}

#[derive(Clone, Debug)]
pub struct LocalServiceProviderConfig {
    pub api_base: String,
    pub headers: HeaderMap,
    pub api_key: Secret<String>,
    pub query: HashMap<String, String>,
}

impl Config for LocalServiceProviderConfig {
    fn headers(&self) -> HeaderMap {
        self.headers.clone()
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.api_base, path)
    }

    fn query(&self) -> Vec<(&str, &str)> {
        self.query
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
    }

    fn api_base(&self) -> &str {
        &self.api_base
    }

    fn api_key(&self) -> &Secret<String> {
        &self.api_key
    }
}

pub fn parse_summary_from_raw_json(input: &str) -> anyhow::Result<String> {
    use regex::Regex;
    let parsed = match serde_json::from_str(input) {
        Ok(v) => v,
        Err(e) => {
            log::error!("Error parsing JSON: {:?}", e);
            // Attempt to extract fields using regex if JSON parsing fails
            let mut values_map = std::collections::HashMap::new();
            let keys = ["impactful", "alignment", "patterns", "synergy", "significance"];
            for key in keys.iter() {
                let regex_pattern = format!(r#""{}":\s*"([^"]*)""#, key);
                let regex = Regex::new(&regex_pattern)
                    .map_err(|_| anyhow::Error::msg("Failed to compile regex pattern"))
                    .expect("Failed to compile regex pattern");
                if let Some(captures) = regex.captures(input) {
                    if let Some(value) = captures.get(1) {
                        values_map.insert(*key, value.as_str().to_string());
                    }
                }
            }

            if values_map.len() != keys.len() {
                return Err(anyhow::Error::msg("Failed to extract all fields from JSON"));
            }

            Value::Object(
                values_map
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), Value::String(v)))
                    .collect()
            )
        }
    };

    let mut output = String::new();
    let keys = ["impactful", "alignment", "patterns", "synergy", "significance"];

    for key in keys.iter() {
        if let Some(value) = parsed.get(*key) {
            if value.is_string() {
                if !output.is_empty() {
                    output.push_str(" ");
                }
                output.push_str(value.as_str().unwrap());
            }
        }
    }

    Ok(output)
}

pub fn parse_issue_summary_from_json(input: &str) -> anyhow::Result<Vec<(String, String)>> {
    use regex::Regex;
    use serde_json::Map;

    let parsed_result: Result<Map<String, Value>, serde_json::Error> = serde_json::from_str(input);

    match parsed_result {
        Ok(parsed) => {
            let summaries = parsed
                .iter()
                .filter_map(|(key, value)| {
                    if let Some(summary_str) = value.as_str() {
                        Some((key.clone(), summary_str.to_owned()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<(String, String)>>();
            Ok(summaries)
        }

        Err(e) => {
            log::error!("Error parsing JSON: {:?}", e);

            let re = Regex::new(r#""([^"]+)":\s*"([^"]*)""#).map_err(|_|
                anyhow::Error::msg("Failed to compile regex pattern")
            )?;

            let mut results = Vec::new();

            for cap in re.captures_iter(input) {
                if let (Some(key), Some(value)) = (cap.get(1), cap.get(2)) {
                    results.push((key.as_str().to_string(), value.as_str().to_string()));
                }
            }

            if results.is_empty() {
                Err(anyhow::Error::msg("No fields could be extracted from malformed JSON"))
            } else {
                Ok(results)
            }
        }
    }
}

/* pub fn parse_issue_summary_from_json(input: &str) -> anyhow::Result<Vec<(String, String)>> {
    let parsed: serde_json::Map<String, serde_json::Value> = serde_json::from_str(input)?;

    let summaries = parsed
        .iter()
        .filter_map(|(key, value)| {
            if let Some(summary_str) = value.as_str() {
                Some((key.clone(), summary_str.to_owned()))
            } else {
                None
            }
        })
        .collect::<Vec<(String, String)>>(); // Collect into a Vec of tuples

    Ok(summaries)
} */

/* pub async fn github_http_post_gql(query: &str) -> anyhow::Result<Vec<u8>> {
    use http_req::{request::Method, request::Request, uri::Uri};
    let token = std::env::var("GITHUB_TOKEN").expect("github_token is required");
    let base_url = "https://api.github.com/graphql";
    let base_url = Uri::try_from(base_url).unwrap();
    let mut writer = Vec::new();

    let query = serde_json::json!({"query": query});
    match Request::new(&base_url)
        .method(Method::POST)
        .header("User-Agent", "flows-network connector")
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {}", token))
        .header("Content-Length", &query.to_string().len())
        .body(&query.to_string().into_bytes())
        .send(&mut writer)
    {
        Ok(res) => {
            if !res.status_code().is_success() {
                log::error!("Github http error {:?}", res.status_code());
                return Err(anyhow::anyhow!("Github http error {:?}", res.status_code()));
            };
            Ok(writer)
        }
        Err(_e) => {
            log::error!("Error getting response from Github: {:?}", _e);
            Err(anyhow::anyhow!(_e))
        }
    }
} */

pub async fn github_http_get(url: &str) -> anyhow::Result<Vec<u8>> {
    use http_req::{ request::Method, request::Request, uri::Uri };
    let token = std::env::var("GITHUB_TOKEN").expect("github_token is required");
    let mut writer = Vec::new();
    let url = Uri::try_from(url).unwrap();

    match
        Request::new(&url)
            .method(Method::GET)
            .header("User-Agent", "flows-network connector")
            .header("Content-Type", "application/json")
            .header("Authorization", &format!("Bearer {}", token))
            .header("CONNECTION", "close")
            .send(&mut writer)
    {
        Ok(res) => {
            if !res.status_code().is_success() {
                log::error!("Github http error {:?}", res.status_code());
                return Err(anyhow::anyhow!("Github http error {:?}", res.status_code()));
            }
            Ok(writer)
        }
        Err(_e) => {
            log::error!("Error getting response from Github: {:?}", _e);
            Err(anyhow::anyhow!(_e))
        }
    }
}
