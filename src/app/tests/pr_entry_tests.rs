//! What a PR review session looks like *after* the App has actually entered
//! one.
//!
//! These drive the real entry point — `App::enter_pr_diff_mode` — rather
//! than hand-assigning the state it is supposed to produce. That matters:
//! a test that sets `vcs_info.vcs_type = VcsType::PullRequest` itself and
//! then asserts on it still passes when `enter_pr_diff_mode` stops setting
//! it, which is exactly the regression worth guarding.

use std::path::{Path, PathBuf};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

use crate::app::{App, DiffSource, InputMode};
use crate::error::Result;
use crate::forge::pr_open::{OpenedPullRequest, prepare_open_pr};
use crate::forge::traits::{
    ForgeBackend, ForgeFileLinesRequest, ForgeRepository, PagedPullRequests, PullRequestCommit,
    PullRequestDetails, PullRequestInfo, PullRequestListQuery, PullRequestReviewMetadata,
    PullRequestTarget,
};
use crate::model::{DiffFile, DiffLine, FilePatch, FileStatus, ReviewSession, SessionDiffSource};
use crate::syntax::SyntaxHighlighter;
use crate::vcs::traits::{VcsBackend, VcsInfo, VcsType};

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

/// One file, two additions and one deletion, so the rendered `+N -N` stat
/// is non-empty and distinguishable from the suppressed case.
const SIMPLE_PATCH: &str = r##"diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,4 @@
 pub fn answer() -> u32 {
-    41
+    42
+    // and a comment
 }
"##;

fn repo() -> ForgeRepository {
    ForgeRepository::github("github.com", "agavra", "tuicr")
}

fn pr_details() -> PullRequestDetails {
    PullRequestDetails {
        repository: repo(),
        number: 125,
        title: "Review workflow".to_string(),
        url: "https://github.com/agavra/tuicr/pull/125".to_string(),
        state: "OPEN".to_string(),
        is_draft: false,
        author: Some("alice".to_string()),
        head_ref_name: "reviews".to_string(),
        base_ref_name: "main".to_string(),
        head_sha: "abcdef0123456789".to_string(),
        base_sha: "1234567890abcdef".to_string(),
        body: "body".to_string(),
        updated_at: None,
        closed: false,
        merged_at: None,
        diff_start_sha: None,
    }
}

/// Minimal `ForgeBackend`. `enter_pr_diff_mode` only parks the backend on
/// the App for later context expansion, so nothing here is called during
/// entry; the methods exist to satisfy the trait.
struct StubForge {
    details: PullRequestDetails,
}

impl StubForge {
    fn new() -> Self {
        Self {
            details: pr_details(),
        }
    }
}

impl ForgeBackend for StubForge {
    fn list_pull_requests(&self, _query: PullRequestListQuery) -> Result<PagedPullRequests> {
        unimplemented!("PR entry does not list pull requests")
    }
    fn get_pull_request(&self, _target: PullRequestTarget) -> Result<PullRequestDetails> {
        Ok(self.details.clone())
    }
    fn get_pull_request_diff(&self, _pr: &PullRequestDetails) -> Result<Vec<FilePatch>> {
        Ok(crate::vcs::diff_parser::git_fixture_file_patches(
            SIMPLE_PATCH,
        ))
    }
    fn fetch_file_lines(&self, _request: ForgeFileLinesRequest) -> Result<Vec<DiffLine>> {
        Ok(Vec::new())
    }
    fn list_review_threads(
        &self,
        _pr: &PullRequestDetails,
    ) -> Result<Vec<crate::forge::remote_comments::RemoteReviewThread>> {
        Ok(Vec::new())
    }
    fn list_pull_request_commits(
        &self,
        _pr: &PullRequestDetails,
    ) -> Result<Vec<PullRequestCommit>> {
        Ok(Vec::new())
    }
    fn get_pull_request_commit_range_diff(
        &self,
        _pr: &PullRequestDetails,
        _start_sha: &str,
        _end_sha: &str,
    ) -> Result<Vec<FilePatch>> {
        Ok(crate::vcs::diff_parser::git_fixture_file_patches(
            SIMPLE_PATCH,
        ))
    }
    fn create_review(
        &self,
        _pr: &PullRequestDetails,
        _request: crate::forge::traits::CreateReviewRequest<'_>,
    ) -> Result<crate::forge::traits::GhCreateReviewResponse> {
        unimplemented!("PR entry does not submit reviews")
    }
}

/// Build the `OpenedPullRequest` handed to `enter_pr_diff_mode`, through the
/// same `prepare_open_pr` the production open path uses. Only the network
/// half (`fetch_pr_data`) is stubbed out.
fn opened_pr() -> OpenedPullRequest {
    let details = pr_details();
    let patches = crate::vcs::diff_parser::git_fixture_file_patches(SIMPLE_PATCH);
    let highlighter = SyntaxHighlighter::default();
    prepare_open_pr(
        details,
        patches,
        Vec::new(),
        PullRequestReviewMetadata::default(),
        PullRequestInfo::from_details(pr_details()),
        None,
        &highlighter,
    )
    .expect("prepare opened pr")
}

struct StubVcs(VcsInfo);

impl VcsBackend for StubVcs {
    fn info(&self) -> &VcsInfo {
        &self.0
    }
    fn get_working_tree_diff(&self, _hl: &SyntaxHighlighter) -> Result<Vec<DiffFile>> {
        Ok(Vec::new())
    }
    fn fetch_context_lines(
        &self,
        _path: &Path,
        _status: FileStatus,
        _ref_commit: Option<&str>,
        _start: u32,
        _end: u32,
    ) -> Result<Vec<DiffLine>> {
        Ok(Vec::new())
    }
    fn file_line_count(
        &self,
        _path: &Path,
        _status: FileStatus,
        _ref_commit: Option<&str>,
    ) -> Result<u32> {
        Ok(0)
    }
}

fn local_file(path: &str) -> DiffFile {
    DiffFile {
        old_path: None,
        new_path: Some(PathBuf::from(path)),
        status: FileStatus::Modified,
        hunks: vec![],
        is_binary: false,
        is_too_large: false,
        is_commit_message: false,
        content_hash: 0,
    }
}

/// An ordinary local working-tree App — the state a user is in before they
/// open a PR from the selector.
fn local_app() -> App {
    let vcs_info = VcsInfo {
        root_path: PathBuf::from("/tmp"),
        head_commit: "head".into(),
        branch_name: Some("main".into()),
        vcs_type: VcsType::Git,
    };
    let session = ReviewSession::new(
        vcs_info.root_path.clone(),
        vcs_info.head_commit.clone(),
        vcs_info.branch_name.clone(),
        SessionDiffSource::WorkingTree,
    );
    App::build(
        Box::new(StubVcs(vcs_info.clone())),
        vcs_info,
        crate::theme::Theme::dark(),
        None,
        false,
        vec![local_file("a.rs")],
        session,
        DiffSource::WorkingTree,
        InputMode::Normal,
        Vec::new(),
        None,
        None,
    )
    .expect("build local app")
}

/// Drive the real PR entry point on `app`.
fn enter_pr(app: &mut App) {
    app.enter_pr_diff_mode(Box::new(StubForge::new()), opened_pr())
        .expect("enter pr diff mode");
}

fn buffer_text(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn draw(app: &mut App) -> Buffer {
    let backend = TestBackend::new(120, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| crate::ui::render(frame, app))
        .expect("draw frame");
    terminal.backend().buffer().clone()
}

// ---------------------------------------------------------------------
// R1 — the in-app selector / `:reload` entry point
// ---------------------------------------------------------------------

/// A PR review is a real diff against a base branch, so the file tree must
/// keep showing per-file `+N -N` stats. `is_whole_file_view()` suppresses
/// those for `--all-files` and `--file`, and a PR session that lands on
/// `VcsType::File` gets swept up with them.
///
/// The assertion is on the session `enter_pr_diff_mode` actually produces,
/// not on a `vcs_type` the test assigned itself.
#[test]
fn should_not_treat_a_pr_session_entered_from_the_selector_as_a_whole_file_view() {
    let mut app = local_app();

    enter_pr(&mut app);

    assert!(
        !app.is_whole_file_view(),
        "a PR session must not read as a whole-file view (vcs_type was {:?}, \
         is_pristine_mode was {})",
        app.vcs_info.vcs_type,
        app.is_pristine_mode
    );
}

/// The narrower half of the same guarantee, split out so a failure says
/// which half broke: the PR session must not carry the `--file` marker.
#[test]
fn should_not_leave_a_pr_session_on_the_file_vcs_type() {
    let mut app = local_app();

    enter_pr(&mut app);

    assert_ne!(
        app.vcs_info.vcs_type,
        VcsType::File,
        "enter_pr_diff_mode left the session on VcsType::File, which \
         is_whole_file_view() reads as `--file` mode"
    );
}

// ---------------------------------------------------------------------
// R3 — entering a PR from an `--all-files` session
// ---------------------------------------------------------------------

/// `--all-files` sets `is_pristine_mode`, and `is_whole_file_view()` is an
/// OR of that flag and the VCS type. Opening a PR from such a session is a
/// full mode switch: the diff, the session and the backend are all
/// replaced, so the pristine flag from the *previous* mode must not survive
/// and keep suppressing the PR's stats.
#[test]
fn should_clear_pristine_mode_when_entering_a_pr_session() {
    let mut app = local_app();
    app.is_pristine_mode = true;

    enter_pr(&mut app);

    assert!(
        !app.is_pristine_mode,
        "entering a PR left is_pristine_mode set from the previous --all-files session"
    );
    assert!(
        !app.is_whole_file_view(),
        "a PR entered from --all-files still reads as a whole-file view"
    );
}

// ---------------------------------------------------------------------
// R6 — what the file tree actually renders
// ---------------------------------------------------------------------

/// The end the other tests are a means to: after entering a PR, the file
/// tree row for a changed file carries its `+N -N` counts. Rendering
/// through `ui::render` catches the suppression however it is reached.
#[test]
fn should_render_file_line_stats_for_a_file_in_a_pr_session() {
    let mut app = local_app();
    enter_pr(&mut app);
    app.show_file_list = true;

    let text = buffer_text(&draw(&mut app));

    assert!(
        text.contains("lib.rs +2 -1"),
        "expected per-file line stats in the PR file tree, got:\n{text}"
    );
}
