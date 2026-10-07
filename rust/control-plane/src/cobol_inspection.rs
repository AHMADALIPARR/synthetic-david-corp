//! Bounded lexical structure inspection. This is deliberately not a compiler.
use crate::{Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Clone)]
struct Word {
    text: String,
    line: usize,
    start: usize,
    end: usize,
}
fn span(first: &Word, last: &Word) -> Value {
    json!({"start":{"line":first.line,"column":first.start},"end":{"line":last.line,"column":last.end}})
}
fn name(word: &Word) -> bool {
    word.text.bytes().any(|b| b.is_ascii_alphabetic())
        && word
            .text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

pub(super) fn inspect(payload: &Value) -> Result<Value> {
    let text = payload["text"].as_str().ok_or("SCHEMA_MISMATCH")?;
    let dialect = payload["dialect"].as_str().ok_or("UNKNOWN_DIALECT")?;
    ensure(
        matches!(dialect, "COBOL85-FIXED" | "COBOL85-FREE"),
        "UNKNOWN_DIALECT",
    )?;
    let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
    ensure(
        payload["source"]["sha256"] == digest,
        "SOURCE_HASH_MISMATCH",
    )?;
    ensure(
        text.bytes()
            .all(|b| b.is_ascii_graphic() || matches!(b, b' ' | b'\r' | b'\n')),
        "UNSUPPORTED_SOURCE_ENCODING",
    )?;
    ensure(
        !text.replace("\r\n", "\n").contains('\r'),
        "UNSUPPORTED_LINE_ENDING",
    )?;
    let fixed = dialect == "COBOL85-FIXED";
    let mut words = Vec::new();
    let mut diagnostics = Vec::new();
    for (line_index, raw) in text.split('\n').enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let bytes = line.as_bytes();
        let (start, end) = if fixed {
            if bytes.len() < 7 {
                ensure(bytes.iter().all(|b| *b == b' '), "INVALID_FIXED_FORMAT")?;
                continue;
            }
            ensure(
                bytes[..6].iter().all(|b| *b == b' ' || b.is_ascii_digit()),
                "INVALID_FIXED_FORMAT",
            )?;
            match bytes[6] {
                b'*' | b'/' => continue,
                b'-' => return Err("UNSUPPORTED_CONTINUATION".into()),
                b'D' | b'd' => return Err("UNSUPPORTED_DEBUG_LINE".into()),
                b' ' => (),
                _ => return Err("INVALID_FIXED_FORMAT".into()),
            }
            if bytes.len() > 72 && bytes[72..].iter().any(|b| *b != b' ') {
                diagnostics.push(json!({"code":"FIXED_IDENTIFICATION_AREA_IGNORED","line":line_index+1,"startColumn":73}));
            }
            (7, bytes.len().min(72))
        } else {
            (0, bytes.len())
        };
        let mut i = start;
        while i < end {
            if bytes[i] == b' ' {
                i += 1;
                continue;
            }
            if i + 1 < end && &bytes[i..i + 2] == b"*>" {
                break;
            }
            if bytes[i] == b'\'' || bytes[i] == b'"' {
                let quote = bytes[i];
                i += 1;
                let mut closed = false;
                while i < end {
                    if bytes[i] == quote {
                        i += 1;
                        if i < end && bytes[i] == quote {
                            i += 1;
                        } else {
                            closed = true;
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
                ensure(closed, "UNSUPPORTED_MULTILINE_LITERAL")?;
                // A sentinel prevents declarations from matching across a literal.
                words.push(Word {
                    text: "<LITERAL>".into(),
                    line: line_index + 1,
                    start: i + 1,
                    end: i + 1,
                });
                continue;
            }
            let begin = i;
            i += 1;
            if bytes[begin].is_ascii_alphanumeric() {
                while i < end && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
                    i += 1;
                }
            }
            words.push(Word {
                text: line[begin..i].to_ascii_uppercase(),
                line: line_index + 1,
                start: begin + 1,
                end: i + 1,
            });
        }
    }
    ensure(
        !words
            .iter()
            .any(|w| matches!(w.text.as_str(), "COPY" | "REPLACE" | "EXEC" | ">>")),
        "UNSUPPORTED_PREPROCESSING",
    )?;
    // Compiler directives are not accepted even when the lexer separates > tokens.
    ensure(
        !words
            .windows(2)
            .any(|w| w[0].text == ">" && w[1].text == ">"),
        "UNSUPPORTED_PREPROCESSING",
    )?;
    let mut divisions = Vec::new();
    let mut programs = Vec::new();
    let mut sections = Vec::new();
    let mut paragraphs = Vec::new();
    let mut data_items = Vec::new();
    let mut in_procedure = false;
    let mut in_data = false;
    for (i, word) in words.iter().enumerate() {
        let tail = &words[i..];
        if tail.len() >= 3
            && tail[1].text == "DIVISION"
            && matches!(
                word.text.as_str(),
                "IDENTIFICATION" | "ENVIRONMENT" | "DATA" | "PROCEDURE"
            )
        {
            in_procedure = word.text == "PROCEDURE";
            in_data = word.text == "DATA";
            let last = if tail[2].text == "." {
                &tail[2]
            } else {
                &tail[1]
            };
            divisions.push(json!({"name":word.text,"span":span(word,last)}));
        }
        if word.text == "PROGRAM-ID"
            && tail.len() >= 4
            && tail[1].text == "."
            && name(&tail[2])
            && tail[3].text == "."
        {
            programs.push(json!({"name":tail[2].text,"span":span(word,&tail[3])}));
        }
        if tail.len() >= 3 && name(word) && tail[1].text == "SECTION" && tail[2].text == "." {
            sections.push(json!({"name":word.text,"span":span(word,&tail[2])}));
        }
        let first_on_line = i == 0 || words[i - 1].line != word.line;
        if in_procedure
            && first_on_line
            && name(word)
            && !word.text.starts_with("END-")
            && !matches!(
                word.text.as_str(),
                "EXIT"
                    | "GOBACK"
                    | "CONTINUE"
                    | "ADD"
                    | "ACCEPT"
                    | "CALL"
                    | "CANCEL"
                    | "CLOSE"
                    | "COMPUTE"
                    | "DELETE"
                    | "DISPLAY"
                    | "DIVIDE"
                    | "EVALUATE"
                    | "INITIALIZE"
                    | "INSPECT"
                    | "MERGE"
                    | "MOVE"
                    | "MULTIPLY"
                    | "OPEN"
                    | "PERFORM"
                    | "READ"
                    | "RELEASE"
                    | "RETURN"
                    | "REWRITE"
                    | "SEARCH"
                    | "SET"
                    | "SORT"
                    | "START"
                    | "STOP"
                    | "STRING"
                    | "SUBTRACT"
                    | "UNSTRING"
                    | "WRITE"
                    | "IF"
                    | "ELSE"
                    | "NEXT"
                    | "SENTENCE"
            )
            && tail.len() >= 2
            && tail[1].text == "."
            && tail[1].line == word.line
            && (tail.len() == 2 || tail[2].line != word.line)
        {
            paragraphs.push(json!({"name":word.text,"span":span(word,&tail[1])}));
        }
        if in_data && first_on_line && word.text.len() == 2 && tail.len() >= 2 && name(&tail[1]) {
            if let Ok(level) = word.text.parse::<u8>() {
                if (1..=49).contains(&level) || matches!(level, 66 | 77 | 88) {
                    data_items.push(
                        json!({"name":tail[1].text,"level":level,"span":span(word,&tail[1])}),
                    );
                }
            }
        }
    }
    ensure(
        programs.len() == 1 && divisions.iter().any(|d| d["name"] == "IDENTIFICATION"),
        "UNSUPPORTED_PROGRAM_STRUCTURE",
    )?;
    Ok(
        json!({"schema":"synthetic-david/AnalysisIR","version":1,"language":"COBOL","dialect":dialect,"source":payload["source"],"sourceHashAlgorithm":"SHA-256-RAW-UTF8","sourceSha256":digest,"spanConvention":"one-based ASCII columns; end exclusive","programs":programs,"divisions":divisions,"sections":sections,"paragraphs":paragraphs,"dataItems":data_items,"diagnostics":diagnostics,"compilerValidated":false,"semanticEquivalenceProven":false,"sourceOfTruth":"LEGACY","limitations":["Lexical declaration inventory only; declarations are candidates, not a validated symbol table.","No parsing of statements, control flow, PIC semantics, external dependencies or data layout.","One program only; ASCII source; no tabs, continuation/debug lines, multiline literals, COPY/REPLACE/EXEC or compiler directives.","Paragraph candidates must occupy their own physical line; procedure USING headers are not fully parsed.","Fixed source uses columns 8-72; sequence and identification areas are not executable source."]}),
    )
}
