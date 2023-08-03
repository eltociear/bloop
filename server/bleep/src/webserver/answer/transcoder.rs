//! Transcoder for articles generated by the LLM.
//!
//! The LLM generates articles with a special format, for example it uses XML to denote code blocks
//! instead of regular Markdown code blocks. This module both decodes this format into markdown
//! components, and encodes them back.

use std::{borrow::Cow, collections::HashMap};

use anyhow::{Context, Result};
use comrak::nodes::{NodeHtmlBlock, NodeValue};
use lazy_regex::regex;
use regex::Regex;
use serde::Deserialize;

/// Decode an article.
///
/// If successful, this returns a tuple of `(body, conclusion)`.
pub fn decode(llm_message: &str) -> (String, Option<String>) {
    let sanitized = sanitize(llm_message);
    let markdown = xml_for_each(&sanitized, |code| xml_to_markdown(code).ok());

    // The `comrak` crate has a very unusual API which makes this logic difficult to follow. It
    // favours arena allocation instead of a tree-based AST, and requires `Write`rs to regenerate
    // markdown output.
    //
    // There are quirks to the parsing logic, comments have been added for clarity.

    let arena = comrak::Arena::new();
    let mut options = comrak::ComrakOptions::default();
    options.extension.footnotes = true;

    // We don't have an easy built-in way to generate a string with `comrak`, so we encapsulate
    // that logic here.
    let comrak_to_string = |node| {
        let mut out = Vec::<u8>::new();
        comrak::format_commonmark(node, &options, &mut out).unwrap();
        String::from_utf8_lossy(&out).trim().to_owned()
    };

    // `comrak` will not recognize footnote definitions unless they have been referenced at least
    // once. To ensure our potential summary appears in the parse tree, we prepend the entire
    // response with a sentinel reference to the footnote. After parsing, we look for that
    // footnote and immediately remove (detach) it from the root node. This ensures that our
    // artifical reference does not appear in the output.

    let document = format!("[^summary]\n\n{markdown}");
    let root = comrak::parse_document(&arena, &document, &options);
    let mut children = root.children();
    // Detach the sentinel footnote reference.
    children.next().unwrap().detach();

    for child in children {
        match &child.data.borrow().value {
            NodeValue::FootnoteDefinition(def) if def.name == "summary" => (),
            _ => continue,
        };

        if let Some(first_child) = child.children().next() {
            if let NodeValue::Paragraph = &first_child.data.borrow().value {
                // We detach the summary from the main text, so that it does not end up in the final
                // article output.
                child.detach();
                return (comrak_to_string(root), Some(comrak_to_string(first_child)));
            }
        }
    }

    (comrak_to_string(root), None)
}

pub fn encode(markdown: &str, conclusion: Option<&str>) -> String {
    let arena = comrak::Arena::new();
    let mut options = comrak::ComrakOptions::default();
    options.extension.footnotes = true;

    let root = comrak::parse_document(&arena, markdown, &options);

    for child in root.children() {
        let (info, literal) = match &mut child.data.borrow_mut().value {
            NodeValue::CodeBlock(block) => (block.info.clone(), block.literal.clone()),
            _ => continue,
        };

        let attributes = info
            .split(',')
            .filter_map(|param| {
                let mut iter = param.trim().split(':');

                let key = iter.next()?;
                let value = iter.next()?;

                Some((key.to_owned(), value.to_owned()))
            })
            .collect::<HashMap<String, String>>();

        let xml = attributes.get("type").and_then(|ty| match ty.as_str() {
            "Quoted" => {
                let path = attributes.get("path")?;
                let lang = attributes.get("lang")?;
                let mut lines = attributes.get("lines")?.split('-');

                let start_line = lines.next()?;
                let end_line = lines.next()?;

                Some(format!(
                    "<QuotedCode>\n\
                    <Code>\n\
                    {literal}\
                    </Code>\n\
                    <Language>{lang}</Language>\n\
                    <Path>{path}</Path>\n\
                    <StartLine>{start_line}</StartLine>\n\
                    <EndLine>{end_line}</EndLine>\n\
                    </QuotedCode>"
                ))
            }

            "Generated" => {
                let lang = attributes.get("lang")?;

                Some(format!(
                    "<GeneratedCode>\n\
                    <Code>\n\
                    {literal}\
                    </Code>\n\
                    <Language>{lang}</Language>\n\
                    </GeneratedCode>"
                ))
            }

            _ => None,
        });

        if let Some(xml) = xml {
            child.data.borrow_mut().value = NodeValue::HtmlBlock(NodeHtmlBlock {
                literal: xml,
                // The block type here is not used.
                block_type: 0,
            });
        }
    }

    let mut out = Vec::<u8>::new();
    comrak::format_commonmark(root, &options, &mut out).unwrap();
    let body = String::from_utf8_lossy(&out).trim().to_owned();

    if let Some(conclusion) = conclusion {
        body + "\n\n[^summary]: " + conclusion
    } else {
        body
    }
}

pub fn encode_summarized(markdown: &str, conclusion: Option<&str>, model: &str) -> Result<String> {
    let article = xml_for_each(&encode(markdown, conclusion), |xml| {
        try_trim_code_xml(xml).ok()
    });
    let bpe = tiktoken_rs::get_bpe_from_model(model)?;
    Ok(super::limit_tokens(&article, bpe, 500).to_owned())
}

fn sanitize(article: &str) -> String {
    let sanitized = xml_for_each(article, |code| Some(fixup_xml_code(code).into_owned()));
    regex!("<!--.*?-->")
        .replace_all(&sanitized, "")
        .into_owned()
}

fn fixup_xml_code(xml: &str) -> Cow<str> {
    if !xml.trim().starts_with('<') {
        return Cow::Borrowed(xml);
    }

    if let Some(match_) = regex!("<(Generated|Quoted)Code>\\s*<Code>(.*)"sm)
        .captures(xml)
        .and_then(|cap| cap.get(2))
    {
        let mut buf = String::new();

        buf += &xml[..match_.start()];

        // First, we clean up incorrectly escaped symbols in the code block.
        {
            let s = &xml[match_.range()];

            let code_len = regex!("</Code>")
                .find(s)
                .map(|m| m.start())
                .unwrap_or(s.len());
            let (s, tail) = s.split_at(code_len);

            // The `regex` crate does not support negative lookahead, so we cannot write a regex
            // like `&(?!amp;)`. So, we just perform naive substitutions to first obtain an
            // unescaped copy of the string, and then re-escape it in order to fix up the result.
            //
            // This matters if the input string is something like `&amp;foo < &bar&lt;i32&gt;()`:
            //
            // - First, we convert that to `&foo < &bar<i32>()`
            // - Second, we convert it to `&amp;foo < &amp;bar&lt;i32&gt;`, our desired result.

            let s = regex!("&lt;"m).replace_all(s, "<");
            let s = regex!("&gt;"m).replace_all(&s, ">");
            let s = regex!("&amp;"m).replace_all(&s, "&");

            let s = regex!("&"m).replace_all(&s, "&amp;");
            let s = regex!("<"m).replace_all(&s, "&lt;");
            let s = regex!(">"m).replace_all(&s, "&gt;");

            buf += &s;
            buf += tail;
        }

        {
            // Next, we clean up the tags.
            //
            // Because the LLM is generating XML output token-by-token, we may end up in a
            // situation where closing tags are missing, or tags are half written. To fix this,
            // first we remove all half-complete opening or closing tags (e.g. `<foo` or `</`).
            // Then, we add missing closing tags, *in the order we expect them to appear in the
            // final XML output.* This is not perfect, but it should work well enough to allow us
            // to parse the XML.

            buf = regex!("<[^>]*$").replace_all(&buf, "").into_owned();

            for tag in [
                "Code",
                "Language",
                "Path",
                "StartLine",
                "EndLine",
                "QuotedCode",
                "GeneratedCode",
            ] {
                let opening_tag = format!("<{tag}>");
                let closing_tag = format!("</{tag}>");

                if buf.contains(&opening_tag) && !buf.contains(&closing_tag) {
                    buf += &closing_tag;
                }
            }
        }

        Cow::Owned(buf)
    } else {
        Cow::Borrowed(xml)
    }
}

fn xml_to_markdown(xml: &str) -> Result<String> {
    let code_chunk =
        quick_xml::de::from_str::<CodeChunk>(xml).context("failed to deserialize code chunk")?;

    Ok(code_chunk.to_markdown())
}

/// An XML code chunk that is generated by the LLM.
#[derive(serde::Deserialize, Debug)]
enum CodeChunk {
    QuotedCode {
        #[serde(default, rename = "Code")]
        code: String,
        #[serde(default, rename = "Language")]
        language: String,
        #[serde(default, rename = "Path")]
        path: String,
        #[serde(default, rename = "StartLine", deserialize_with = "deserialize_lineno")]
        start_line: Option<u32>,
        #[serde(default, rename = "EndLine", deserialize_with = "deserialize_lineno")]
        end_line: Option<u32>,
    },
    GeneratedCode {
        #[serde(default, rename = "Code")]
        code: String,
        #[serde(default, rename = "Language")]
        language: String,
    },
}

fn deserialize_lineno<'a, D: serde::Deserializer<'a>>(de: D) -> Result<Option<u32>, D::Error> {
    let opt = Option::deserialize(de)?;
    let opt = opt.and_then(|s: String| {
        if s.is_empty() {
            Some(0)
        } else {
            s.parse().ok()
        }
    });

    Ok(opt)
}

impl CodeChunk {
    fn to_markdown(&self) -> String {
        let (ty, code, lang, path, start, end) = match self {
            CodeChunk::QuotedCode {
                code,
                language,
                path,
                start_line,
                end_line,
            } => (
                "Quoted",
                code,
                language,
                path.as_str(),
                *start_line,
                *end_line,
            ),
            CodeChunk::GeneratedCode { code, language } => {
                ("Generated", code, language, "", None, None)
            }
        };

        format!(
            "```type:{ty},lang:{lang},path:{path},lines:{}-{}\n{code}\n```",
            start.unwrap_or(0),
            end.unwrap_or(0)
        )
    }
}

/// Modify every XML section of a markdown document.
///
/// The provided closure returns an option, which returns `Some(..)` with a replacement for the
/// input string, or `None` if the input string does not need to be replaced.
///
/// This function operates heuristically, in order to allow malformed XML and XML that contains
/// multiple serial newlines. This means we accept invalid markdown, and are more forgiving with
/// the input, at the expense of creating parsing edge cases that can cause trouble due to input
/// ambiguity.
///
/// One such case is this:
///
/// ```xml
/// This is a sample markdown document. **Hello** world.
///
/// <Code>
///     println!("code ends with </Code>");
/// </Code>
/// ```
///
/// The above markdown document contains an XML block enclosed in `<Code>...</Code>`, but it is
/// not valid as the code snippet contains unescape characters. Of note, the `println!` call
/// contains literal `<` and `>` characters, which in valid XML *must* be escaped as `&lt;` and
/// `&gt;`, respectively. Because of this, the xml block will be incorrectly parsed to terminate
/// halfway through the string literal provided in the code sample.
///
/// In general, there is no great way around this. We tolerate *most* ambiguity, but this edge case
/// remains as a consequence of ambiguous input.
///
/// For further context, we must accept ambiguous unescaped (invalid) input, as the LLM may
/// generate such documents.
fn xml_for_each(article: &str, f: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = article;

    while let Some(captures) = regex!(r"\n\s*(<(\w+)>)").captures(rest) {
        let tag = captures.get(1).unwrap();
        let name = &rest[captures.get(2).unwrap().range()];

        out += &rest[..tag.start()];

        let xml = if let Some(m) = Regex::new(&format!(r"</{name}>")).unwrap().find(rest) {
            let xml = &rest[tag.start()..m.end()];
            rest = &rest[m.end()..];
            xml
        } else {
            let xml = &rest[tag.start()..];
            rest = "";
            xml
        };

        if let Some(update) = f(xml) {
            out += &update;
        } else {
            out += xml;
        }
    }

    out += rest;
    out
}

fn try_trim_code_xml(xml: &str) -> Result<String> {
    let xml = fixup_xml_code(xml);

    let code_chunk = quick_xml::de::from_str(&xml).context("couldn't parse as XML code block")?;

    Ok(match code_chunk {
        CodeChunk::QuotedCode {
            code: _,
            language,
            path,
            start_line,
            end_line,
        } => {
            let start_line = start_line
                .map(|n| format!("<StartLine>{n}</StartLine>\n"))
                .unwrap_or_default();
            let end_line = end_line
                .map(|n| format!("<EndLine>{n}</EndLine>\n"))
                .unwrap_or_default();

            format!(
                "<QuotedCode>\n\
                <Code>[REDACTED]</Code>\n\
                <Language>{language}</Language>\n\
                <Path>{path}</Path>\n\
                {start_line}\
                {end_line}\
                </QuotedCode>"
            )
        }

        CodeChunk::GeneratedCode { code: _, language } => {
            format!(
                "<GeneratedCode>\n\
                <Code>[REDACTED]</Code>\n\
                <Language>{language}</Language>\n\
                </GeneratedCode>"
            )
        }
    })
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_trim_code() {
        let input = "Sample Markdown test.

<QuotedCode>
<Code>
fn foo() -> i32 {
    42
}
</Code>
<Language>Rust</Language>
<Path>src/main.rs</Path>
<StartLine>10</StartLine>
<EndLine>12</EndLine>
</QuotedCode>

<GeneratedCode>
<Code>
fn foo() -> i32 {
    42
}
</Code>
<Language>Rust</Language>
</GeneratedCode>

test
test
test";

        let expected = "Sample Markdown test.

<QuotedCode>
<Code>[REDACTED]</Code>
<Language>Rust</Language>
<Path>src/main.rs</Path>
<StartLine>10</StartLine>
<EndLine>12</EndLine>
</QuotedCode>

<GeneratedCode>
<Code>[REDACTED]</Code>
<Language>Rust</Language>
</GeneratedCode>

test
test
test";

        let out = xml_for_each(input, |code| try_trim_code_xml(code).ok());

        assert_eq!(expected, out);
    }

    #[test]
    fn test_fixup_quoted_code() {
        let input = "<QuotedCode>
<Code>
fn foo<T>(t: T) -> bool {
    &amp;foo < &bar&lt;i32&gt;(t)
}
</Code>
<Language>Rust</Language>
<Path>src/main.rs</Path>
<StartLine>10</StartLine>
<EndLine>12</EndLine>
</QuotedCode>";

        let expected = "<QuotedCode>
<Code>
fn foo&lt;T&gt;(t: T) -&gt; bool {
    &amp;foo &lt; &amp;bar&lt;i32&gt;(t)
}
</Code>
<Language>Rust</Language>
<Path>src/main.rs</Path>
<StartLine>10</StartLine>
<EndLine>12</EndLine>
</QuotedCode>";

        assert_eq!(expected, &fixup_xml_code(input));
    }

    #[test]
    fn test_fixup_generated_code() {
        let input = "<GeneratedCode>
<Code>
fn foo<T>(t: T) -> bool {
    &amp;foo < &bar&lt;i32&gt;(t)
}
</Code>
<Language>Rust</Language>
</GeneratedCode>";

        let expected = "<GeneratedCode>
<Code>
fn foo&lt;T&gt;(t: T) -&gt; bool {
    &amp;foo &lt; &amp;bar&lt;i32&gt;(t)
}
</Code>
<Language>Rust</Language>
</GeneratedCode>";

        assert_eq!(expected, &fixup_xml_code(input));
    }

    #[test]
    fn test_sanitize_article() {
        let input = "First, we test some *generated code* below:

<GeneratedCode>
<Code>
fn foo<T>(t: T) -> bool {
    &amp;foo < &bar&lt;i32&gt;(t)
}
</Code>
<Language>Rust</Language>
</GeneratedCode>

Then, we test some quoted code:

<QuotedCode>
<Code>
fn foo<T>(t: T) -> bool {
    &amp;foo < &bar&lt;i32&gt;(t)
}
</Code>
<Language>Rust</Language>
<Path>src/main.rs</Path>
<StartLine>10</StartLine>
<EndLine>12</EndLine>
</QuotedCode>

# Foo

These should result in sanitized XML output, while maintaining the rest of the markdown article.
";

        let expected = "First, we test some *generated code* below:

<GeneratedCode>
<Code>
fn foo&lt;T&gt;(t: T) -&gt; bool {
    &amp;foo &lt; &amp;bar&lt;i32&gt;(t)
}
</Code>
<Language>Rust</Language>
</GeneratedCode>

Then, we test some quoted code:

<QuotedCode>
<Code>
fn foo&lt;T&gt;(t: T) -&gt; bool {
    &amp;foo &lt; &amp;bar&lt;i32&gt;(t)
}
</Code>
<Language>Rust</Language>
<Path>src/main.rs</Path>
<StartLine>10</StartLine>
<EndLine>12</EndLine>
</QuotedCode>

# Foo

These should result in sanitized XML output, while maintaining the rest of the markdown article.
";

        assert_eq!(expected, sanitize(&input));
    }

    #[test]
    fn test_sanitize_article_partial_generation() {
        let input = "First, we test some **partially** *generated code* below:

<GeneratedCode>
<Code>
fn foo<T>(t: T) -> bool {
    &amp;foo <
";

        let expected = "First, we test some **partially** *generated code* below:

<GeneratedCode>
<Code>
fn foo&lt;T&gt;(t: T) -&gt; bool {
    &amp;foo &lt;
</Code></GeneratedCode>";

        assert_eq!(expected, sanitize(&input));
    }

    #[test]
    fn test_decode_2() {
        let input = "First, we test some *generated code* below:

<GeneratedCode>
<Code>
fn foo&lt;T&gt;(t: T) -&gt; bool {
    &amp;foo &lt; &amp;bar&lt;i32&gt;(t)
}
</Code>
<Language>Rust</Language>
</GeneratedCode>

Then, we test some quoted code:

<QuotedCode>
<Code>
fn foo&lt;T&gt;(t: T) -&gt; bool {
    &amp;foo &lt; &amp;bar&lt;i32&gt;(t)
}
</Code>
<Language>Rust</Language>
<Path>src/main.rs</Path>
<StartLine>10</StartLine>
<EndLine>12</EndLine>
</QuotedCode>

# Foo

These should result in base64-encoded XML output, while maintaining the rest of the markdown article.";

        let expected = "First, we test some *generated code* below:

``` type:Generated,lang:Rust,path:,lines:0-0
fn foo<T>(t: T) -> bool {
    &foo < &bar<i32>(t)
}
```

Then, we test some quoted code:

``` type:Quoted,lang:Rust,path:src/main.rs,lines:10-12
fn foo<T>(t: T) -> bool {
    &foo < &bar<i32>(t)
}
```

# Foo

These should result in base64-encoded XML output, while maintaining the rest of the markdown article.";

        let (body, conclusion) = decode(&input);
        assert_eq!(None, conclusion);
        assert_eq!(expected, body);
    }

    #[test]
    fn test_decode_partial_xml() {
        let input = "The `Compiler` struct in [`server/bleep/src/query/compiler.rs`](server/bleep/src/query/compiler.rs) is used to compile a list of queries into a single Tantivy query that matches any of them. Here is an example of its usage:

<QuotedCode>
<Code>
let mut compiler = Compiler::new();
compiler.literal(schema.name, |q| q.repo.clone());
let compiled_query = compiler.compile(queries, tantivy_index);
</Code>
<Language>Rust</Language>
<Path>server/bleep/s
";

        let expected = "The `Compiler` struct in [`server/bleep/src/query/compiler.rs`](server/bleep/src/query/compiler.rs) is used to compile a list of queries into a single Tantivy query that matches any of them. Here is an example of its usage:

``` type:Quoted,lang:Rust,path:server/bleep/s,lines:0-0
let mut compiler = Compiler::new();
compiler.literal(schema.name, |q| q.repo.clone());
let compiled_query = compiler.compile(queries, tantivy_index);
```";

        let (body, conclusion) = decode(input);

        assert_eq!(None, conclusion);
        assert_eq!(expected, body);
    }

    #[test]
    fn test_decode_partial_xml_no_path() {
        let input = "## Example of Using the Query Compiler

The `Compiler` struct in [`server/bleep/src/query/compiler.rs`](server/bleep/src/query/compiler.rs) is used to compile a list of queries into a single Tantivy query that matches any of them. Here is an example of its usage:

<QuotedCode>
<Code>
let mut compiler = Compiler::new();
compiler.literal(schema.name, |q| q.repo.clone());
let compiled_query = compiler.compile(queries, tantivy_index);
</Code>
<Language>Rust</Language>
</QuotedCode>
";

        let expected = "## Example of Using the Query Compiler

The `Compiler` struct in [`server/bleep/src/query/compiler.rs`](server/bleep/src/query/compiler.rs) is used to compile a list of queries into a single Tantivy query that matches any of them. Here is an example of its usage:

``` type:Quoted,lang:Rust,path:,lines:0-0
let mut compiler = Compiler::new();
compiler.literal(schema.name, |q| q.repo.clone());
let compiled_query = compiler.compile(queries, tantivy_index);
```";

        let (body, conclusion) = decode(input);
        assert_eq!(None, conclusion);
        assert_eq!(expected, body);
    }

    #[test]
    fn test_sanitize_multi_blocks() {
        let input = "## Example of Using the Query Compiler

The `Compiler` struct in [`server/bleep/src/query/compiler.rs`](server/bleep/src/query/compiler.rs) is used to compile a list of queries into a single Tantivy query that matches any of them. Here is an example of its usage:

<QuotedCode>
<Code>
let mut compiler = Compiler::new();

compiler.literal(schema.name, |q| q.repo.clone());
let compiled_query =
";

        let expected = "## Example of Using the Query Compiler

The `Compiler` struct in [`server/bleep/src/query/compiler.rs`](server/bleep/src/query/compiler.rs) is used to compile a list of queries into a single Tantivy query that matches any of them. Here is an example of its usage:

``` type:Quoted,lang:,path:,lines:0-0
let mut compiler = Compiler::new();

compiler.literal(schema.name, |q| q.repo.clone());
let compiled_query =
```";

        let (body, conclusion) = decode(input);
        assert_eq!(None, conclusion);
        assert_eq!(expected, body);
    }

    #[test]
    fn test_decode_partial_xml_empty_line_number() {
        let input = "## Example of Using the Query Compiler

The `Compiler` struct in [`server/bleep/src/query/compiler.rs`](server/bleep/src/query/compiler.rs) is used to compile a list of queries into a single Tantivy query that matches any of them. Here is an example of its usage:

<QuotedCode>
<Code>
let mut compiler = Compiler::new();
compiler.literal(schema.name, |q| q.repo.clone());
let compiled_query = compiler.compile(queries, tantivy_index);
</Code>
<Language>Rust</Language>
<StartLine>";

        let expected = "## Example of Using the Query Compiler

The `Compiler` struct in [`server/bleep/src/query/compiler.rs`](server/bleep/src/query/compiler.rs) is used to compile a list of queries into a single Tantivy query that matches any of them. Here is an example of its usage:

``` type:Quoted,lang:Rust,path:,lines:0-0
let mut compiler = Compiler::new();
compiler.literal(schema.name, |q| q.repo.clone());
let compiled_query = compiler.compile(queries, tantivy_index);
```";

        let (body, conclusion) = decode(input);
        assert_eq!(None, conclusion);
        assert_eq!(expected, body);
    }

    #[test]
    fn test_decode_with_summary_and_xml() {
        let input = "Bug reports are sent to the endpoint `https://api.bloop.ai/bug_reports` via a POST request. This is done in the function [`saveBugReport`](client/src/services/api.ts#L168-L172) in the file `client/src/services/api.ts`.

Here is the relevant code:
<QuotedCode>
<Code>
export const saveBugReport = (report: {
  email: string;
  name: string;
  text: string;
  unique_id: string;
}) => axios.post(`${DB_API}/bug_reports`, report).then((r) => r.data);
</Code>
<Language>TypeScript</Language>
<Path>client/src/services/api.ts</Path>
<StartLine>168</StartLine>
<EndLine>172</EndLine>
</QuotedCode>

[^summary]: Bug reports are sent to the endpoint `https://api.bloop.ai/bug_reports` via a POST request in the `saveBugReport` function.";
        let (article, summary) = decode(&input);

        let expected_article = "Bug reports are sent to the endpoint `https://api.bloop.ai/bug_reports` via a POST request. This is done in the function [`saveBugReport`](client/src/services/api.ts#L168-L172) in the file `client/src/services/api.ts`.

Here is the relevant code:

``` type:Quoted,lang:TypeScript,path:client/src/services/api.ts,lines:168-172
export const saveBugReport = (report: {
  email: string;
  name: string;
  text: string;
  unique_id: string;
}) => axios.post(`${DB_API}/bug_reports`, report).then((r) => r.data);
```";

        let expected_summary = "Bug reports are sent to the endpoint `https://api.bloop.ai/bug_reports` via a POST request in the `saveBugReport` function.";

        assert_eq!(expected_article, article);
        assert_eq!(expected_summary, summary.unwrap());
    }

    #[test]
    fn test_decode() {
        let (body, summary) = decode(
            r#"Hello world

[^summary]: This is an example summary, with **bold text**."#,
        );

        assert_eq!(body, "Hello world");
        assert_eq!(
            summary.unwrap(),
            "This is an example summary, with **bold text**."
        );

        let (body, summary) = decode(
            r#"Hello world.

Goodbye world.

Hello again, world.

[^summary]: This is an example summary, with **bold text**."#,
        );

        assert_eq!(
            body,
            "Hello world.\n\nGoodbye world.\n\nHello again, world."
        );
        assert_eq!(
            summary.unwrap(),
            "This is an example summary, with **bold text**."
        );
    }

    #[test]
    fn test_encode() {
        let input = "Foo

``` type:Quoted,lang:Rust,path:src/main.rs,lines:1-3
fn main() {
    println!(\"hello world\");
}
```

Bar.

``` type:Generated,lang:Rust,path:,lines:0-0
fn main() {
    println!(\"hello world\");
}
```

";

        let expected = "Foo

<QuotedCode>
<Code>
fn main() {
    println!(\"hello world\");
}
</Code>
<Language>Rust</Language>
<Path>src/main.rs</Path>
<StartLine>1</StartLine>
<EndLine>3</EndLine>
</QuotedCode>

Bar.

<GeneratedCode>
<Code>
fn main() {
    println!(\"hello world\");
}
</Code>
<Language>Rust</Language>
</GeneratedCode>

[^summary]: Test **summary**.";

        let encoded = encode(input, Some("Test **summary**."));

        assert_eq!(expected, encoded);
    }

    #[test]
    fn test_encode_summarized() {
        let input = "Foo

``` type:Quoted,lang:Rust,path:src/main.rs,lines:1-3
fn main() {
    println!(\"hello world\");
}
```

Bar.

``` type:Generated,lang:Rust,path:,lines:0-0
fn main() {
    println!(\"hello world\");
}
```

";

        let expected = "Foo

<QuotedCode>
<Code>[REDACTED]</Code>
<Language>Rust</Language>
<Path>src/main.rs</Path>
<StartLine>1</StartLine>
<EndLine>3</EndLine>
</QuotedCode>

Bar.

<GeneratedCode>
<Code>[REDACTED]</Code>
<Language>Rust</Language>
</GeneratedCode>

[^summary]: Test **summary**.";

        let encoded = encode_summarized(input, Some("Test **summary**."), "gpt-4-0613").unwrap();

        assert_eq!(expected, encoded);
    }

    #[test]
    fn test_xml_empty_lines() {
        let input = "
Foo



bar

<GeneratedCode>
<Code>
fn main() {
    let x = 1;

    dbg!(x);
}
</Code>
<Language>Rust</Language>
</GeneratedCode>

quux";

        let expected = "Foo

bar

``` type:Generated,lang:Rust,path:,lines:0-0
fn main() {
    let x = 1;

    dbg!(x);
}
```

quux";

        let (body, conclusion) = decode(&sanitize(input));

        assert_eq!(None, conclusion);
        assert_eq!(expected, body);
    }
}
