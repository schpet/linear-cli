//! The context every command runs in: loaded configuration, lazily resolved
//! credentials and API client, the async runtime, and the terminal.
use std::cell::OnceCell;
use std::future::Future;
use std::io::{self, IsTerminal};
use std::num::NonZeroU16;
use std::path::{Path, PathBuf};

use crate::auth::keyring::NativeKeyringReader;
use crate::auth::{
    self, ApiKeyInput, CredentialSelection, CredentialSelectionInputs, CredentialStore,
    CredentialWarning, LookupFailureCategory,
};
use crate::config::{ConfigOptions, StartupConfig, TransportEnvInputs};
use crate::error::{Error, Result};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::markdown_terminal::{self, HostSource, RenderOptions};
use crate::platform::output::{self, Stdout, StdoutWriter};
use crate::platform::spinner::Spinner;
use crate::platform::{editor, opener, pager, style};
use crate::refs::WorkspaceScope;

/// Which standard streams are terminals, and whether each gets color.
#[derive(Clone, Copy, Debug)]
pub struct Terminal {
    pub stdin_tty: bool,
    pub stdout_tty: bool,
    pub stderr_tty: bool,
    /// `NO_COLOR` is set to a nonempty value.
    pub no_color: bool,
}

impl Terminal {
    pub fn detect(no_color: bool) -> Self {
        Self {
            stdin_tty: io::stdin().is_terminal(),
            stdout_tty: io::stdout().is_terminal(),
            stderr_tty: io::stderr().is_terminal(),
            no_color,
        }
    }

    pub fn stdout_color(self) -> bool {
        self.stdout_tty && !self.no_color
    }

    pub fn stderr_color(self) -> bool {
        self.stderr_tty && !self.no_color
    }
}

pub struct Ctx {
    config: StartupConfig,
    debug: bool,
    workspace: Option<String>,
    cwd: PathBuf,
    terminal: Terminal,
    credentials_path: Option<PathBuf>,
    credentials: OnceCell<CredentialStore>,
    client: OnceCell<GraphQlTransport>,
    runtime: tokio::runtime::Runtime,
    stdout: Stdout,
}

pub struct CtxInit {
    pub config: StartupConfig,
    pub debug: bool,
    /// The global `--workspace` flag.
    pub workspace: Option<String>,
    pub cwd: PathBuf,
    pub terminal: Terminal,
    pub credentials_path: Option<PathBuf>,
}

impl Ctx {
    pub fn new(init: CtxInit) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                Error::new(format!("Failed to start the async runtime: {error}")).with_source(error)
            })?;
        Ok(Self {
            config: init.config,
            debug: init.debug,
            workspace: init.workspace,
            cwd: init.cwd,
            terminal: init.terminal,
            credentials_path: init.credentials_path,
            credentials: OnceCell::new(),
            client: OnceCell::new(),
            runtime,
            stdout: Stdout::new(),
        })
    }

    pub fn config(&self) -> &StartupConfig {
        &self.config
    }

    pub fn options(&self) -> &ConfigOptions {
        &self.config.options
    }

    /// The global `--workspace` flag.
    pub fn workspace(&self) -> Option<&str> {
        self.workspace.as_deref()
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn debug(&self) -> bool {
        self.debug
    }

    pub fn terminal(&self) -> Terminal {
        self.terminal
    }

    pub fn stdin_tty(&self) -> bool {
        self.terminal.stdin_tty
    }

    pub fn stdout_tty(&self) -> bool {
        self.terminal.stdout_tty
    }

    /// Whether stdout output may be colored.
    pub fn color(&self) -> bool {
        self.terminal.stdout_color()
    }

    pub fn credentials_path(&self) -> Option<&Path> {
        self.credentials_path.as_deref()
    }

    /// The credentials file, read on first use. Keyring entries are read
    /// later, per workspace, when a key is needed.
    pub fn credentials(&self) -> Result<&CredentialStore> {
        if let Some(store) = self.credentials.get() {
            return Ok(store);
        }
        let store = auth::file::load(self.credentials_path(), Box::new(NativeKeyringReader))?;
        Ok(self.credentials.get_or_init(|| store))
    }

    /// The workspace-related inputs that decide which credential is used.
    pub fn selection(&self) -> CredentialSelectionInputs<'_> {
        selection_inputs(self.options(), self.workspace())
    }

    /// Local workspace knowledge for checking Linear URLs against the active workspace.
    pub fn scope(&self) -> Result<WorkspaceScope<'_>> {
        Ok(WorkspaceScope::new(
            self.selection(),
            self.credentials()?.default(),
        ))
    }

    /// The authenticated API client, built on first use and shared after.
    pub fn client(&self) -> Result<&GraphQlTransport> {
        if let Some(client) = self.client.get() {
            return Ok(client);
        }
        let credentials = self.credentials()?;
        let client = connect(
            self.options(),
            credentials,
            &self.selection(),
            &self.config.transport_env,
        );
        self.report_credential_warnings(credentials)?;
        let client = client?;
        Ok(self.client.get_or_init(|| client))
    }

    fn report_credential_warnings(&self, credentials: &CredentialStore) -> Result<()> {
        for warning in credentials.take_warnings() {
            let line = style::warning(&credential_warning(&warning), self.terminal.stderr_color());
            self.eprint(format!("{line}\n"))?;
        }
        Ok(())
    }

    /// Runs `future` to completion on the process's runtime.
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }

    /// Like [`Ctx::block_on`], with a spinner on stderr while it runs. Pass
    /// `show = false` for `--json` output. Nothing is drawn unless stderr is a
    /// terminal.
    pub fn spin<F: Future>(&self, show: bool, future: F) -> F::Output {
        let _spinner = self.spinner(show, "");
        self.block_on(future)
    }

    /// Like [`Ctx::spin`], with `message` next to the spinner.
    pub fn spin_with<F: Future>(&self, message: &str, future: F) -> F::Output {
        let _spinner = self.spinner(true, message);
        self.block_on(future)
    }

    /// A spinner on stderr that stops and clears when dropped.
    pub fn spinner(&self, show: bool, message: &str) -> Spinner {
        if show && self.terminal.stderr_tty {
            Spinner::start(message)
        } else {
            Spinner::hidden()
        }
    }

    /// Writes to stdout (buffered until the command ends or [`Ctx::flush`]).
    pub fn print(&self, output: impl AsRef<[u8]>) -> Result<()> {
        self.stdout.write(output.as_ref())
    }

    /// Writes to stderr, after flushing stdout so the streams stay in order.
    pub fn eprint(&self, output: impl AsRef<[u8]>) -> Result<()> {
        self.flush()?;
        output::eprint(output.as_ref())
    }

    pub fn flush(&self) -> Result<()> {
        self.stdout.flush()
    }

    /// An `io::Write` handle on stdout, for code that streams output.
    pub fn stdout(&self) -> StdoutWriter<'_> {
        self.stdout.writer()
    }

    /// Shows rendered terminal output, through the pager when `paging` is on
    /// and it does not fit on the screen.
    pub fn page(&self, rendered: &str, paging: bool) -> Result<()> {
        if paging && self.terminal.stdout_tty && pager::too_long(rendered, pager::stdout_size()) {
            self.flush()?;
            match pager::page(
                rendered,
                self.config.pager.as_deref(),
                &self.config.child_env,
            )? {
                pager::Paged::Shown => return Ok(()),
                pager::Paged::NoPager => {}
            }
        }
        self.print(rendered)?;
        if !rendered.ends_with('\n') {
            self.print("\n")?;
        }
        Ok(())
    }

    /// Prints Markdown: rendered for the terminal (and paged when `paging` is
    /// on and it is long) when stdout is a terminal, verbatim otherwise.
    pub fn show_markdown(&self, markdown: &str, paging: bool) -> Result<()> {
        if !self.terminal.stdout_tty {
            return self.print(format!("{markdown}\n"));
        }
        let rendered = self.render_markdown(markdown)?;
        self.page(&rendered, paging)
    }

    /// Renders Markdown for the stdout terminal.
    pub fn render_markdown(&self, markdown: &str) -> Result<String> {
        let columns = pager::stdout_size()
            .and_then(|size| NonZeroU16::new(size.columns))
            .unwrap_or(markdown_terminal::FALLBACK_COLUMNS);
        let hyperlinks = self
            .options()
            .hyperlink_format()
            .map(|value| value.value().as_str());
        let options =
            RenderOptions::for_terminal(columns, self.color(), hyperlinks, HostSource::System);
        markdown_terminal::render(markdown, &options)
    }

    /// Opens `initial` in the user's editor and returns the saved text.
    pub fn edit_text(&self, initial: &str) -> Result<String> {
        self.flush()?;
        editor::edit(initial, &self.config.child_env)
    }

    /// Opens a page of the active workspace (`path` like `issue/ENG-1`) in the
    /// browser, or in the desktop app with `app`.
    pub fn open_in_linear(&self, path: &str, app: bool) -> Result<()> {
        let url = format!("https://linear.app/{}/{path}", self.workspace_url_key()?);
        self.open_url(&url, app)
    }

    /// Opens a full Linear URL in the browser, or in the desktop app with `app`.
    pub fn open_url(&self, url: &str, app: bool) -> Result<()> {
        let destination = if app { "Linear.app" } else { "web browser" };
        self.print(format!("Opening {url} in {destination}\n"))?;
        self.flush()?;
        opener::open(url, app)
    }

    /// The URL key of the active workspace. `--workspace` and the configured
    /// workspace name it directly; a stored credential is named by its
    /// workspace; otherwise (an API key from the environment or config) the
    /// API is asked.
    pub fn workspace_url_key(&self) -> Result<String> {
        let configured = self
            .workspace()
            .or_else(|| {
                self.options()
                    .workspace()
                    .map(|value| value.value().as_str())
            })
            .filter(|value| !value.is_empty());
        if let Some(workspace) = configured {
            return Ok(workspace.to_owned());
        }
        let stored_default = match ApiKeyInput::from_options(self.options()) {
            ApiKeyInput::Absent => self.credentials()?.default().map(str::to_owned),
            ApiKeyInput::Raw { .. } | ApiKeyInput::Sourced { .. } => None,
        };
        if let Some(workspace) = stored_default {
            return Ok(workspace);
        }
        let client = self.client()?;
        self.block_on(crate::graphql::operations::viewer::url_key(client))
    }

    /// Fails unless stdin is a terminal, naming `flag` as the way to skip the prompt.
    pub fn require_tty(&self, flag: &str) -> Result<()> {
        if self.terminal.stdin_tty {
            Ok(())
        } else {
            Err(Error::new(
                "This command needs to ask for confirmation, but stdin is not a terminal",
            )
            .with_hint(format!("Pass {flag} to proceed without a prompt.")))
        }
    }

    /// Asks a yes/no question on the terminal. Without a terminal it fails,
    /// naming `skip_flag` as the way to proceed. Ctrl-C cancels the command.
    pub fn confirm(&self, message: &str, skip_flag: &str) -> Result<bool> {
        use crate::platform::prompt::{PromptOutcome, PromptSession};
        self.require_tty(skip_flag)?;
        self.flush()?;
        let mut session = PromptSession::stdin_stdio(self.stdout())?;
        let result = session.confirm(message, false);
        match session.finish_result(result)? {
            PromptOutcome::Submitted(answer) => Ok(answer),
            PromptOutcome::Interrupted => Err(Error::cancelled()),
            PromptOutcome::EndOfInput => Err(Error::new("Unexpected end of input at a prompt")),
        }
    }

    /// The interactive prompt session for multi-step prompts. Its output goes
    /// to stdout; non-terminal stdin is read line by line.
    pub fn prompts(
        &self,
    ) -> Result<crate::platform::prompt::PromptSession<io::Stdin, StdoutWriter<'_>>> {
        self.flush()?;
        crate::platform::prompt::PromptSession::stdin_stdio_cr_or_lf(self.stdout())
    }
}

pub fn selection_inputs<'a>(
    options: &'a ConfigOptions,
    cli_workspace: Option<&'a str>,
) -> CredentialSelectionInputs<'a> {
    CredentialSelectionInputs {
        api_key: ApiKeyInput::from_options(options),
        cli_workspace,
        sourced_workspace: options
            .workspace()
            .map(|resolved| (resolved.value().as_str(), resolved.source().clone())),
    }
}

/// Builds an API client from the selected credential.
pub fn connect(
    options: &ConfigOptions,
    credentials: &CredentialStore,
    inputs: &CredentialSelectionInputs<'_>,
    transport_env: &TransportEnvInputs,
) -> Result<GraphQlTransport> {
    let secret = match auth::resolve(inputs, credentials) {
        CredentialSelection::Selected { secret, .. } => secret,
        CredentialSelection::NoKey => {
            return Err(Error::auth("No API key configured").with_hint(
                "Set LINEAR_API_KEY, add api_key to .linear.toml, or run `linear auth login`.",
            ));
        }
        CredentialSelection::EnvWorkspaceConflict => {
            return Err(
                Error::new("Cannot use --workspace while LINEAR_API_KEY is set")
                    .with_hint("Unset LINEAR_API_KEY or remove the --workspace flag."),
            );
        }
        CredentialSelection::MissingExplicitWorkspace { workspace } => {
            return Err(Error::auth(format!(
                "Workspace \"{workspace}\" not found in credentials"
            ))
            .with_hint(
                "Run `linear auth login` to add it, or `linear auth list` to see configured workspaces.",
            ));
        }
    };
    let key = auth::header::to_api_key(secret).map_err(|error| {
        Error::new("API key cannot be used as an HTTP header").with_source(error)
    })?;
    GraphQlTransport::new(
        options.endpoint().value().clone(),
        key,
        transport_env.production(),
    )
    .map_err(Error::from)
}

fn credential_warning(warning: &CredentialWarning) -> String {
    match warning {
        CredentialWarning::InvalidDefault { workspace } => format!(
            "Warning: Default workspace \"{workspace}\" is not in the workspaces list. Run `linear auth default <workspace>` to set a valid default."
        ),
        CredentialWarning::LookupMiss { workspace } => format!(
            "Warning: No keyring entry for workspace \"{workspace}\". Run `linear auth login` to re-authenticate."
        ),
        CredentialWarning::LookupFailed {
            workspace,
            category,
        } => {
            let reason = match category {
                LookupFailureCategory::Unavailable => "keyring tool unavailable",
                LookupFailureCategory::Permission => "permission denied",
                LookupFailureCategory::Other => "lookup failed",
                LookupFailureCategory::UnsupportedPlatform => "unsupported platform",
            };
            format!("Warning: Failed to read keyring for workspace \"{workspace}\": {reason}")
        }
    }
}
