use clap::Parser;
use grepapp::{GrepAppClient, LineMatch, SearchOptions, SearchQuery, languages};
use serde::Serialize;
use std::cmp::Ordering;
use std::io::{self, IsTerminal};
use std::process;
use std::time::Duration;

#[path = "../gg_cli.rs"]
mod gg_cli;
use gg_cli::Cli;

const MATCH_START: &str = "\u{1b}[32m";
const MATCH_END: &str = "\u{1b}[0m";

#[derive(Debug)]
struct MatchRecord {
    repo: String,
    path: String,
    branch: String,
    line_number: usize,
    line: String,
    match_ranges: Vec<std::ops::Range<usize>>,
    is_match: bool,
}

#[derive(Serialize)]
struct JsonRecord {
    repo: String,
    path: String,
    branch: String,
    line_number: usize,
    line: String,
    match_ranges: Vec<[usize; 2]>,
    is_match: bool,
}

#[tokio::main]
async fn main() {
    // Preserve `gg langs` behavior while allowing the main parser to remain testable.
    // (We keep a tiny top-level command enum here rather than complicating the shared CLI struct.)
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    if args.len() >= 2 && args[1] == "langs" {
        let langs = match languages() {
            Ok(l) => l,
            Err(err) => {
                eprintln!("error: {err}");
                process::exit(1);
            }
        };
        for lang in langs {
            println!("{lang}");
        }
        return;
    }

    let cli = Cli::parse();

    let pattern = match cli.pattern.as_deref() {
        Some(pattern) => pattern,
        None => {
            eprintln!("gg: missing search pattern");
            process::exit(2);
        }
    };

    let query = build_query(&cli, pattern);
    let options = SearchOptions::default()
        .max_pages(cli.max_pages)
        .concurrency(cli.concurrency)
        .timeout(Duration::from_secs(cli.timeout_secs));

    let base_url = match reqwest::Url::parse(&cli.base_url) {
        Ok(url) => url,
        Err(err) => {
            eprintln!("gg: invalid base URL: {err}");
            process::exit(2);
        }
    };

    let client = GrepAppClient::with_base_url(base_url);

    let result = match client.search(&query, &options).await {
        Ok(result) => result,
        Err(err) => {
            eprintln!("gg: {err}");
            process::exit(2);
        }
    };

    if cli.matched_repos {
        let repos = result.matched_repos();
        if repos.is_empty() {
            return;
        }
        for repo in repos {
            println!("{repo}");
        }
        return;
    }

    let mut records = collect_records(result.hits, cli.context);
    records.sort_by(|a, b| match a.repo.cmp(&b.repo) {
        Ordering::Equal => match a.path.cmp(&b.path) {
            Ordering::Equal => a.line_number.cmp(&b.line_number),
            other => other,
        },
        other => other,
    });
    if let Some(limit) = cli.limit {
        records.truncate(limit);
    }

    if cli.json {
        emit_json(records);
        return;
    }

    let use_color = !cli.no_color && io::stdout().is_terminal();
    let grouped = cli.heading && !cli.flat;
    if grouped {
        emit_grouped(records, use_color);
    } else {
        emit_flat(records, use_color);
    }
}

fn build_query(cli: &Cli, pattern: &str) -> SearchQuery {
    let mut query = SearchQuery::new(pattern)
        .whole_words(cli.word_regexp)
        .case_sensitive(!cli.ignore_case);
    if cli.fixed_strings {
        query = query.regex(false);
    }
    if let Some(repo) = &cli.repo {
        query = query.repo_filter(repo);
    }
    if let Some(path) = &cli.path {
        query = query.path_filter(path);
    }
    if !cli.languages.is_empty() {
        query = query.languages(cli.languages.clone());
    }
    query
}

fn collect_records(hits: Vec<grepapp::SearchHit>, context: usize) -> Vec<MatchRecord> {
    let mut records = Vec::new();
    for hit in hits {
        let mut lines = hit.lines;
        lines.sort_by_key(|line| line.line_number);
        if context == 0 {
            for line in lines
                .into_iter()
                .filter(|line| !line.match_ranges.is_empty())
            {
                records.push(MatchRecord {
                    repo: hit.repo.clone(),
                    path: hit.path.clone(),
                    branch: hit.branch.clone(),
                    line_number: line.line_number,
                    line: line.line,
                    match_ranges: line.match_ranges,
                    is_match: true,
                });
            }
            continue;
        }
        let line_numbers: Vec<usize> = lines.iter().map(|line| line.line_number).collect();
        let mut keep = vec![false; lines.len()];
        for (idx, line) in lines.iter().enumerate() {
            if line.match_ranges.is_empty() {
                continue;
            }
            let min = line.line_number.saturating_sub(context);
            let max = line.line_number.saturating_add(context);
            for (pos, number) in line_numbers.iter().enumerate() {
                if *number >= min && *number <= max {
                    keep[pos] = true;
                }
            }
            keep[idx] = true;
        }
        for (line, keep_line) in lines.into_iter().zip(keep) {
            if !keep_line {
                continue;
            }
            let is_match = !line.match_ranges.is_empty();
            records.push(MatchRecord {
                repo: hit.repo.clone(),
                path: hit.path.clone(),
                branch: hit.branch.clone(),
                line_number: line.line_number,
                line: line.line,
                match_ranges: line.match_ranges,
                is_match,
            });
        }
    }
    records
}

fn emit_json(records: Vec<MatchRecord>) {
    for record in records {
        let json = JsonRecord {
            repo: record.repo,
            path: record.path,
            branch: record.branch,
            line_number: record.line_number,
            line: record.line,
            match_ranges: record
                .match_ranges
                .into_iter()
                .map(|range| [range.start, range.end])
                .collect(),
            is_match: record.is_match,
        };
        match serde_json::to_string(&json) {
            Ok(line) => println!("{line}"),
            Err(err) => eprintln!("gg: failed to serialize JSON: {err}"),
        }
    }
}

fn emit_flat(records: Vec<MatchRecord>, use_color: bool) {
    let (start, end) = if use_color {
        (MATCH_START, MATCH_END)
    } else {
        ("", "")
    };
    for record in records {
        let line = render_line(&record, start, end);
        println!(
            "{}/{}:{}:{line}",
            record.repo, record.path, record.line_number
        );
    }
}

fn emit_grouped(records: Vec<MatchRecord>, use_color: bool) {
    let (start, end) = if use_color {
        (MATCH_START, MATCH_END)
    } else {
        ("", "")
    };
    let mut current_repo = String::new();
    let mut current_path = String::new();

    for record in records {
        if record.repo != current_repo {
            current_repo = record.repo.clone();
            current_path.clear();
            println!("{current_repo}");
        }
        if record.path != current_path {
            current_path = record.path.clone();
            println!("  /{}", current_path);
        }
        let line = render_line(&record, start, end);
        println!("    {}: {line}", record.line_number);
    }
}

fn render_line(record: &MatchRecord, start: &str, end: &str) -> String {
    let line_match = LineMatch {
        line_number: record.line_number,
        line: record.line.clone(),
        match_ranges: record.match_ranges.clone(),
    };
    line_match.highlight(start, end)
}
