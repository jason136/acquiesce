use core::fmt;
use std::{collections::HashMap, fmt::Display, sync::OnceLock};

use llguidance::{ParserFactory, api::TopLevelGrammar, toktrie::ApproximateTokEnv};
use serde_json::json;

use crate::{
    Acquiesce, Arguments, Config, Error, Lexeme, OrderedLexemes, Thinking, ToolCall, ToolCalls,
    render::{
        gbnf::{gbnf_regex, gbnf_string_literal},
        lark::{lark_json_schema, lark_regex, lark_string_literal, lark_token_literal},
        schema::{
            ChatTool, ChatToolChoice, CustomTool, CustomToolFormat, CustomToolGrammar,
            CustomToolSyntax, FunctionName, FunctionTool,
        },
        template::{TemplateChatMessage, TemplateTool},
    },
    schema::{Schema, SchemaCompiler, ArraySchema, ObjectSchema, NumberSchema, StringSchema},
};

pub(crate) mod gbnf;
pub(crate) mod lark;

pub mod schema;
pub mod template;

pub enum GrammarSyntax {
    Lark,
    GBNF,
}

pub struct RenderResult {
    pub prompt: String,
    pub grammar: Option<String>,
    // pub parser: Option<Parser>,
}

impl Acquiesce {
    pub fn render(
        &self,
        messages: impl Into<Vec<TemplateChatMessage>>,
        tools: Vec<ChatTool>,
        tool_choice: ChatToolChoice,
        parallel_tool_calls: bool,
        mixed_content_tool_calls: bool,
        grammar_syntax: GrammarSyntax,
    ) -> Result<RenderResult, RenderError> {
        match self {
            Config::Components {
                chat_template,
                thinking,
                tool_calls,
            } => {
                let (Some(tool_calls), false, false) = (
                    tool_calls,
                    tools.is_empty(),
                    matches!(tool_choice, ChatToolChoice::None),
                ) else {
                    let prompt = chat_template.render(messages.into(), &[])?;

                    return Ok(RenderResult {
                        prompt,
                        grammar: None,
                        // parser: None,
                    });
                };

                let validated_tools =
                    tools
                        .into_iter()
                        .try_fold(Vec::new(), |mut tool_acc, tool| {
                            match &tool {
                                ChatTool::Function {
                                    function:
                                        FunctionTool {
                                            name, parameters, ..
                                        },
                                } => {
                                    jsonschema::meta::validate(parameters).map_err(|e| {
                                        RenderError::JsonSchema(name.clone(), e.to_string())
                                    })?;
                                }
                                ChatTool::Custom {
                                    custom: CustomTool { name, format, .. },
                                } => match format {
                                    CustomToolFormat::Text => {}
                                    CustomToolFormat::Grammar {
                                        grammar: CustomToolGrammar { definition, syntax },
                                    } => match syntax {
                                        CustomToolSyntax::Lark => {
                                            static PARSER_FACTORY: OnceLock<ParserFactory> =
                                                OnceLock::new();

                                            let parser_factory = PARSER_FACTORY.get_or_init(|| {
                                                let tok_env = ApproximateTokEnv::single_byte_env();
                                                ParserFactory::new_simple(&tok_env).unwrap()
                                            });

                                            let grammar =
                                                TopLevelGrammar::from_lark(definition.clone());
                                            parser_factory.create_parser(grammar).map_err(|e| {
                                                RenderError::Lark(name.clone(), e.to_string())
                                            })?;
                                        }
                                        CustomToolSyntax::Regex => {
                                            regex::Regex::new(definition).map_err(|e| {
                                                RenderError::Regex(name.clone(), e.to_string())
                                            })?;
                                        }
                                    },
                                },
                            }

                            tool_acc.push(tool.into());

                            Ok::<_, RenderError>(tool_acc)
                        })?;

                let prompt = chat_template.render(messages.into(), &validated_tools)?;

                let mut rules = Rules::new(grammar_syntax);

                let Some((tools_rule, allow_content)) = (match tool_calls {
                    ToolCalls::ToolCall { tool_call } => {
                        tool_choice.render(tool_call, &validated_tools, &mut rules)?
                    }
                    ToolCalls::ToolCallsSection {
                        prefix,
                        tool_call,
                        suffix,
                    } => tool_choice
                        .render(tool_call, &validated_tools, &mut rules)?
                        .map(|(mut tool_choice, allow_content)| {
                            let mut acc = vec![prefix.render(&mut rules)?];

                            if parallel_tool_calls {
                                tool_choice =
                                    rules.insert_repetition("tool_choice", tool_choice, 0, None);
                            }

                            acc.push(tool_choice);

                            if let Some(suffix) = suffix {
                                acc.push(suffix.render(&mut rules)?);
                            }

                            let tools_rule = rules.insert_sequence("tool_choices", &acc);
                            Ok::<_, RenderError>((tools_rule, allow_content))
                        })
                        .transpose()?,
                }) else {
                    return Ok(RenderResult {
                        prompt,
                        grammar: None,
                        // parser: None,
                    });
                };

                let text_rule = rules.insert_text_lexeme()?;
                let mut acc = Vec::new();

                if let Some(Thinking { prefix, suffix }) = thinking {
                    acc.push(prefix.render(&mut rules)?);
                    acc.push(text_rule.clone());
                    acc.push(suffix.render(&mut rules)?);
                }

                if allow_content || mixed_content_tool_calls {
                    acc.push(text_rule.clone());
                }

                acc.push(tools_rule);

                let root = rules.insert_sequence("root", &acc);
                let grammar = rules.resolve(root);

                Ok(RenderResult {
                    prompt,
                    grammar: Some(grammar),
                    // parser: self.parser(),
                })
            }
            Config::Harmony => Ok(RenderResult {
                prompt: String::new(),
                grammar: None,
                // parser: None,
            }),
        }
    }
}

impl OrderedLexemes {
    fn render(&self, rules: &mut Rules) -> Result<RuleKey, RenderError> {
        let OrderedLexemes(literals) = self;

        let sequence_keys = literals
            .iter()
            .map(|lexeme| rules.insert_lexeme("sequence", lexeme))
            .collect::<Result<Vec<_>, RenderError>>()?;

        Ok(rules.insert_sequence("sequence", &sequence_keys))
    }
}

impl TemplateTool {
    fn naive_json_schema(&self, name_key: &str, argument_key: &str) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "enum": [
                        name_key,
                    ],
                },
                argument_key: {
                    "type": "object",
                    "properties": self.parameters,
                },
                "required": [
                    "name",
                    argument_key,
                ]
            }
        })
    }
}

impl ChatToolChoice {
    fn render(
        &self,
        tool_call: &ToolCall,
        validated_tools: &[TemplateTool],
        rules: &mut Rules,
    ) -> Result<Option<(RuleKey, bool)>, RenderError> {
        Ok(match self {
            ChatToolChoice::Auto => {
                let tool_choice = tool_call.render(validated_tools, rules)?;
                let tool_choice = rules.insert_repetition("tool_choice", tool_choice, 0, Some(1));

                Some((tool_choice, true))
            }
            ChatToolChoice::None => None,
            ChatToolChoice::Required => Some((tool_call.render(validated_tools, rules)?, false)),
            ChatToolChoice::Function(FunctionName { name }) => {
                let selected_tool = validated_tools
                    .iter()
                    .find(|tool| &tool.name == name)
                    .ok_or(RenderError::ChatToolChoice)?;
                let tool_choice = tool_call.render(std::slice::from_ref(selected_tool), rules)?;

                Some((tool_choice, false))
            }
        })
    }
}

impl ToolCall {
    fn render(&self, tools: &[TemplateTool], rules: &mut Rules) -> Result<RuleKey, RenderError> {
        match self {
            ToolCall::JsonObject {
                name_key,
                argument_key,
            } => {
                let schema_choices = tools
                    .iter()
                    .map(|tool| tool.naive_json_schema(name_key, argument_key))
                    .collect::<Vec<_>>();

                let object_schema = json!({
                    "anyOf": schema_choices,
                });

                Ok(rules.insert_lexeme("tool_choice", &Lexeme::JsonSchema(object_schema))?)
            }
            ToolCall::JsonArray {
                name_key,
                argument_key,
            } => {
                let schema_choices = tools
                    .iter()
                    .map(|tool| tool.naive_json_schema(name_key, argument_key))
                    .collect::<Vec<_>>();

                let array_schema = json!({
                    "type": "array",
                    "items": {
                        "anyOf": schema_choices,
                    },
                });

                Ok(rules.insert_lexeme("tool_choice", &Lexeme::JsonSchema(array_schema))?)
            }
            ToolCall::NamedParameters {
                prefix,
                delimiter,
                arguments,
                suffix,
            } => {
                let alternative_keys = tools
                    .iter()
                    .map(|tool| {
                        let mut acc = Vec::new();

                        if let Some(prefix) = prefix {
                            acc.push(prefix.render(rules)?);
                        }

                        acc.push(rules.insert_lexeme("name", &Lexeme::Text(tool.name.clone()))?);

                        if let Some(delimiter) = delimiter {
                            acc.push(delimiter.render(rules)?);
                        }

                        match arguments {
                            Arguments::JsonObject => {
                                acc.push(rules.insert_lexeme(
                                    "parameters",
                                    &Lexeme::JsonSchema(tool.parameters.clone()),
                                )?);
                            }
                        }

                        if let Some(suffix) = suffix {
                            acc.push(suffix.render(rules)?);
                        }

                        Ok(rules.insert_sequence("tool_choice_item", &acc))
                    })
                    .collect::<Result<Vec<_>, RenderError>>()?;

                Ok(rules.insert_alternative("tool_choices", &alternative_keys))
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct RuleKey(String, usize);

impl Display for RuleKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.0, self.1)
    }
}

struct Rules {
    rules: HashMap<RuleKey, String>,
    syntax: GrammarSyntax,
}

impl Rules {
    fn new(syntax: GrammarSyntax) -> Self {
        Self {
            rules: HashMap::new(),
            syntax,
        }
    }

    fn insert_sequence(&mut self, key: &str, sequence_keys: &[RuleKey]) -> RuleKey {
        let rule = sequence_keys
            .iter()
            .map(|rule_key| rule_key.to_string())
            .collect::<Vec<_>>()
            .join(" ");

        self.insert_rule(key, rule)
    }

    fn insert_alternative(&mut self, key: &str, alternative_keys: &[RuleKey]) -> RuleKey {
        let rule = alternative_keys
            .iter()
            .map(|rule_key| rule_key.to_string())
            .collect::<Vec<_>>()
            .join(" | ");

        self.insert_rule(key, rule)
    }

    fn insert_repetition(
        &mut self,
        key: &str,
        repetition_key: RuleKey,
        start: usize,
        end: Option<usize>,
    ) -> RuleKey {
        let rule = match (start, end) {
            (0, None) => format!("{}*", repetition_key),
            (1, None) => format!("{}+", repetition_key),
            (0, Some(1)) => format!("{}?", repetition_key),
            (exact, Some(maybe_exact)) if exact == maybe_exact => {
                format!("{}{{{}}}", repetition_key, exact)
            }
            (at_least, None) => format!("{}{{{},}}", repetition_key, at_least),
            (at_least, Some(at_most)) => format!("{}{{{},{}}}", repetition_key, at_least, at_most),
        };

        self.insert_rule(key, rule)
    }

    fn insert_lexeme(&mut self, key: &str, lexeme: &Lexeme) -> Result<RuleKey, RenderError> {
        match self.syntax {
            GrammarSyntax::Lark => {
                let rule = match lexeme {
                    Lexeme::Text(text) => lark_string_literal(text),
                    Lexeme::Token(token) => lark_token_literal(token),
                    Lexeme::Regex { pattern } => lark_regex(pattern),
                    Lexeme::JsonSchema(json_schema) => lark_json_schema(json_schema),
                };

                Ok(self.insert_rule(&key.to_uppercase(), rule))
            }
            GrammarSyntax::GBNF => {
                match lexeme {
                    Lexeme::Text(text) => Ok(self.insert_rule(key, gbnf_string_literal(text))),
                    Lexeme::Token(token) => Ok(self.insert_rule(key, gbnf_string_literal(token))),
                    Lexeme::Regex { pattern } => Ok(self.insert_rule(key, gbnf_regex(pattern))),
                    Lexeme::JsonSchema(json_schema) => {
                        let schema = SchemaCompiler::compile(json_schema)
                            .map_err(|e| RenderError::JsonSchemaConversion(e.to_string()))?;
                        self.insert_schema(key, &schema)
                    }
                }
            }
        }
    }

    /// Render a Schema AST to grammar rules
    fn insert_schema(&mut self, name: &str, schema: &Schema) -> Result<RuleKey, RenderError> {
        match schema {
            Schema::Any => self.insert_primitive("value"),
            Schema::Unsatisfiable(reason) => {
                Err(RenderError::JsonSchemaConversion(format!("Unsatisfiable: {}", reason)))
            }
            Schema::Null => self.insert_primitive("null"),
            Schema::Boolean(None) => self.insert_primitive("boolean"),
            Schema::Boolean(Some(b)) => {
                let lit = if *b { "true" } else { "false" };
                Ok(self.insert_rule(name, format!(r#""{}" space"#, lit)))
            }
            Schema::Number(num) => self.insert_number_schema(name, num),
            Schema::String(str_schema) => self.insert_string_schema(name, str_schema),
            Schema::Array(arr) => self.insert_array_schema(name, arr),
            Schema::Object(obj) => self.insert_object_schema(name, obj),
            Schema::AnyOf(alts) | Schema::OneOf(alts) => {
                let alt_keys: Vec<RuleKey> = alts.iter().enumerate()
                    .map(|(i, s)| self.insert_schema(&format!("{}-{}", name, i), s))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(self.insert_alternative(name, &alt_keys))
            }
            Schema::Const(val) => {
                let json_str = serde_json::to_string(val)?;
                let lit = gbnf_string_literal(&json_str);
                Ok(self.insert_rule(name, format!("{} space", lit)))
            }
            Schema::Enum(vals) => {
                let alts: Vec<String> = vals.iter()
                    .map(|v| gbnf_string_literal(&serde_json::to_string(v).unwrap_or_default()))
                    .collect();
                Ok(self.insert_rule(name, format!("({}) space", alts.join(" | "))))
            }
            Schema::Ref(ref_name) => {
                Ok(RuleKey(ref_name.split('/').last().unwrap_or(ref_name).to_string(), 0))
            }
        }
    }

    fn insert_number_schema(&mut self, _name: &str, num: &NumberSchema) -> Result<RuleKey, RenderError> {
        if num.integer {
            self.insert_primitive("integer")
        } else {
            self.insert_primitive("number")
        }
    }

    fn insert_string_schema(&mut self, name: &str, str_schema: &StringSchema) -> Result<RuleKey, RenderError> {
        // Handle format
        if let Some(ref fmt) = str_schema.format {
            let fmt_name = format!("{}-string", fmt);
            if let Some((content, deps)) = lookup_format_rule(&fmt_name) {
                return self.insert_primitive_with_deps(&fmt_name, content, deps);
            }
        }

        // Handle pattern
        if let Some(ref pattern) = str_schema.pattern {
            let pat = pattern.trim_start_matches('^').trim_end_matches('$');
            return Ok(self.insert_rule(name, format!(r#""\"" ({}) "\"" space"#, pat)));
        }

        // Handle length constraints
        if str_schema.min_length > 0 || str_schema.max_length.is_some() {
            let char_key = self.insert_primitive("char")?;
            let rep = self.build_repetition_str(&char_key.to_string(), str_schema.min_length, str_schema.max_length);
            return Ok(self.insert_rule(name, format!(r#""\"" {} "\"" space"#, rep)));
        }

        self.insert_primitive("string")
    }

    fn insert_array_schema(&mut self, name: &str, arr: &ArraySchema) -> Result<RuleKey, RenderError> {
        if !arr.prefix_items.is_empty() {
            // Tuple
            let item_keys: Vec<RuleKey> = arr.prefix_items.iter().enumerate()
                .map(|(i, s)| self.insert_schema(&format!("{}-tuple-{}", name, i), s))
                .collect::<Result<Vec<_>, _>>()?;
            
            let comma = self.insert_rule("comma", r#""," space"#.to_string());
            let mut seq = Vec::new();
            for (i, key) in item_keys.into_iter().enumerate() {
                if i > 0 { seq.push(comma.clone()); }
                seq.push(key);
            }
            let inner = self.insert_sequence(&format!("{}-items", name), &seq);
            Ok(self.insert_rule(name, format!(r#""[" space {} "]" space"#, inner)))
        } else if let Some(ref items) = arr.items {
            // Homogeneous array
            let item_key = self.insert_schema(&format!("{}-item", name), items)?;
            let rep = self.build_repetition_sep(&item_key.to_string(), arr.min_items, arr.max_items, r#""," space"#);
            Ok(self.insert_rule(name, format!(r#""[" space {} "]" space"#, rep)))
        } else {
            self.insert_primitive("array")
        }
    }

    fn insert_object_schema(&mut self, name: &str, obj: &ObjectSchema) -> Result<RuleKey, RenderError> {
        if obj.properties.is_empty() && obj.additional_properties.is_none() {
            return self.insert_primitive("object");
        }

        let mut kv_rules: Vec<(String, RuleKey)> = Vec::new();

        // Generate rules for each property
        for (prop_name, prop_schema) in &obj.properties {
            let prop_key = self.insert_schema(&format!("{}-{}", name, prop_name), prop_schema)?;
            let key_lit = gbnf_string_literal(&format!("\"{}\"", prop_name));
            let kv_rule = format!(r#"{} space ":" space {}"#, key_lit, prop_key);
            let kv_key = self.insert_rule(&format!("{}-{}-kv", name, prop_name), kv_rule);
            kv_rules.push((prop_name.clone(), kv_key));
        }

        let required: Vec<_> = kv_rules.iter()
            .filter(|(k, _)| obj.required.contains(k))
            .map(|(_, key)| key.clone())
            .collect();
        let optional: Vec<_> = kv_rules.iter()
            .filter(|(k, _)| !obj.required.contains(k))
            .map(|(_, key)| key.clone())
            .collect();

        let mut parts = Vec::new();
        
        // Required properties
        if !required.is_empty() {
            let comma = self.insert_rule("comma", r#""," space"#.to_string());
            let mut seq = Vec::new();
            for (i, key) in required.iter().enumerate() {
                if i > 0 { seq.push(comma.clone()); }
                seq.push(key.clone());
            }
            parts.push(self.insert_sequence(&format!("{}-required", name), &seq));
        }

        // Optional properties (simplified - just make them all optional with ?)
        for opt_key in &optional {
            let comma_opt = self.insert_rule(
                &format!("{}-opt", opt_key),
                format!(r#"("," space {})?"#, opt_key)
            );
            parts.push(comma_opt);
        }

        let inner = if parts.is_empty() {
            self.insert_rule(&format!("{}-empty", name), String::new())
        } else {
            self.insert_sequence(&format!("{}-body", name), &parts)
        };

        Ok(self.insert_rule(name, format!(r#""{{"  space {} "}}" space"#, inner)))
    }

    fn insert_primitive(&mut self, name: &str) -> Result<RuleKey, RenderError> {
        if let Some((content, deps)) = lookup_primitive_rule(name) {
            self.insert_primitive_with_deps(name, content, deps)
        } else {
            Ok(RuleKey(name.to_string(), 0))
        }
    }

    fn insert_primitive_with_deps(&mut self, name: &str, content: &str, deps: &[&str]) -> Result<RuleKey, RenderError> {
        // Add dependencies first
        for dep in deps {
            if !self.rules.iter().any(|(k, _)| &k.0 == dep) {
                self.insert_primitive(dep)?;
            }
        }
        
        // Add the primitive itself
        if !self.rules.iter().any(|(k, _)| &k.0 == name) {
            self.rules.insert(RuleKey(name.to_string(), 0), content.to_string());
        }
        
        Ok(RuleKey(name.to_string(), 0))
    }

    fn build_repetition_str(&self, item: &str, min: usize, max: Option<usize>) -> String {
        match (min, max) {
            (0, Some(0)) => String::new(),
            (0, Some(1)) => format!("{}?", item),
            (1, None) => format!("{}+", item),
            (0, None) => format!("{}*", item),
            (min, None) => format!("{}{{{},}}", item, min),
            (min, Some(max)) if min == max => format!("{}{{{}}}", item, min),
            (min, Some(max)) => format!("{}{{{},{}}}", item, min, max),
        }
    }

    fn build_repetition_sep(&self, item: &str, min: usize, max: Option<usize>, sep: &str) -> String {
        if max == Some(0) { return String::new(); }
        if min == 0 && max == Some(1) { return format!("{}?", item); }
        
        let inner = format!("({} {})", sep, item);
        let inner_min = min.saturating_sub(1);
        let inner_max = max.map(|m| m.saturating_sub(1));
        let inner_rep = self.build_repetition_str(&inner, inner_min, inner_max);
        let result = format!("{} {}", item, inner_rep);
        
        if min == 0 { format!("({})?", result) } else { result }
    }

    fn insert_rule(&mut self, key: &str, value: String) -> RuleKey {
        let mut count = 0;
        let mut rule_key = RuleKey(key.to_string(), count);

        while let Some(rule) = self.rules.get(&rule_key) {
            if rule == &value {
                return rule_key;
            }

            count += 1;
            rule_key.1 = count;
        }

        self.rules.insert(rule_key.clone(), value);

        rule_key
    }

    fn insert_text_lexeme(&mut self) -> Result<RuleKey, RenderError> {
        match self.syntax {
            GrammarSyntax::Lark => {
                self.insert_lexeme("text", &Lexeme::Text(lark::TEXT.to_string()))
            }
            GrammarSyntax::GBNF => {
                self.insert_lexeme("text", &Lexeme::Text(gbnf::TEXT.to_string()))
            }
        }
    }

    fn resolve(&mut self, root_key: RuleKey) -> String {
        let root_rule = self.rules.remove(&root_key).unwrap_or_default();

        match self.syntax {
            GrammarSyntax::Lark => {
                format!(
                    "start: {root_rule}\n{}",
                    self.rules
                        .iter()
                        .map(|(key, value)| format!("{key}: {value}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
            GrammarSyntax::GBNF => {
                format!(
                    "root ::= {root_rule}\n{}",
                    self.rules
                        .iter()
                        .map(|(key, value)| format!("{key} ::= {value}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
        }
    }
}

// Primitive GBNF rules
const PRIMITIVE_RULES: &[(&str, &str, &[&str])] = &[
    ("space", r#"| " " | "\n"{1,2} [ \t]{0,20}"#, &[]),
    ("boolean", r#"("true" | "false") space"#, &[]),
    ("decimal-part", "[0-9]{1,16}", &[]),
    ("integral-part", "[0] | [1-9] [0-9]{0,15}", &[]),
    ("number", r#"("-"? integral-part) ("." decimal-part)? ([eE] [-+]? integral-part)? space"#, &["integral-part", "decimal-part"]),
    ("integer", r#"("-"? integral-part) space"#, &["integral-part"]),
    ("value", "object | array | string | number | boolean | null", &["object", "array", "string", "number", "boolean", "null"]),
    ("object", r#""{" space ( string ":" space value ("," space string ":" space value)* )? "}" space"#, &["string", "value"]),
    ("array", r#""[" space ( value ("," space value)* )? "]" space"#, &["value"]),
    ("char", r#"[^"\\\x7F\x00-\x1F] | [\\] (["\\bfnrt] | "u" [0-9a-fA-F]{4})"#, &[]),
    ("string", r#""\"" char* "\"" space"#, &["char"]),
    ("null", r#""null" space"#, &[]),
];

const FORMAT_RULES: &[(&str, &str, &[&str])] = &[
    ("date", r#"[0-9]{4} "-" ( "0" [1-9] | "1" [0-2] ) "-" ( "0" [1-9] | [1-2] [0-9] | "3" [0-1] )"#, &[]),
    ("time", r#"([01] [0-9] | "2" [0-3]) ":" [0-5] [0-9] ":" [0-5] [0-9] ( "." [0-9]{3} )? ( "Z" | ( "+" | "-" ) ( [01] [0-9] | "2" [0-3] ) ":" [0-5] [0-9] )"#, &[]),
    ("date-time", r#"date "T" time"#, &["date", "time"]),
    ("date-string", r#""\"" date "\"" space"#, &["date"]),
    ("time-string", r#""\"" time "\"" space"#, &["time"]),
    ("date-time-string", r#""\"" date-time "\"" space"#, &["date-time"]),
];

fn lookup_primitive_rule(name: &str) -> Option<(&'static str, &'static [&'static str])> {
    PRIMITIVE_RULES.iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, content, deps)| (*content, *deps))
}

fn lookup_format_rule(name: &str) -> Option<(&'static str, &'static [&'static str])> {
    FORMAT_RULES.iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, content, deps)| (*content, *deps))
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("json schema for tool {0} is invalid: {1}")]
    JsonSchema(String, String),

    #[error("json schema conversion error: {0}")]
    JsonSchemaConversion(String),

    #[error("regex for tool {0} is invalid: {1}")]
    Regex(String, String),

    #[error("tool choice not found in provided tools")]
    ChatToolChoice,

    #[error("lark grammar for tool {0} is invalid: {1}")]
    Lark(String, String),

    #[error("chat template render error: {0}")]
    Template(#[from] minijinja::Error),

    #[error("json serialization error: {0}")]
    Json(#[from] serde_json::Error),
}
