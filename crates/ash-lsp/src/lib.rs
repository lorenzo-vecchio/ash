//! ash-lsp — Language Server Protocol implementation for the Ash language.
//!
//! Features:
//! - Semantic tokens (syntax highlighting) via textDocument/semanticTokens/full
//! - Parse diagnostics via textDocument/publishDiagnostics on every file change
//! - Hover documentation for stdlib functions via textDocument/hover
//! - Go-to-definition for user-defined functions

use std::collections::HashMap;
use std::sync::Arc;

use ash_lexer::{Lexer, Token};
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

// ─── Semantic token types & modifiers ────────────────────────────────────────

const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::KEYWORD,   // 0
    SemanticTokenType::STRING,    // 1
    SemanticTokenType::NUMBER,    // 2
    SemanticTokenType::OPERATOR,  // 3
    SemanticTokenType::VARIABLE,  // 4
    SemanticTokenType::FUNCTION,  // 5
    SemanticTokenType::TYPE,      // 6
    SemanticTokenType::COMMENT,   // 7
    SemanticTokenType::PARAMETER, // 8
    SemanticTokenType::NAMESPACE, // 9
];

fn token_type_index(tt: SemanticTokenType) -> u32 {
    TOKEN_TYPES.iter().position(|t| *t == tt).unwrap_or(4) as u32
}

// ─── Backend ──────────────────────────────────────────────────────────────────

struct AshBackend {
    client: Client,
    /// Map from file URI → source text (for re-analysis on change)
    documents: Arc<RwLock<HashMap<Url, String>>>,
}

impl AshBackend {
    fn new(client: Client) -> Self {
        AshBackend {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Lex source and emit context-aware semantic tokens.
    fn semantic_tokens(source: &str) -> Vec<SemanticToken> {
        let tokens = match Lexer::new(source).tokenize() {
            Ok(t) => t,
            Err(_) => return vec![],
        };

        // Filter out structural/whitespace tokens for context analysis,
        // but keep their positions.
        let meaningful: Vec<_> = tokens
            .iter()
            .filter(|s| {
                !matches!(
                    s.node,
                    Token::Indent | Token::Dedent | Token::Newline | Token::Eof
                )
            })
            .collect();

        // Build the set of known stdlib namespaces so we can colour them
        // distinctly from regular variables.
        let stdlib_ns: std::collections::HashSet<&str> = [
            "math", "file", "http", "json", "re", "env", "go", "db", "cache", "queue", "auth",
            "mail", "store", "ai",
        ]
        .iter()
        .copied()
        .collect();

        // Collect which indices are function parameters.
        // We scan for `fn name ( p1 p2 ... )` and mark each bare ident
        // inside the parens as PARAMETER.
        let mut param_indices: std::collections::HashSet<usize> = Default::default();
        {
            let mut i = 0;
            while i < meaningful.len() {
                if matches!(meaningful[i].node, Token::Fn) {
                    // skip fn name
                    i += 1;
                    if i < meaningful.len() && matches!(meaningful[i].node, Token::Ident(_)) {
                        i += 1;
                    }
                    // optional `(` — collect params until `)`
                    if i < meaningful.len() && matches!(meaningful[i].node, Token::LParen) {
                        i += 1;
                        while i < meaningful.len() && !matches!(meaningful[i].node, Token::RParen) {
                            if matches!(meaningful[i].node, Token::Ident(_)) {
                                param_indices.insert(i);
                            }
                            i += 1;
                        }
                    } else {
                        // space-separated params until newline or body colon
                        while i < meaningful.len() && matches!(meaningful[i].node, Token::Ident(_))
                        {
                            param_indices.insert(i);
                            i += 1;
                            // skip optional `: Type` annotation
                            if i < meaningful.len() && matches!(meaningful[i].node, Token::Colon) {
                                i += 1; // colon
                                if i < meaningful.len() {
                                    i += 1;
                                } // type
                            }
                        }
                    }
                } else {
                    i += 1;
                }
            }
        }

        let mut result = Vec::new();
        let mut prev_line = 0u32;
        let mut prev_col = 0u32;

        for (idx, spanned) in meaningful.iter().enumerate() {
            let prev_tok = idx.checked_sub(1).map(|j| &meaningful[j].node);
            let next_tok = meaningful.get(idx + 1).map(|s| &s.node);
            // peek two ahead to handle `ns . fn (`
            let next2_tok = meaningful.get(idx + 2).map(|s| &s.node);

            let tt = classify_token_ctx(
                &spanned.node,
                prev_tok,
                next_tok,
                next2_tok,
                param_indices.contains(&idx),
                &stdlib_ns,
            );

            let tt = match tt {
                Some(t) => t,
                None => continue,
            };

            let span = &spanned.span;
            let line = span.line.saturating_sub(1) as u32;
            let col = span.col.saturating_sub(1) as u32;
            let len = span.len as u32;

            let delta_line = line - prev_line;
            let delta_start = if delta_line == 0 { col - prev_col } else { col };

            result.push(SemanticToken {
                delta_line,
                delta_start,
                length: len,
                token_type: token_type_index(tt),
                token_modifiers_bitset: 0,
            });

            prev_line = line;
            prev_col = col;
        }

        result
    }

    /// Run the parser on source and collect diagnostics.
    fn diagnostics(source: &str) -> Vec<Diagnostic> {
        let tokens = match Lexer::new(source).tokenize() {
            Ok(t) => t,
            Err(e) => {
                return vec![Diagnostic {
                    range: Range {
                        start: Position {
                            line: 0,
                            character: 0,
                        },
                        end: Position {
                            line: 0,
                            character: 1,
                        },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    message: format!("lex error: {e}"),
                    source: Some("ash".into()),
                    ..Default::default()
                }];
            }
        };

        match ash_parser::parse(tokens) {
            Ok(_) => vec![],
            Err(e) => {
                // Extract line/col from span in the error message if present
                let (line, col) = parse_span_from_msg(&e.msg);
                vec![Diagnostic {
                    range: Range {
                        start: Position {
                            line,
                            character: col,
                        },
                        end: Position {
                            line,
                            character: col + 1,
                        },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    message: e.msg.clone(),
                    source: Some("ash".into()),
                    ..Default::default()
                }]
            }
        }
    }

    async fn publish_diagnostics(&self, uri: Url, source: &str) {
        let diags = Self::diagnostics(source);
        self.client.publish_diagnostics(uri, diags, None).await;
    }
}

/// Context-aware token classifier.
fn classify_token_ctx(
    tok: &Token,
    prev: Option<&Token>,
    next: Option<&Token>,
    _next2: Option<&Token>,
    is_param: bool,
    stdlib_ns: &std::collections::HashSet<&str>,
) -> Option<SemanticTokenType> {
    match tok {
        // Keywords
        Token::Fn
        | Token::Let
        | Token::Mut
        | Token::If
        | Token::Else
        | Token::Return
        | Token::While
        | Token::For
        | Token::In
        | Token::Match
        | Token::Type
        | Token::Move
        | Token::Await
        | Token::Panic
        | Token::Use => Some(SemanticTokenType::KEYWORD),

        Token::Bool(_) => Some(SemanticTokenType::KEYWORD),

        // Literals
        Token::Str(_) => Some(SemanticTokenType::STRING),
        Token::Int(_) | Token::Float(_) => Some(SemanticTokenType::NUMBER),

        // Operators
        Token::Plus
        | Token::Minus
        | Token::Star
        | Token::Slash
        | Token::Percent
        | Token::EqEq
        | Token::NotEq
        | Token::Lt
        | Token::Gt
        | Token::LtEq
        | Token::GtEq
        | Token::And
        | Token::Or
        | Token::Not
        | Token::Assign
        | Token::FatArrow
        | Token::ThinArrow
        | Token::Pipe
        | Token::Question
        | Token::NullCoalesce
        | Token::SafeDot
        | Token::Bang
        | Token::Amp
        | Token::DotDot => Some(SemanticTokenType::OPERATOR),

        Token::Ident(s) => {
            let is_upper = s.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);

            // Uppercase identifiers are always types
            if is_upper {
                return Some(SemanticTokenType::TYPE);
            }

            // After `fn` → function definition name
            if matches!(prev, Some(Token::Fn)) {
                return Some(SemanticTokenType::FUNCTION);
            }

            // After `type` → type definition name (lowercase edge case)
            if matches!(prev, Some(Token::Type)) {
                return Some(SemanticTokenType::TYPE);
            }

            // stdlib namespace (e.g. `math`, `db`) followed by `.` → NAMESPACE
            if stdlib_ns.contains(s.as_str())
                && matches!(next, Some(Token::Dot) | Some(Token::SafeDot))
            {
                return Some(SemanticTokenType::NAMESPACE);
            }

            // After `.` (method call or namespace function) → FUNCTION if followed by `(`
            if matches!(prev, Some(Token::Dot) | Some(Token::SafeDot)) {
                if matches!(next, Some(Token::LParen)) {
                    return Some(SemanticTokenType::FUNCTION);
                }
                // property access
                return Some(SemanticTokenType::VARIABLE);
            }

            // Function call: ident followed by `(`
            // But not if it's a namespace prefix (`ns.fn(` — the ns is handled above)
            if matches!(next, Some(Token::LParen)) {
                return Some(SemanticTokenType::FUNCTION);
            }

            // Pipeline RHS: `|> funcname` — next meaningful token after ident
            // (funcname with no parens in pipeline position)
            if matches!(prev, Some(Token::Pipe)) {
                // Could be function ref passed to pipeline
                return Some(SemanticTokenType::FUNCTION);
            }

            // Function parameter (detected by pre-pass)
            if is_param {
                return Some(SemanticTokenType::PARAMETER);
            }

            // Lambda parameter: `x =>` or `(x y) =>`
            if matches!(next, Some(Token::FatArrow)) {
                return Some(SemanticTokenType::PARAMETER);
            }

            // After `next2` is `=>` with next being a comma or rparen
            // catches multi-param lambda `(x y) => ...`
            // This is handled well enough by the FatArrow check above for single params

            Some(SemanticTokenType::VARIABLE)
        }

        // Everything else (punctuation, structural): skip
        _ => None,
    }
}

/// Try to extract (line, col) from an error message like "1:5: unexpected token"
fn parse_span_from_msg(msg: &str) -> (u32, u32) {
    // Messages may be prefixed with "line:col: ..." from the Span Display impl
    let parts: Vec<&str> = msg.splitn(3, ':').collect();
    if parts.len() >= 2 {
        let line = parts[0]
            .trim()
            .parse::<u32>()
            .unwrap_or(1)
            .saturating_sub(1);
        let col = parts[1]
            .trim()
            .parse::<u32>()
            .unwrap_or(1)
            .saturating_sub(1);
        return (line, col);
    }
    (0, 0)
}

// ─── LanguageServer trait impl ────────────────────────────────────────────────

#[tower_lsp::async_trait]
impl LanguageServer for AshBackend {
    async fn initialize(&self, _params: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "ash-lsp".into(),
                version: Some("0.1.0".into()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: TOKEN_TYPES.to_vec(),
                                token_modifiers: vec![],
                            },
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: None,
                            work_done_progress_options: Default::default(),
                        },
                    ),
                ),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "ash-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.documents
            .write()
            .await
            .insert(uri.clone(), text.clone());
        self.publish_diagnostics(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            let text = change.text;
            self.documents
                .write()
                .await
                .insert(uri.clone(), text.clone());
            self.publish_diagnostics(uri, &text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents
            .write()
            .await
            .remove(&params.text_document.uri);
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> LspResult<Option<SemanticTokensResult>> {
        let docs = self.documents.read().await;
        let source = match docs.get(&params.text_document.uri) {
            Some(s) => s.clone(),
            None => return Ok(None),
        };
        drop(docs);

        let tokens = Self::semantic_tokens(&source);
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens,
        })))
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let docs = self.documents.read().await;
        let source = match docs.get(uri) {
            Some(s) => s.clone(),
            None => return Ok(None),
        };
        drop(docs);

        let word = word_at(&source, pos.line as usize, pos.character as usize);
        if word.is_empty() {
            return Ok(None);
        }

        // Look up in stdlib
        let all = ash_stdlib::all_functions();
        let matches: Vec<_> = all
            .iter()
            .filter(|f| f.full_name() == word || f.name == word)
            .collect();

        if matches.is_empty() {
            return Ok(None);
        }

        let mut md = String::new();
        for f in matches {
            let params_str: Vec<String> =
                f.params.iter().map(|(n, t)| format!("{n}: {t}")).collect();
            md.push_str(&format!(
                "**{}**({}) → {}\n\n{}\n\n",
                f.full_name(),
                params_str.join(", "),
                f.ret,
                f.doc,
            ));
        }

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: md,
            }),
            range: None,
        }))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let docs = self.documents.read().await;
        let source = match docs.get(uri) {
            Some(s) => s.clone(),
            None => return Ok(None),
        };
        drop(docs);

        let word = word_at(&source, pos.line as usize, pos.character as usize);
        if word.is_empty() {
            return Ok(None);
        }

        // Scan source lines for `fn <word>` or `type <word>`
        for (line_idx, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with(&format!("fn {word}"))
                || trimmed.starts_with(&format!("type {word}"))
            {
                let col = line.find(&word).unwrap_or(0) as u32;
                let range = Range {
                    start: Position {
                        line: line_idx as u32,
                        character: col,
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: col + word.len() as u32,
                    },
                };
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: uri.clone(),
                    range,
                })));
            }
        }

        Ok(None)
    }
}

// ─── Helper: extract word at cursor ──────────────────────────────────────────

fn word_at(source: &str, line: usize, col: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let line_str = match lines.get(line) {
        Some(l) => l,
        None => return String::new(),
    };
    let chars: Vec<char> = line_str.chars().collect();
    if col >= chars.len() {
        return String::new();
    }

    let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '.';

    let mut start = col;
    while start > 0 && is_word(chars[start - 1]) {
        start -= 1;
    }
    let mut end = col;
    while end < chars.len() && is_word(chars[end]) {
        end += 1;
    }

    chars[start..end].iter().collect()
}

// ─── Public entry point ───────────────────────────────────────────────────────

/// Start the LSP server on stdin/stdout (standard stdio transport).
pub async fn run_server() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(AshBackend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
