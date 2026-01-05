use acquiesce::AcquiesceRepr;
use acquiesce::render::schema::{ChatMessages, ChatTool, ChatToolChoice};
use acquiesce::render::{GrammarSyntax, RenderResult};
use hf_hub::Cache;
use hf_hub::api::sync::Api;
use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader};

const TEST_MODELS: &[&str] = &[
    // "zai-org/GLM-4.6-FP8",
    // "zai-org/GLM-4.5-Air-FP8",
    "moonshotai/Kimi-K2-Instruct-0905",
    "moonshotai/Kimi-K2-Thinking",
    // "deepseek-ai/DeepSeek-V3",
    // "meta-llama/Llama-4-Maverick-17B-128E-Instruct",
    // "Qwen/Qwen3-Coder-480B-A35B-Instruct-FP8",
    // "Qwen/Qwen3-Next-80B-A3B-Instruct",
    // "Qwen/Qwen3-Next-80B-A3B-Thinking",
    // "Qwen/Qwen3-235B-A22B-Thinking-2507-FP8",
    // "Qwen/Qwen3-235B-A22B-Instruct-2507-FP8",
    // "EssentialAI/rnj-1-instruct",
];

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
        } in test_cases.clone()
        {
            let RenderResult {
                grammar: lark_grammar,
                ..
            } = acquiesce
                .render(
                    messages.clone(),
                    tools.clone(),
                    tool_choice.clone(),
                    true,
                    true,
                    GrammarSyntax::Lark,
                )
                .unwrap();

            let RenderResult {
                prompt,
                grammar: gbnf_grammar,
            } = acquiesce
                .render(
                    messages,
                    tools,
                    tool_choice,
                    true,
                    true,
                    GrammarSyntax::GBNF,
                )
                .unwrap();

            println!("Prompt: {prompt}");
            println!("GBNF Grammar: {gbnf_grammar:?}");
            println!("Lark Grammar: {lark_grammar:?}");
        }
    }
}
