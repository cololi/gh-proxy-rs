//! URL pattern matching for GitHub and Hugging Face resources.
//!
//! Ported from `internal/matcher/matcher.go`. Patterns are translated verbatim;
//! the `regex` crate uses RE2-like syntax compatible with Go's `regexp`.

use once_cell::sync::Lazy;
use regex::Regex;

// GitHub patterns.
pub static EXP_RELEASE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:https?://)?(?:www\.)?github\.com/([^/]+?)/([^/]+?)/(?:releases|archive)/.*$")
        .expect("EXP_RELEASE")
});
pub static EXP_BLOB: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:https?://)?(?:www\.)?github\.com/([^/]+?)/([^/]+?)/(?:blob|raw)/.*$")
        .expect("EXP_BLOB")
});
pub static EXP_GIT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:https?://)?(?:www\.)?github\.com/([^/]+?)/([^/]+?)/(?:info|git-).*$")
        .expect("EXP_GIT")
});
pub static EXP_TREE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:https?://)?(?:www\.)?github\.com/([^/]+?)/([^/]+?)/(?:tree|tag)/.*$")
        .expect("EXP_TREE")
});
pub static EXP_REPO: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:https?://)?(?:www\.)?github\.com/([^/]+?)/([^/]+?)/?$").expect("EXP_REPO")
});
pub static EXP_RAW: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(?:https?://)?raw\.(?:githubusercontent|github)\.com/([^/]+?)/([^/]+?)/.+?/.+$",
    )
    .expect("EXP_RAW")
});
pub static EXP_GIST: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:https?://)?gist\.(?:githubusercontent|github)\.com/([^/]+?)/.+?/.+$")
        .expect("EXP_GIST")
});

// Hugging Face patterns. Order matters: dataset_git and spaces_git must come
// before the generic git matcher; the *_root variants follow their non-root
// siblings; repo is last.
pub static EXP_HF_DATASET_GIT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(?:https?://)?(?:www\.)?huggingface\.co/(datasets/[^/]+?)/([^/]+?)/(?:info|git-|resolve|raw|blob).*$",
    )
    .expect("EXP_HF_DATASET_GIT")
});
pub static EXP_HF_DATASET_GIT_ROOT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(?:https?://)?(?:www\.)?huggingface\.co/(datasets/[^/]+?)/(?:info|git-|resolve|raw|blob).*$",
    )
    .expect("EXP_HF_DATASET_GIT_ROOT")
});
pub static EXP_HF_SPACES_GIT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(?:https?://)?(?:www\.)?huggingface\.co/(spaces/[^/]+?)/([^/]+?)/(?:info|git-|resolve|raw|blob).*$",
    )
    .expect("EXP_HF_SPACES_GIT")
});
pub static EXP_HF_GIT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(?:https?://)?(?:www\.)?huggingface\.co/([^/]+?)/([^/]+?)/(?:info|git-|resolve|raw|blob).*$",
    )
    .expect("EXP_HF_GIT")
});
pub static EXP_HF_GIT_ROOT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(?:https?://)?(?:www\.)?huggingface\.co/([^/]+?)/(?:info|git-|resolve|raw|blob).*$",
    )
    .expect("EXP_HF_GIT_ROOT")
});
pub static EXP_HF_REPO: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:https?://)?(?:www\.)?huggingface\.co/([^/]+?)(?:/([^/]+?))?/?$")
        .expect("EXP_HF_REPO")
});

/// Run a URL against an ordered list of patterns and return the first match's
/// capture groups (excluding the full match). For optional groups that did not
/// participate, returns an empty string — matching Go's `FindStringSubmatch`
/// semantics where every declared group has a slot in the result.
fn match_first(exps: &[&Lazy<Regex>], u: &str) -> Option<Vec<String>> {
    for exp in exps {
        if let Some(cap) = exp.captures(u) {
            let groups: Vec<String> = cap
                .iter()
                .skip(1)
                .map(|m| m.map(|x| x.as_str().to_string()).unwrap_or_default())
                .collect();
            return Some(groups);
        }
    }
    None
}

/// Match the URL against GitHub then Hugging Face pattern lists and return the
/// capture groups (typically `[user]` or `[user, repo]`). Returns `None` if no
/// pattern matches or the URL doesn't reference either host family.
pub fn match_url(u: &str) -> Option<Vec<String>> {
    if u.contains("github") {
        let gh_exps: [&Lazy<Regex>; 7] = [
            &EXP_RELEASE,
            &EXP_BLOB,
            &EXP_GIT,
            &EXP_TREE,
            &EXP_REPO,
            &EXP_RAW,
            &EXP_GIST,
        ];
        if let Some(groups) = match_first(&gh_exps, u) {
            return Some(groups);
        }
    }
    if u.contains("huggingface.co") {
        let hf_exps: [&Lazy<Regex>; 6] = [
            &EXP_HF_DATASET_GIT,
            &EXP_HF_DATASET_GIT_ROOT,
            &EXP_HF_SPACES_GIT,
            &EXP_HF_GIT,
            &EXP_HF_GIT_ROOT,
            &EXP_HF_REPO,
        ];
        if let Some(groups) = match_first(&hf_exps, u) {
            return Some(groups);
        }
    }
    None
}

/// Reports whether the URL is a GitHub or Hugging Face blob (browser preview) URL.
pub fn is_blob(u: &str) -> bool {
    EXP_BLOB.is_match(u) || (u.contains("/blob/") && is_hf(u))
}

/// Reports whether the URL is a Hugging Face URL.
pub fn is_hf(u: &str) -> bool {
    u.contains("huggingface.co")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> String {
        v.to_string()
    }

    #[test]
    fn test_match_url() {
        let cases: &[(&str, &str, Option<Vec<String>>)] = &[
            (
                "release",
                "https://github.com/user/repo/releases/download/v1/file.zip",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "archive",
                "https://github.com/user/repo/archive/main.zip",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "blob",
                "https://github.com/user/repo/blob/main/README.md",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "raw on github.com",
                "https://github.com/user/repo/raw/main/file.txt",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "git info",
                "https://github.com/user/repo/info/refs",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "git-upload-pack",
                "https://github.com/user/repo/git-upload-pack",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "raw subdomain",
                "https://raw.githubusercontent.com/user/repo/main/file.txt",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "raw github short",
                "https://raw.github.com/user/repo/main/file.txt",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "gist",
                "https://gist.githubusercontent.com/user/abcdef/raw/file.txt",
                Some(vec![s("user")]),
            ),
            (
                "no scheme",
                "github.com/user/repo/releases/v1/x",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "http scheme",
                "http://github.com/user/repo/blob/main/x",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "github root",
                "https://github.com/user/repo",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "github tree",
                "https://github.com/user/repo/tree/main",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "github tag",
                "https://github.com/user/repo/tag/v1.0",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "www github",
                "https://www.github.com/user/repo",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "hf repo",
                "https://huggingface.co/user/repo",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "hf model",
                "https://huggingface.co/gpt2",
                Some(vec![s("gpt2"), s("")]),
            ),
            (
                "non-github",
                "https://example.com/user/repo/blob/main/x",
                None,
            ),
        ];
        for (name, url, want) in cases {
            let got = match_url(url);
            assert_eq!(&got, want, "case {name}: match_url({url:?})");
        }
    }

    #[test]
    fn test_match_hf_url() {
        let cases: &[(&str, &str, Option<Vec<String>>)] = &[
            (
                "hf model git info",
                "https://huggingface.co/gpt2/info/refs",
                Some(vec![s("gpt2")]),
            ),
            (
                "hf user model git info",
                "https://huggingface.co/user/repo/info/refs",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "hf dataset git info",
                "https://huggingface.co/datasets/user/repo/info/refs",
                Some(vec![s("datasets/user"), s("repo")]),
            ),
            (
                "hf dataset root git info",
                "https://huggingface.co/datasets/glue/info/refs",
                Some(vec![s("datasets/glue")]),
            ),
            (
                "hf model root git upload pack",
                "https://huggingface.co/gpt2/git-upload-pack",
                Some(vec![s("gpt2")]),
            ),
            (
                "hf space git info",
                "https://huggingface.co/spaces/user/repo/info/refs",
                Some(vec![s("spaces/user"), s("repo")]),
            ),
            (
                "hf model git upload pack",
                "https://huggingface.co/user/repo/git-upload-pack",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "hf model resolve",
                "https://huggingface.co/user/repo/resolve/main/config.json",
                Some(vec![s("user"), s("repo")]),
            ),
            (
                "hf model blob",
                "https://huggingface.co/user/repo/blob/main/README.md",
                Some(vec![s("user"), s("repo")]),
            ),
        ];
        for (name, url, want) in cases {
            let got = match_url(url);
            assert_eq!(&got, want, "case {name}: match_url({url:?})");
        }
    }

    #[test]
    fn test_is_blob() {
        assert!(is_blob("https://github.com/user/repo/blob/main/README.md"));
        assert!(is_blob("https://huggingface.co/user/repo/blob/main/x"));
        assert!(!is_blob(
            "https://github.com/user/repo/releases/download/v1/file.zip"
        ));
    }

    #[test]
    fn test_is_hf() {
        assert!(is_hf("https://huggingface.co/gpt2"));
        assert!(!is_hf("https://github.com/user/repo"));
    }
}
