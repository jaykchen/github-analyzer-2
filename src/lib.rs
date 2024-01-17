pub mod data_analyzers;
pub mod github_data_fetchers;
pub mod reports;
pub mod utils;
use dotenv::dotenv;
use flowsnet_platform_sdk::logger;
use reports::*;
use serde_json::Value;
use std::collections::HashMap;
use webhook_flows::{ create_endpoint, request_handler, send_response };

#[no_mangle]
#[tokio::main(flavor = "current_thread")]
pub async fn on_deploy() {
    create_endpoint().await;
}

#[request_handler]
async fn handler(
    _headers: Vec<(String, String)>,
    _subpath: String,
    _qry: HashMap<String, Value>,
    _body: Vec<u8>
) {
    dotenv().ok();
    logger::init();

    let OPENAI_API_KEY = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");

    let (owner, repo) = match
        (
            _qry.get("owner").unwrap_or(&Value::Null).as_str(),
            _qry.get("repo").unwrap_or(&Value::Null).as_str(),
        )
    {
        (Some(o), Some(r)) => (o.to_string(), r.to_string()),
        (_, _) => {
            send_response(
                400,
                vec![(String::from("content-type"), String::from("text/plain"))],
                "You must provide an owner and repo name.".as_bytes().to_vec()
            );
            return;
        }
    };

    let user_name = _qry
        .get("username")
        .unwrap_or(&Value::Null)
        .as_str()
        .map(|n| n.to_string());
    let token = _qry
        .get("token")
        .unwrap_or(&Value::Null)
        .as_str()
        .map(|n| n.to_string());

    let output = weekly_report(&owner, &repo, user_name, token.clone()).await;

    send_response(
        200,
        vec![(String::from("content-type"), String::from("text/plain"))],
        output.as_bytes().to_vec()
    );
}
