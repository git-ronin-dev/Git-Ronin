//! Exercises a provider against a real service, for checking the parsing
//! against live answers. The token comes from the file named by
//! `RONIN_TOKEN_FILE`, so it never shows up in a command line.
//!
//! ```sh
//! RONIN_TOKEN_FILE=~/token cargo run -p ronin-hosting --example live -- \
//!     github https://github.com owner/repo read
//! ```
//!
//! Commands: `read` lists things; `issue` creates an issue; `pr <source>
//! <target>` opens a pull request, comments, approves and prints it;
//! `merge <number>` squash-merges; `ci <sha>`; `detail <number>`.

use std::sync::Arc;

use ronin_hosting::{
    Account, Credential, MergeMethod, NewPullRequest, ProviderKind, UreqTransport, User,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [kind, url, repo, command, rest @ ..] = args.as_slice() else {
        eprintln!("usage: live <github|gitlab|…> <url> <repo> <command> [args]");
        std::process::exit(2);
    };
    let kind: ProviderKind =
        serde_json::from_value(serde_json::Value::String(kind.clone())).expect("a provider kind");
    let file = std::env::var("RONIN_TOKEN_FILE").expect("RONIN_TOKEN_FILE");
    let token = std::fs::read_to_string(file).expect("the token file");
    let account = Account {
        kind,
        url: url.clone(),
        login: std::env::var("RONIN_LOGIN").unwrap_or_default(),
        ..Account::default()
    };
    let provider = ronin_hosting::connect(
        &account,
        &Credential::token(token.trim()),
        Arc::new(UreqTransport::default()),
    )
    .expect("a provider");
    let me: User = provider.user().expect("the user");
    println!("signed in as {} ({}, id {})", me.username, me.name, me.id);
    let show = |label: &str, value: &dyn std::fmt::Debug| println!("{label}: {value:#?}");
    match (command.as_str(), rest) {
        ("read", _) => {
            let repos = provider.repos(20).expect("repos");
            let found = repos.iter().find(|r| &r.path == repo);
            println!("{} repos; {repo}: {found:#?}", repos.len());
            show("pull requests", &provider.pull_requests(repo));
            show("issues", &provider.issues(repo));
            show("my pull requests", &provider.my_pull_requests(&me));
            show("my issues", &provider.my_issues(&me));
        }
        ("issue", _) => show(
            "created",
            &provider.create_issue(repo, "Git Ronin test issue", "Created by a live check."),
        ),
        ("pr", [source, target]) => {
            let pr = provider
                .create_pull_request(
                    repo,
                    &NewPullRequest {
                        title: format!("Git Ronin test: {source}"),
                        body: "Opened by a live check.".into(),
                        source_branch: source.clone(),
                        target_branch: target.clone(),
                        source_repo: None,
                        draft: false,
                    },
                )
                .expect("a new pull request");
            show("created", &pr);
            show(
                "comment",
                &provider.comment(repo, pr.number, "A comment from a live check."),
            );
            show("approve", &provider.approve(repo, pr.number, &me));
            show("detail", &provider.pull_request(repo, pr.number));
        }
        ("detail", [number]) => show(
            "detail",
            &provider.pull_request(repo, number.parse().expect("a number")),
        ),
        ("merge", [number]) => show(
            "merge",
            &provider.merge(repo, number.parse().expect("a number"), MergeMethod::Squash),
        ),
        ("ci", [sha]) => show("ci", &provider.ci_status(repo, sha)),
        ("key", [title, key]) => show("ssh key", &provider.add_ssh_key(&me, title, key)),
        ("fork", _) => show("fork", &provider.fork(repo)),
        _ => {
            eprintln!("unknown command");
            std::process::exit(2);
        }
    }
}
