//! Release checks through `craft-update`, the library every Craft App shares. Only public release
//! metadata is requested.

use photocraft_engine::updates_cmds::{CheckFn, CheckResult, Release};

pub fn service() -> CheckFn {
    Box::new(|| {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("update check".into())
            .spawn(move || {
                let result = check();
                if let Err(e) = &result {
                    log::warn!("Update check: {e}");
                }
                let _ = tx.send(result);
            })
            .map_err(|e| format!("Could not start update check: {e}"))?;
        Ok(rx)
    })
}

/// What the last self-update came to, once: `craft-update` removes its records as it reads them.
pub fn last_update() -> Option<Result<String, String>> {
    use craft_update::self_update::{Outcome, current_version, take_outcome};
    let app = &photocraft_ui_egui::UPDATE_APP;
    let paths = craft_update::Paths::for_app(app).ok()?;
    let version = photocraft_engine::build_info::VERSION;
    let running = current_version(app, version).map_or_else(|| version.to_string(), |v| v.to_string());
    match take_outcome(&paths, &running)? {
        Outcome::Installed(installed) => Some(Ok(installed.to_string())),
        Outcome::Failed(reason) => Some(Err(reason)),
    }
}

fn check() -> CheckResult {
    use craft_update::http::Fetched;
    let app = &photocraft_ui_egui::UPDATE_APP;
    let http = craft_update::Http::new("PIRA/Update")?;
    let version = photocraft_engine::build_info::VERSION;
    let running = craft_update::self_update::current_version(app, version).map_or_else(|| version.to_string(), |v| v.to_string());
    let url = format!("https://api.github.com/repos/{}/releases?per_page=30", app.repo);
    let found = match http.get_api(&url, None)? {
        Fetched::Body { text, .. } => offered(&text, app.repo, &running)?,
        Fetched::RateLimited { .. } => return Err("GitHub's request limit was reached; try again in an hour".into()),
        Fetched::NotModified | Fetched::NotFound => None,
    };
    Ok(found.map(|release| Release { version: release.tag.trim_start_matches(['v', 'V']).to_string(), url: release.html_url }))
}

/// The release in GitHub's release list `json` to offer a copy running `running`, if any.
///
/// `craft-update`'s own check only ever offers stable releases. PIRA ships betas
/// (`v0.2.0-beta`, `v0.2.0-beta.2`, marked as pre-releases), so a beta copy follows the betas and
/// then the release (0.2.0-beta < 0.2.0-beta.2 < 0.2.0), while a stable copy is only offered
/// stable releases.
fn offered(json: &str, repo: &str, running: &str) -> Result<Option<craft_update::Release>, String> {
    let Some(running) = craft_update::parse_version(running) else { return Ok(None) };
    let follow_betas = !running.pre.is_empty();
    let latest = craft_update::parse_github_release_list(json, repo, follow_betas)?;
    Ok(latest.filter(|release| release.version.as_ref().is_some_and(|v| craft_update::version::offers_update(v, &running))))
}

#[cfg(test)]
mod tests {
    use super::offered;

    const REPO: &str = "PIRA-Painting-Illustration/PIRA";

    fn list(releases: &[(&str, bool)]) -> String {
        let items: Vec<String> = releases
            .iter()
            .map(|(tag, pre)| format!(r#"{{"tag_name":"{tag}","prerelease":{pre},"draft":false,"assets":[]}}"#))
            .collect();
        format!("[{}]", items.join(","))
    }

    fn offer(releases: &[(&str, bool)], running: &str) -> Option<String> {
        offered(&list(releases), REPO, running).unwrap().map(|r| r.tag)
    }

    #[test]
    fn a_beta_follows_the_betas_then_the_release() {
        assert_eq!(offer(&[("v0.2.0-beta.2", true), ("v0.2.0-beta", true)], "0.2.0-beta").as_deref(), Some("v0.2.0-beta.2"));
        assert_eq!(offer(&[("v0.2.0-beta", true)], "0.2.0-beta"), None, "already on it");
        assert_eq!(offer(&[("v0.2.0", false), ("v0.2.0-beta.2", true)], "0.2.0-beta.2").as_deref(), Some("v0.2.0"));
        assert_eq!(offer(&[("v0.2.0-beta.2", true)], "0.2.0"), None, "a beta never replaces its release");
    }

    #[test]
    fn a_stable_copy_is_offered_only_stable_releases() {
        assert_eq!(offer(&[("v0.3.0-beta", true), ("v0.2.1", false)], "0.2.0").as_deref(), Some("v0.2.1"));
        assert_eq!(offer(&[("v0.3.0-beta", true)], "0.2.0"), None);
    }
}
