use acquiesce::AcquiesceRepr;
use acquiesce::render::schema::{ChatMessages, ChatTool, ChatToolChoice};
use acquiesce::render::{GrammarSyntax, RenderResult};
use hf_hub::Cache;
use hf_hub::api::sync::Api;
use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader};

const TEST_MODELS: &[&str] = &["moonshotai/Kimi-K2-Instruct-0905"];

const TEST_CORPUS_PATH: &str = "tests/messages.jsonl";

#[derive(Clone, Deserialize)]
struct TestCase {
    messages: ChatMessages,
    tools: Vec<ChatTool>,
    #[serde(default)]
    tool_choice: ChatToolChoice,
}

#[test]
fn test_render_corpus() {
    let api = Api::new().unwrap();

    let file = File::open(TEST_CORPUS_PATH).unwrap();
    let reader = BufReader::new(file);

    let test_cases = reader
        .lines()
        .map(|line| {
            let line = line.unwrap();
            serde_json::from_str::<TestCase>(&line).unwrap()
        })
        .collect::<Vec<_>>();

    for model in TEST_MODELS {
        println!("Testing model: {model}\n\n");

        let cache = Cache::default().model(model.to_string());
        let repo = api.model(model.to_string());

        repo.get("chat_template.jinja").unwrap();
        repo.get("tokenizer_config.json").unwrap();
        repo.get("config.json").unwrap();

        let acquiesce = AcquiesceRepr::infer_default(model)
            .unwrap()
            .resolve_from_repo(&cache)
            .unwrap();

        for TestCase {
            messages,
            tools,
            tool_choice,
            ..
        } in test_cases.clone()
        {
            let RenderResult { prompt, grammar } = acquiesce
                .render(
                    messages,
                    tools,
                    tool_choice,
                    false,
                    false,
                    GrammarSyntax::Lark,
                )
                .unwrap();

            println!("Prompt: {prompt}\nGrammar: {grammar:?}\n\n");
        }
    }
}
