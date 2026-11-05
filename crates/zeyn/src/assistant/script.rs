//! A scripted model for tests, demos and offline development. It answers from rules instead of a
//! network call, streams its text word by word and calls tools like a real model, so the whole
//! path (permissions, confirmation, audit, streaming) runs for real.

use super::{ChatEvent, ChatRequest, ChatStream, LlmProvider, Role, STREAM_BUFFER, ToolCall};
use crate::ctx::BoxFuture;
use crate::text::fold;
use serde_json::Value as Json;
use std::time::Duration;

/// What a scripted model does in one round.
#[derive(Clone, Debug)]
pub enum Say {
    /// Streams text, a word at a time.
    Text(String),
    /// Calls a tool.
    Tool(String, Json),
}

impl Say {
    pub fn text(text: impl Into<String>) -> Self {
        Say::Text(text.into())
    }

    pub fn tool(name: impl Into<String>, arguments: Json) -> Self {
        Say::Tool(name.into(), arguments)
    }
}

/// A model that follows rules.
///
/// Each rule matches the user's last message when it contains all of its words (ignoring case
/// and Turkish dotted letters) and gives one list of actions per round: round 0 answers the
/// message, round 1 follows the first tool results, and so on. A declined confirmation is
/// answered with a short note.
///
/// ```
/// use zeyn::assistant::{ScriptedProvider, Say};
/// use serde_json::json;
///
/// let model = ScriptedProvider::new()
///     .when("orders izmir", vec![
///         vec![Say::tool("navigate", json!({ "screen": "today", "filters": [{ "field": "city", "contains": "İzmir" }] }))],
///         vec![Say::text("Here are today's orders from İzmir.")],
///     ])
///     .otherwise(vec![vec![Say::text("I can show orders and products.")]]);
/// ```
#[derive(Clone, Debug, Default)]
pub struct ScriptedProvider {
    rules: Vec<(Vec<String>, Vec<Vec<Say>>)>,
    otherwise: Vec<Vec<Say>>,
    delay: Duration,
}

impl ScriptedProvider {
    pub fn new() -> Self {
        ScriptedProvider::default()
    }

    /// Answers messages containing every word of `words`, round by round.
    pub fn when(mut self, words: &str, rounds: Vec<Vec<Say>>) -> Self {
        self.rules.push((
            fold(words)
                .split_whitespace()
                .map(str::to_string)
                .collect(),
            rounds,
        ));
        self
    }

    /// Answers messages no rule matches.
    pub fn otherwise(mut self, rounds: Vec<Vec<Say>>) -> Self {
        self.otherwise = rounds;
        self
    }

    /// Pause between streamed words, to look like a real model.
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// What to do this round.
    fn plan(&self, request: &ChatRequest) -> Vec<Say> {
        let last_user = request
            .messages
            .iter()
            .rposition(|m| m.role == Role::User);
        let text = last_user
            .map(|i| fold(&request.messages[i].text))
            .unwrap_or_default();
        let round = last_user.map_or(0, |i| {
            request.messages[i..]
                .iter()
                .filter(|m| m.role == Role::Assistant)
                .count()
        });
        if request
            .messages
            .last()
            .is_some_and(|m| m.role == Role::Tool && m.text.contains("declined"))
        {
            return vec![Say::text("All right, I left it as it was.")];
        }
        let rounds = self
            .rules
            .iter()
            .find(|(words, _)| {
                words
                    .iter()
                    .all(|w| text.contains(w.as_str()))
            })
            .map_or(&self.otherwise, |(_, r)| r);
        rounds
            .get(round)
            .cloned()
            .unwrap_or_else(|| vec![Say::text("Done.")])
    }
}

impl LlmProvider for ScriptedProvider {
    fn chat(&self, request: ChatRequest) -> BoxFuture<crate::Result<ChatStream>> {
        let plan = self.plan(&request);
        let delay = self.delay;
        let input = (request.system.len()
            + request
                .messages
                .iter()
                .map(|m| m.text.len())
                .sum::<usize>())
            / 4;
        let (tx, rx) = tokio::sync::mpsc::channel(STREAM_BUFFER);
        tokio::spawn(async move {
            let mut output = 0;
            for (i, say) in plan.into_iter().enumerate() {
                match say {
                    Say::Text(text) => {
                        output += text.len() / 4;
                        for word in text.split_inclusive(' ') {
                            if !delay.is_zero() {
                                tokio::time::sleep(delay).await;
                            }
                            if tx
                                .send(Ok(ChatEvent::Text(word.to_string())))
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                    Say::Tool(name, arguments) => {
                        let call = ToolCall {
                            id: format!("script_{i}_{name}"),
                            name,
                            arguments,
                        };
                        if tx
                            .send(Ok(ChatEvent::ToolCall(call)))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                }
            }
            let _ = tx
                .send(Ok(ChatEvent::Usage {
                    input: input as u32,
                    output: output as u32,
                }))
                .await;
        });
        Box::pin(async move { Ok(rx) })
    }
}
